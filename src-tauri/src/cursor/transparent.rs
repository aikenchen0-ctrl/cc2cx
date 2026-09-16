//! Loopback fake-IP TLS entry for Cursor NodeService connections.

use std::{
    collections::HashMap,
    convert::Infallible,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use bytes::Bytes;
use http::{header::HOST, Request, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::{
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer, ServerName},
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
    ServerConfig,
};
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;

use super::{
    ca::LoadedCa,
    error::{CursorError, Result},
    fake_ip::FakeIpMap,
    routes::{classify_request, is_cursor_host, RouteDecision, UnmatchedPathPolicy},
};

type ResponseBody = BoxBody<Bytes, std::io::Error>;

pub struct TransparentRuntime {
    addresses: HashMap<String, SocketAddr>,
    backend: Arc<std::sync::Mutex<SocketAddr>>,
    cancel: CancellationToken,
    tasks: Vec<JoinHandle<()>>,
}

impl TransparentRuntime {
    pub fn address_for(&self, hostname: &str) -> SocketAddr {
        let key = hostname.trim().trim_end_matches('.').to_ascii_lowercase();
        self.addresses
            .get(&key)
            .copied()
            .unwrap_or_else(|| panic!("missing transparent listener for {hostname}"))
    }

    pub fn set_backend(&self, address: SocketAddr) {
        *self.backend.lock().expect("transparent backend lock") = address;
    }

    pub async fn stop(&mut self) {
        self.cancel.cancel();
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
        self.addresses.clear();
    }
}

pub async fn start(
    ca: LoadedCa,
    fake_ips: Arc<FakeIpMap>,
    backend: SocketAddr,
) -> Result<TransparentRuntime> {
    start_on_port(ca, fake_ips, backend, 443).await
}

pub async fn start_on_port(
    ca: LoadedCa,
    fake_ips: Arc<FakeIpMap>,
    backend: SocketAddr,
    port: u16,
) -> Result<TransparentRuntime> {
    let entries = fake_ips.entries();
    if entries.is_empty() {
        return Err(CursorError::Config(
            "transparent entry 需要至少一个 fake-IP".to_string(),
        ));
    }

    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let ca = Arc::new(ca);
    let backend = Arc::new(std::sync::Mutex::new(backend));
    let cancel = CancellationToken::new();
    let mut addresses = HashMap::new();
    let mut tasks = Vec::new();

    for (fake_ip, hostname) in entries {
        let listener = match TcpListener::bind(SocketAddr::from((fake_ip, port))).await {
            Ok(listener) => listener,
            Err(error) => {
                abort_startup(&cancel, &mut tasks).await;
                return Err(CursorError::Config(format!(
                    "绑定 fake-IP {fake_ip}:{port} 失败: {error}"
                )));
            }
        };
        let bound = match listener.local_addr() {
            Ok(bound) => bound,
            Err(error) => {
                abort_startup(&cancel, &mut tasks).await;
                return Err(CursorError::Io(error));
            }
        };
        if !bound.ip().is_loopback() {
            abort_startup(&cancel, &mut tasks).await;
            return Err(CursorError::Config(format!(
                "transparent entry 只允许 loopback: {bound}"
            )));
        }
        addresses.insert(hostname.to_ascii_lowercase(), bound);

        let acceptor = match server_config(fake_ip, fake_ips.clone(), ca.clone()) {
            Ok(config) => TlsAcceptor::from(Arc::new(config)),
            Err(error) => {
                abort_startup(&cancel, &mut tasks).await;
                return Err(error);
            }
        };
        let task_cancel = cancel.clone();
        let backend = backend.clone();
        tasks.push(tokio::spawn(async move {
            accept_loop(listener, acceptor, task_cancel, backend).await;
        }));
    }

    Ok(TransparentRuntime {
        addresses,
        backend,
        cancel,
        tasks,
    })
}

async fn abort_startup(cancel: &CancellationToken, tasks: &mut Vec<JoinHandle<()>>) {
    cancel.cancel();
    for task in tasks.drain(..) {
        let _ = task.await;
    }
}

fn server_config(
    fake_ip: Ipv4Addr,
    fake_ips: Arc<FakeIpMap>,
    ca: Arc<LoadedCa>,
) -> Result<ServerConfig> {
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(SniResolver {
            fake_ip,
            fake_ips,
            ca,
        }));
    config.alpn_protocols = vec![b"h2".to_vec()];
    Ok(config)
}

async fn accept_loop(
    listener: TcpListener,
    acceptor: TlsAcceptor,
    cancel: CancellationToken,
    backend: Arc<std::sync::Mutex<SocketAddr>>,
) {
    let listen_ip = listener
        .local_addr()
        .map(|address| address.ip().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            accepted = listener.accept() => {
                let Ok((stream, peer)) = accepted else {
                    continue;
                };
                let acceptor = acceptor.clone();
                let backend = backend.clone();
                let listen_ip = listen_ip.clone();
                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls) => serve_http2(tls, backend).await,
                        Err(error) => eprintln!(
                            "cursor_transparent_tls ip={listen_ip} peer={} error={error}",
                            peer.ip()
                        ),
                    }
                });
            }
        }
    }
}

async fn serve_http2(
    stream: tokio_rustls::server::TlsStream<TcpStream>,
    backend: Arc<std::sync::Mutex<SocketAddr>>,
) {
    let io = TokioIo::new(stream);
    let service = service_fn(move |request| {
        let backend = backend.clone();
        async move {
            let address = *backend.lock().expect("transparent backend lock");
            handle_request(request, address).await
        }
    });
    if let Err(error) = hyper::server::conn::http2::Builder::new(TokioExecutor::new())
        .serve_connection(io, service)
        .await
    {
        eprintln!("cursor_transparent_h2 error={error}");
    }
}

