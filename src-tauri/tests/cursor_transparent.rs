use axum::{body::Body, extract::Request, http::HeaderMap, routing::post, Router};
use bytes::Bytes;
use cc_launch_lib::cursor::{
    ca::CaManager,
    fake_ip::FakeIpMap,
    transparent::{start_on_port, TransparentRuntime},
};
use futures::channel::mpsc;
use http_body::Frame;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::client::conn::http2;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::{
    pki_types::{pem::PemObject, CertificateDer, ServerName},
    ClientConfig, RootCertStore,
};
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsConnector;

async fn start_test_runtime() -> Result<(TransparentRuntime, String, Arc<FakeIpMap>), String> {
    start_test_runtime_with_backend("127.0.0.1:1".parse().unwrap()).await
}

async fn start_test_runtime_with_backend(
    backend: SocketAddr,
) -> Result<(TransparentRuntime, String, Arc<FakeIpMap>), String> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let manager = CaManager::new(dir.path().to_path_buf());
    manager.initialize().map_err(|error| error.to_string())?;
    let ca = manager.load().map_err(|error| error.to_string())?;
    let pem = manager
        .certificate_pem()
        .map_err(|error| error.to_string())?;
    let fake_ips = Arc::new(
        FakeIpMap::allocate(&["api2.cursor.sh".into(), "api3.cursor.sh".into()])
            .map_err(|error| error.to_string())?,
    );
    let runtime = start_on_port(ca, fake_ips.clone(), backend, 0)
        .await
        .map_err(|error| error.to_string())?;
    drop(dir);
    Ok((runtime, pem, fake_ips))
}

async fn tls_connect(address: SocketAddr, sni: &str, ca_pem: &str) -> Result<(), String> {
    tls_connect_with_alpn(address, sni, ca_pem, b"h2").await
}

async fn tls_connect_with_alpn(
    address: SocketAddr,
    sni: &str,
    ca_pem: &str,
    alpn: &[u8],
) -> Result<(), String> {
    let mut roots = RootCertStore::empty();
    let cert = CertificateDer::from_pem_slice(ca_pem.as_bytes())
        .map_err(|error| format!("SNI CA parse failed: {error}"))?;
    roots
        .add(cert)
        .map_err(|error| format!("SNI CA trust failed: {error}"))?;
    let mut config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![alpn.to_vec()];
    let connector = TlsConnector::from(Arc::new(config));
    let stream = TcpStream::connect(address)
        .await
        .map_err(|error| format!("SNI TCP connect failed: {error}"))?;
    let name =
        ServerName::try_from(sni.to_string()).map_err(|error| format!("SNI invalid: {error}"))?;
    connector
        .connect(name, stream)
        .await
        .map(|_| ())
        .map_err(|error| format!("SNI handshake rejected: {error}"))
}

