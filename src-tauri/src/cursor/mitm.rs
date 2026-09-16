//! Local Cursor explicit proxy.
//!
//! This is intentionally a narrow front door. It only intercepts `*.cursor.sh` TLS and leaves
//! unrelated hosts untouched. Protocol decoding belongs to the later `cursor::protocol` layer.

use std::{net::SocketAddr, time::Duration};

use http::{header::HOST, uri::Authority};
use hudsucker::{
    certificate_authority::RcgenAuthority,
    hyper::{Request, Response, StatusCode, Uri},
    rustls::crypto::aws_lc_rs,
    Body, HttpContext, HttpHandler, Proxy, RequestOrResponse,
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

use super::{
    ca::LoadedCa,
    routes::{classify_request, RouteDecision, UnmatchedPathPolicy},
};
use crate::cursor::routes::is_cursor_host;

const ORIGINAL_URL_HEADER: &str = "x-cc2cx-cursor-original-url";

#[derive(Debug, Clone, Copy)]
pub struct CursorProxyConfig {
    pub requested_port: u16,
    pub backend: Option<SocketAddr>,
    pub unmatched_path_policy: UnmatchedPathPolicy,
}

impl Default for CursorProxyConfig {
    fn default() -> Self {
        Self {
            requested_port: 15722,
            backend: None,
            unmatched_path_policy: UnmatchedPathPolicy::Passthrough,
        }
    }
}

#[derive(Default)]
pub struct CursorProxyRuntime {
    url: Option<String>,
    port: Option<u16>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}

impl CursorProxyRuntime {
    pub fn running(&self) -> bool {
        self.task.as_ref().is_some_and(|task| !task.is_finished())
    }

    pub fn url(&self) -> Option<String> {
        self.running().then(|| self.url.clone()).flatten()
    }

    pub fn port(&self) -> Option<u16> {
        self.running().then_some(self.port).flatten()
    }

    pub async fn start(
        &mut self,
        ca: LoadedCa,
        config: CursorProxyConfig,
    ) -> Result<(String, u16), String> {
        if let Some(url) = self.url() {
            return Ok((url, self.port.unwrap_or_default()));
        }

        let listener = bind_listener(config.requested_port)
            .await
            .map_err(|error| format!("绑定 Cursor 本机代理失败: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("读取 Cursor 代理地址失败: {error}"))?;
        let (stop, done) = oneshot::channel();
        let authority = RcgenAuthority::new(ca.issuer, 1_000, aws_lc_rs::default_provider());
        let proxy = Proxy::builder()
            .with_listener(listener)
            .with_ca(authority)
            .with_rustls_connector(aws_lc_rs::default_provider())
            .with_http_handler(CursorRelay {
                backend: config.backend,
                unmatched_path_policy: config.unmatched_path_policy,
            })
            .with_graceful_shutdown(async move {
                let _ = done.await;
            })
            .build()
            .map_err(|error| format!("构建 Cursor 本机代理失败: {error}"))?;

        let url = format!("http://{address}");
        self.stop = Some(stop);
        self.url = Some(url.clone());
        self.port = Some(address.port());
        let task = tokio::spawn(async move {
            proxy
                .start()
                .await
                .map_err(|error| format!("Cursor 本机代理异常停止: {error}"))
        });
        // The listener is already bound before spawn. Yield once so an immediately failing
        // proxy task is reported to the harness before it writes Cursor settings.
        tokio::task::yield_now().await;
        if task.is_finished() {
            let error = match task.await {
                Ok(Err(error)) => error,
                Ok(Ok(())) => "Cursor 本机代理意外停止".to_string(),
                Err(error) => format!("Cursor 本机代理任务失败: {error}"),
            };
            self.stop = None;
            self.url = None;
            self.port = None;
            return Err(error);
        }
        self.task = Some(task);
        Ok((url, address.port()))
    }

    pub async fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(mut task) = self.task.take() {
            if tokio::time::timeout(Duration::from_secs(5), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
        self.url = None;
        self.port = None;
    }
}

async fn bind_listener(requested_port: u16) -> std::io::Result<TcpListener> {
    let requested = SocketAddr::from(([127, 0, 0, 1], requested_port));
    match TcpListener::bind(requested).await {
        Ok(listener) => Ok(listener),
        Err(error) if requested_port != 0 => {
            log::warn!("Cursor 代理端口 {requested} 不可用，改用随机端口: {error}");
            TcpListener::bind(("127.0.0.1", 0)).await
        }
        Err(error) => Err(error),
    }
}

#[derive(Clone)]
struct CursorRelay {
    backend: Option<SocketAddr>,
    unmatched_path_policy: UnmatchedPathPolicy,
}

impl HttpHandler for CursorRelay {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        request: Request<Body>,
    ) -> RequestOrResponse {
        self.rewrite_request(request).await
    }

    async fn should_intercept_connect(
        &mut self,
        _ctx: &HttpContext,
        request: &Request<Body>,
    ) -> bool {
        request
            .uri()
            .authority()
            .is_some_and(|authority| is_cursor_host(authority.host()))
    }

    async fn should_intercept_tls(
        &mut self,
        _ctx: &HttpContext,
        hello: hudsucker::rustls::server::ClientHello<'_>,
    ) -> bool {
        hello.server_name().is_some_and(is_cursor_host)
    }
}

impl CursorRelay {
    async fn rewrite_request(&mut self, mut request: Request<Body>) -> RequestOrResponse {
        let original = request.uri().clone();
        let host = request_host(&request).unwrap_or_default();
        let path = original
            .path_and_query()
            .map(|value| value.as_str())
            .unwrap_or("/");
        let decision = classify_request(&host, path, self.unmatched_path_policy);
        match decision {
            RouteDecision::Passthrough => return request.into(),
            RouteDecision::Reject => {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::empty())
                    .expect("static reject response must be valid")
                    .into();
            }
            RouteDecision::Local => {}
        }
        let Some(backend) = self.backend else {
            return Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"error":{"code":"cursor_backend_unavailable","message":"Cursor 本地 backend 未配置"}}"#,
                ))
                .expect("static backend-unavailable response must be valid")
                .into();
        };
        if let Some(original_host) = request.headers().get(HOST).cloned().or_else(|| {
            original
                .authority()
                .and_then(|authority| authority.as_str().parse().ok())
        }) {
            request.headers_mut().insert(HOST, original_host);
        }
        retain_backend_headers(request.headers_mut());
        if let Ok(value) = original.to_string().parse() {
            request.headers_mut().insert(ORIGINAL_URL_HEADER, value);
        }
        if let Ok(uri) = format!("http://{backend}{path}").parse::<Uri>() {
            *request.uri_mut() = uri;
        }
        request.into()
    }
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