async fn handle_request(
    request: Request<Incoming>,
    backend: SocketAddr,
) -> std::result::Result<Response<ResponseBody>, Infallible> {
    Ok(route_and_forward(request, backend).await)
}

async fn route_and_forward(
    request: Request<Incoming>,
    backend: SocketAddr,
) -> Response<ResponseBody> {
    let host = request_host(&request);
    let path = request
        .uri()
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/")
        .to_string();
    let decision = classify_request(&host, &path, UnmatchedPathPolicy::Reject);
    eprintln!("cursor_transparent_req host={host} path={path} decision={decision:?}");
    match decision {
        RouteDecision::Local => forward_to_backend(request, backend, &path).await,
        RouteDecision::Reject | RouteDecision::Passthrough => static_response(
            StatusCode::NOT_FOUND,
            br#"{"error":{"code":"cursor_path_rejected"}}"#,
        ),
    }
}

async fn forward_to_backend(
    request: Request<Incoming>,
    backend: SocketAddr,
    path: &str,
) -> Response<ResponseBody> {
    let method = request.method().clone();
    let mut headers = request.headers().clone();
    retain_backend_headers(&mut headers);
    headers.remove(http::header::CONTENT_LENGTH);
    headers.remove(http::header::TRANSFER_ENCODING);
    if let Ok(value) = backend.to_string().parse() {
        headers.insert(HOST, value);
    }
    let tcp = match TcpStream::connect(backend).await {
        Ok(tcp) => tcp,
        Err(_) => {
            return static_response(
                StatusCode::SERVICE_UNAVAILABLE,
                br#"{"error":{"code":"cursor_backend_unavailable"}}"#,
            );
        }
    };
    let (mut sender, conn) = match http1::handshake(TokioIo::new(tcp)).await {
        Ok(pair) => pair,
        Err(_) => {
            return static_response(
                StatusCode::SERVICE_UNAVAILABLE,
                br#"{"error":{"code":"cursor_backend_unavailable"}}"#,
            );
        }
    };
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let mut builder = Request::builder()
        .method(method)
        .uri(format!("http://{backend}{path}"));
    for (name, value) in headers.iter() {
        builder = builder.header(name, value);
    }
    let outbound = match builder.body(request.into_body().map_err(std::io::Error::other)) {
        Ok(request) => request,
        Err(_) => {
            return static_response(
                StatusCode::BAD_REQUEST,
                br#"{"error":{"code":"cursor_path_invalid"}}"#,
            );
        }
    };
    match sender.send_request(outbound).await {
        Ok(response) => {
            let (parts, body) = response.into_parts();
            Response::from_parts(parts, body.map_err(std::io::Error::other).boxed())
        }
        Err(_) => static_response(
            StatusCode::SERVICE_UNAVAILABLE,
            br#"{"error":{"code":"cursor_backend_unavailable"}}"#,
        ),
    }
}

fn static_response(status: StatusCode, body: &'static [u8]) -> Response<ResponseBody> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(
            Full::new(Bytes::from_static(body))
                .map_err(|never| match never {})
                .boxed(),
        )
        .expect("static transparent response")
}

fn request_host(request: &Request<Incoming>) -> String {
    request
        .uri()
        .host()
        .map(str::to_owned)
        .or_else(|| {
            request
                .headers()
                .get(HOST)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

fn retain_backend_headers(headers: &mut http::HeaderMap) {
    const ALLOWED: [&str; 9] = [
        "accept",
        "content-encoding",
        "content-length",
        "content-type",
        "connect-content-encoding",
        "connect-protocol-version",
        "host",
        "user-agent",
        "x-request-id",
    ];
    let to_remove = headers
        .keys()
        .filter(|name| {
            !ALLOWED
                .iter()
                .any(|allowed| name.as_str().eq_ignore_ascii_case(allowed))
        })
        .cloned()
        .collect::<Vec<_>>();
    for name in to_remove {
        headers.remove(name);
    }
}

struct SniResolver {
    fake_ip: Ipv4Addr,
    fake_ips: Arc<FakeIpMap>,
    ca: Arc<LoadedCa>,
}

impl std::fmt::Debug for SniResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SniResolver")
            .field("fake_ip", &self.fake_ip)
            .finish_non_exhaustive()
    }
}

impl ResolvesServerCert for SniResolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        self.fake_ips.hostname(self.fake_ip)?;
        let Some(sni) = client_hello.server_name() else {
            eprintln!("cursor_transparent_sni ip={} missing", self.fake_ip);
            return None;
        };
        if !is_cursor_host(sni) {
            eprintln!(
                "cursor_transparent_sni ip={} got={sni} unmapped",
                self.fake_ip
            );
            return None;
        }
        let _ = ServerName::try_from(sni.to_string()).ok()?;
        sign_leaf(sni, &self.ca)
    }
}

fn sign_leaf(hostname: &str, ca: &LoadedCa) -> Option<Arc<CertifiedKey>> {
    let mut params = CertificateParams::new(vec![hostname.to_string()]).ok()?;
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, hostname);
    params.distinguished_name = distinguished_name;
    let key_pair = KeyPair::generate().ok()?;
    let cert = params.signed_by(&key_pair, &ca.issuer).ok()?;
    let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));
    let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key_der).ok()?;
    Some(Arc::new(CertifiedKey::new(vec![cert_der], signing_key)))
}