#[tokio::test]
async fn transparent_entry_rejects_unknown_sni() {
    let (mut runtime, ca, _fake_ips) = start_test_runtime().await.unwrap();
    let result = tls_connect(runtime.address_for("api2.cursor.sh"), "evil.example", &ca).await;
    assert!(result.unwrap_err().to_string().contains("SNI"));
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_accepts_mapped_cname_sni_on_another_fake_ip() {
    let (mut runtime, ca, _fake_ips) = start_test_runtime().await.unwrap();
    tls_connect(
        runtime.address_for("api2.cursor.sh"),
        "api2direct.cursor.sh",
        &ca,
    )
    .await
    .expect("CNAME SNI on a mapped fake-IP must complete TLS");
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_shuts_down_without_leaking_tasks() {
    let (mut runtime, _ca, _fake_ips) = start_test_runtime().await.unwrap();
    let address = runtime.address_for("api2.cursor.sh");
    runtime.stop().await;
    assert!(TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn transparent_entry_binds_loopback_fake_ips_only() {
    let (mut runtime, _ca, fake_ips) = start_test_runtime().await.unwrap();
    let api2 = runtime.address_for("api2.cursor.sh");
    let api3 = runtime.address_for("api3.cursor.sh");
    assert_eq!(api2.ip(), fake_ips.address("api2.cursor.sh").unwrap());
    assert_eq!(api3.ip(), fake_ips.address("api3.cursor.sh").unwrap());
    assert_ne!(api2, api3);
    assert!(api2.ip().is_loopback());
    assert!(api3.ip().is_loopback());
    runtime.stop().await;
}

async fn h2_request(
    address: SocketAddr,
    sni: &str,
    ca_pem: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: &[u8],
) -> Result<(u16, Vec<u8>, HeaderMap), String> {
    let mut roots = RootCertStore::empty();
    let cert =
        CertificateDer::from_pem_slice(ca_pem.as_bytes()).map_err(|error| error.to_string())?;
    roots.add(cert).map_err(|error| error.to_string())?;
    let mut config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let connector = TlsConnector::from(Arc::new(config));
    let stream = TcpStream::connect(address)
        .await
        .map_err(|error| error.to_string())?;
    let name = ServerName::try_from(sni.to_string()).map_err(|error| error.to_string())?;
    let tls = connector
        .connect(name, stream)
        .await
        .map_err(|error| error.to_string())?;
    let (mut sender, conn) = http2::Builder::new(TokioExecutor::new())
        .handshake(TokioIo::new(tls))
        .await
        .map_err(|error| error.to_string())?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let mut builder = hyper::Request::builder()
        .method("POST")
        .uri(format!("https://{sni}{path}"))
        .header("host", sni);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder
        .body(Full::new(Bytes::copy_from_slice(body)))
        .map_err(|error| error.to_string())?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|error| error.to_string())?
        .to_bytes()
        .to_vec();
    Ok((status, body, headers))
}

async fn h2_request_body<B>(
    address: SocketAddr,
    sni: &str,
    ca_pem: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: B,
) -> Result<(u16, Vec<u8>, HeaderMap), String>
where
    B: http_body::Body<Data = Bytes> + Send + Unpin + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let mut roots = RootCertStore::empty();
    let cert =
        CertificateDer::from_pem_slice(ca_pem.as_bytes()).map_err(|error| error.to_string())?;
    roots.add(cert).map_err(|error| error.to_string())?;
    let mut config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    let connector = TlsConnector::from(Arc::new(config));
    let stream = TcpStream::connect(address)
        .await
        .map_err(|error| error.to_string())?;
    let name = ServerName::try_from(sni.to_string()).map_err(|error| error.to_string())?;
    let tls = connector
        .connect(name, stream)
        .await
        .map_err(|error| error.to_string())?;
    let (mut sender, conn) = http2::Builder::new(TokioExecutor::new())
        .handshake(TokioIo::new(tls))
        .await
        .map_err(|error| error.to_string())?;
    tokio::spawn(async move {
        let _ = conn.await;
    });
    let mut builder = hyper::Request::builder()
        .method("POST")
        .uri(format!("https://{sni}{path}"))
        .header("host", sni);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = builder.body(body).map_err(|error| error.to_string())?;
    let response = sender
        .send_request(request)
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .map_err(|error| error.to_string())?
        .to_bytes()
        .to_vec();
    Ok((status, body, headers))
}

struct RecordingBackend {
    address: SocketAddr,
    hits: Arc<Mutex<Vec<(String, bool)>>>,
    encodings: Arc<Mutex<Vec<Option<String>>>>,
}

async fn start_recording_backend() -> RecordingBackend {
    let hits = Arc::new(Mutex::new(Vec::new()));
    let encodings = Arc::new(Mutex::new(Vec::new()));
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("backend bind");
    let address = listener.local_addr().expect("backend address");
    let recorded_sse = hits.clone();
    let recorded_run = hits.clone();
    let encodings_sse = encodings.clone();
    let encodings_run = encodings.clone();
    let router = Router::new()
        .route(
            "/agent.v1.AgentService/RunSSE",
            post(move |request: Request<Body>| {
                let recorded_sse = recorded_sse.clone();
                let encodings_sse = encodings_sse.clone();
                async move {
                    let authorized = request.headers().get("authorization").is_some();
                    let encoding = request
                        .headers()
                        .get("content-encoding")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    recorded_sse
                        .lock()
                        .expect("hits")
                        .push((request.uri().path().to_string(), authorized));
                    encodings_sse.lock().expect("encodings").push(encoding);
                    "backend-ok"
                }
            }),
        )
        .route(
            "/agent.v1.AgentService/Run",
            post(move |request: Request<Body>| {
                let recorded_run = recorded_run.clone();
                let encodings_run = encodings_run.clone();
                async move {
                    let authorized = request.headers().get("authorization").is_some();
                    let encoding = request
                        .headers()
                        .get("content-encoding")
                        .and_then(|value| value.to_str().ok())
                        .map(str::to_owned);
                    recorded_run
                        .lock()
                        .expect("hits")
                        .push((request.uri().path().to_string(), authorized));
                    encodings_run.lock().expect("encodings").push(encoding);
                    "backend-ok"
                }
            }),
        );
    tokio::spawn(async move {
        axum::serve(listener, router).await.ok();
    });
    RecordingBackend {
        address,
        hits,
        encodings,
    }
}

#[tokio::test]
async fn transparent_entry_forwards_local_http2_path_to_backend() {
    let backend = start_recording_backend().await;
    let (mut runtime, ca, _fake_ips) = start_test_runtime_with_backend(backend.address)
        .await
        .unwrap();
    let (status, body, _headers) = h2_request(
        runtime.address_for("api2.cursor.sh"),
        "api2.cursor.sh",
        &ca,
        "/agent.v1.AgentService/RunSSE",
        &[
            ("content-type", "application/connect+proto"),
            ("authorization", "Bearer secret-must-not-forward"),
        ],
        b"frame",
    )
    .await
    .expect("http2 request");
    assert_eq!(status, 200);
    assert_eq!(body, b"backend-ok");
    let hits = backend.hits.lock().expect("hits").clone();
    assert_eq!(hits, vec![("/agent.v1.AgentService/RunSSE".into(), false)]);
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_forwards_content_encoding_not_authorization() {
    let backend = start_recording_backend().await;
    let (mut runtime, ca, _fake_ips) = start_test_runtime_with_backend(backend.address)
        .await
        .unwrap();
    let (status, body, _headers) = h2_request(
        runtime.address_for("api2.cursor.sh"),
        "api2.cursor.sh",
        &ca,
        "/agent.v1.AgentService/RunSSE",
        &[
            ("content-type", "application/connect+proto"),
            ("content-encoding", "gzip"),
            ("authorization", "Bearer secret-must-not-forward"),
        ],
        b"frame",
    )
    .await
    .expect("http2 request");
    assert_eq!(status, 200);
    assert_eq!(body, b"backend-ok");
    assert_eq!(
        backend.encodings.lock().expect("encodings").clone(),
        vec![Some("gzip".into())]
    );
    let hits = backend.hits.lock().expect("hits").clone();
    assert_eq!(hits, vec![("/agent.v1.AgentService/RunSSE".into(), false)]);
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_does_not_buffer_open_run_body() {
    let backend = start_recording_backend().await;
    let (mut runtime, ca, _fake_ips) = start_test_runtime_with_backend(backend.address)
        .await
        .unwrap();
    let (tx, rx) = mpsc::channel::<Result<Frame<Bytes>, std::io::Error>>(1);
    let mut tx = tx;
    tx.try_send(Ok(Frame::data(Bytes::from_static(b"partial"))))
        .expect("send first body chunk");
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        h2_request_body(
            runtime.address_for("api2.cursor.sh"),
            "api2.cursor.sh",
            &ca,
            "/agent.v1.AgentService/Run",
            &[("content-type", "application/connect+proto")],
            StreamBody::new(rx),
        ),
    )
    .await;
    drop(tx);
    let (status, body, _headers) = result
        .expect("Run must not wait for the client to close the request body")
        .expect("http2 streaming request");
    assert_eq!(status, 200);
    assert_eq!(body, b"backend-ok");
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_can_retarget_backend_after_listen() {
    let first = start_recording_backend().await;
    let second = start_recording_backend().await;
    let (mut runtime, ca, _fake_ips) = start_test_runtime_with_backend(first.address)
        .await
        .unwrap();
    runtime.set_backend(second.address);
    let (status, body, _headers) = h2_request(
        runtime.address_for("api2.cursor.sh"),
        "api2.cursor.sh",
        &ca,
        "/agent.v1.AgentService/RunSSE",
        &[("content-type", "application/connect+proto")],
        b"frame",
    )
    .await
    .expect("http2 request after retarget");
    assert_eq!(status, 200);
    assert_eq!(body, b"backend-ok");
    assert!(first.hits.lock().expect("first").is_empty());
    assert_eq!(
        second.hits.lock().expect("second").clone(),
        vec![("/agent.v1.AgentService/RunSSE".into(), false)]
    );
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_rejects_unmatched_path_without_hitting_backend() {
    let backend = start_recording_backend().await;
    let (mut runtime, ca, _fake_ips) = start_test_runtime_with_backend(backend.address)
        .await
        .unwrap();
    let (status, _body, _headers) = h2_request(
        runtime.address_for("api2.cursor.sh"),
        "api2.cursor.sh",
        &ca,
        "/not-a-cursor-agent-path",
        &[],
        b"",
    )
    .await
    .expect("http2 reject");
    assert_eq!(status, 404);
    assert!(backend.hits.lock().expect("hits").is_empty());
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_accepts_many_parallel_handshakes() {
    let (mut runtime, ca, _fake_ips) = start_test_runtime().await.unwrap();
    let address = runtime.address_for("api2.cursor.sh");
    let mut joins = Vec::new();
    for _ in 0..32 {
        let ca = ca.clone();
        joins.push(tokio::spawn(async move {
            tls_connect(address, "api2.cursor.sh", &ca).await
        }));
    }
    for join in joins {
        join.await
            .expect("join handshake task")
            .expect("parallel handshake must succeed");
    }
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_rejects_unsupported_alpn() {
    let (mut runtime, ca, _fake_ips) = start_test_runtime().await.unwrap();
    let result = tls_connect_with_alpn(
        runtime.address_for("api2.cursor.sh"),
        "api2.cursor.sh",
        &ca,
        b"http/1.1",
    )
    .await;
    assert!(result.is_err());
    runtime.stop().await;
}

#[tokio::test]
async fn transparent_entry_cleans_up_listeners_when_a_later_fake_ip_is_busy() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let dir = tempfile::tempdir().expect("temporary CA directory");
    let manager = CaManager::new(dir.path().to_path_buf());
    manager.initialize().expect("generate CA");
    let ca = manager.load().expect("load CA");
    let fake_ips = Arc::new(
        FakeIpMap::allocate(&["api2.cursor.sh".into(), "api3.cursor.sh".into()])
            .expect("allocate fake IPs"),
    );
    let port_probe = TcpListener::bind((fake_ips.address("api3.cursor.sh").unwrap(), 0))
        .await
        .expect("reserve later fake IP");
    let port = port_probe.local_addr().unwrap().port();

    let result = start_on_port(ca, fake_ips.clone(), "127.0.0.1:1".parse().unwrap(), port).await;
    assert!(result.is_err(), "the second fake-IP is intentionally busy");
    drop(port_probe);

    let first_address = (fake_ips.address("api2.cursor.sh").unwrap(), port);
    let listener = TcpListener::bind(first_address)
        .await
        .expect("a failed start must release listeners already bound");
    drop(listener);
}