fn request_host(request: &Request<Body>) -> Option<String> {
    let raw = request.uri().host().map(str::to_owned).or_else(|| {
        request
            .headers()
            .get(HOST)?
            .to_str()
            .ok()
            .map(str::to_owned)
    })?;
    raw.parse::<Authority>()
        .map(|authority| authority.host().to_owned())
        .ok()
        .or(Some(raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn requested_port_falls_back_when_occupied() {
        let occupied = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = occupied.local_addr().unwrap().port();
        let listener = bind_listener(port).await.unwrap();
        assert_ne!(listener.local_addr().unwrap().port(), port);
    }

    #[test]
    fn decrypted_origin_form_uses_host_header_for_cursor_routing() {
        let request = Request::builder()
            .uri("/agent.v1.AgentService/RunSSE")
            .header(HOST, "API2.CURSOR.SH")
            .body(Body::empty())
            .unwrap();

        assert_eq!(request_host(&request).as_deref(), Some("API2.CURSOR.SH"));
        assert_eq!(
            classify_request(
                request_host(&request).as_deref().unwrap(),
                request.uri().path_and_query().unwrap().as_str(),
                UnmatchedPathPolicy::Passthrough,
            ),
            RouteDecision::Local
        );
    }

    #[test]
    fn host_header_port_is_removed_before_cursor_matching() {
        let request = Request::builder()
            .uri("/agent.v1.AgentService/RunSSE")
            .header(HOST, "api2.cursor.sh:443")
            .body(Body::empty())
            .unwrap();

        assert_eq!(request_host(&request).as_deref(), Some("api2.cursor.sh"));
    }

    #[tokio::test]
    async fn backend_rewrite_preserves_original_host_header() {
        let mut relay = CursorRelay {
            backend: Some("127.0.0.1:18080".parse().unwrap()),
            unmatched_path_policy: UnmatchedPathPolicy::Passthrough,
        };
        let request = Request::builder()
            .uri("https://api2.cursor.sh/agent.v1.AgentService/RunSSE")
            .body(Body::empty())
            .unwrap();

        let rewritten = relay.rewrite_request(request).await;
        let RequestOrResponse::Request(request) = rewritten else {
            panic!("relay should return a request");
        };
        assert_eq!(request.headers().get(HOST).unwrap(), "api2.cursor.sh");
        assert_eq!(request.uri().host(), Some("127.0.0.1"));
    }

    #[tokio::test]
    async fn backend_rewrite_strips_credentials_and_hop_by_hop_headers() {
        let mut relay = CursorRelay {
            backend: Some("127.0.0.1:18080".parse().unwrap()),
            unmatched_path_policy: UnmatchedPathPolicy::Passthrough,
        };
        let request = Request::builder()
            .uri("https://api2.cursor.sh/agent.v1.AgentService/RunSSE")
            .header("content-type", "application/connect+proto")
            .header("content-encoding", "gzip")
            .header("authorization", "Bearer fixture-secret")
            .header("cookie", "session=fixture")
            .header("proxy-authorization", "Basic fixture")
            .header("connection", "keep-alive")
            .header("upgrade", "websocket")
            .header("x-untrusted", "must-not-forward")
            .body(Body::empty())
            .unwrap();

        let rewritten = relay.rewrite_request(request).await;
        let RequestOrResponse::Request(request) = rewritten else {
            panic!("relay should return a request");
        };
        assert_eq!(
            request.headers().get("content-type").unwrap(),
            "application/connect+proto"
        );
        assert_eq!(request.headers().get("content-encoding").unwrap(), "gzip");
        for header in [
            "authorization",
            "cookie",
            "proxy-authorization",
            "connection",
            "upgrade",
            "x-untrusted",
        ] {
            assert!(
                request.headers().get(header).is_none(),
                "header {header} must not reach the local backend"
            );
        }
    }

    #[tokio::test]
    async fn reject_policy_returns_not_found_instead_of_forwarding() {
        let mut relay = CursorRelay {
            backend: Some("127.0.0.1:18080".parse().unwrap()),
            unmatched_path_policy: UnmatchedPathPolicy::Reject,
        };
        let request = Request::builder()
            .uri("https://api2.cursor.sh/not-allowlisted")
            .body(Body::empty())
            .unwrap();

        let result = relay.rewrite_request(request).await;
        let RequestOrResponse::Response(response) = result else {
            panic!("reject policy must return a response");
        };
        assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
    }
}
