//! Minimal local Cursor protocol backend for the P2-1 synthetic stream.
//!
//! The backend is deliberately independent from the existing JSON/Provider proxy. It only
//! understands the two protocol paths needed to prove the first end-to-end Cursor stream:
//! `BidiAppend` publishes a normalized request into the transport registry and `RunSSE` replays
//! that transport. Real provider selection, authentication, tools, and network routing remain
//! outside this slice.

use std::{
    convert::Infallible,
    io::{Read, Write},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, Request, Response, StatusCode},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use bytes::BytesMut;
use futures::StreamExt;
use prost::Message;
use serde::Serialize;
use tokio::{
    net::TcpListener,
    sync::{oneshot, Notify},
    task::JoinHandle,
};

use super::{
    adapter::{
        run_provider_stream, run_synthetic_stream, CursorOutputEvent, CursorRequest,
        ProviderInvocation, ProviderStreamError, ProviderStreamOutcome,
    },
    protocol::{
        bidi::decode_append,
        bidi::DecodedAppend,
        connect::{
            self, decode_unary, encode_message, ConnectCode, ConnectStreamError, END_STREAM_FLAG,
        },
        proto::{agent::v1 as agent, aiserver::v1 as ai},
        run_sse,
    },
    provider::CursorProvider,
    transport::{TransportError, TransportHandle, TransportRegistry},
};

const MAX_REQUEST_BODY_BYTES: usize = 8 * 1024 * 1024;
const RUN_SSE_ROUTE_WAIT: Duration = Duration::from_secs(2);

#[derive(Clone, Default)]
pub struct CursorProtocolBackend {
    registry: TransportRegistry,
    route_changed: std::sync::Arc<Notify>,
    provider: Option<Arc<dyn CursorProvider>>,
    health: Arc<ProviderHealthState>,
}

#[derive(Clone)]
struct BackendState {
    registry: TransportRegistry,
    route_changed: std::sync::Arc<Notify>,
    provider: Option<Arc<dyn CursorProvider>>,
    health: Arc<ProviderHealthState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorBackendHealth {
    NotRequired,
    NotObserved,
    Healthy,
    AuthFailed,
    Timeout,
    TransportFailed,
    ProtocolFailed,
    Failed,
}

impl CursorBackendHealth {
    pub(crate) fn is_failure(self) -> bool {
        matches!(
            self,
            Self::AuthFailed
                | Self::Timeout
                | Self::TransportFailed
                | Self::ProtocolFailed
                | Self::Failed
        )
    }
}

#[derive(Debug)]
struct ProviderHealthState {
    status: std::sync::Mutex<CursorBackendHealth>,
    last_error_code: std::sync::Mutex<Option<String>>,
}

impl Default for ProviderHealthState {
    fn default() -> Self {
        Self::new(false)
    }
}

impl ProviderHealthState {
    fn new(provider_configured: bool) -> Self {
        Self {
            status: std::sync::Mutex::new(if provider_configured {
                CursorBackendHealth::NotObserved
            } else {
                CursorBackendHealth::NotRequired
            }),
            last_error_code: std::sync::Mutex::new(None),
        }
    }

    fn record_error(&self, code: &str) {
        let status = match code {
            "provider_auth" => CursorBackendHealth::AuthFailed,
            "provider_timeout" => CursorBackendHealth::Timeout,
            "provider_transport" => CursorBackendHealth::TransportFailed,
            "provider_protocol" => CursorBackendHealth::ProtocolFailed,
            _ => CursorBackendHealth::Failed,
        };
        *self.status.lock().expect("provider health mutex poisoned") = status;
        *self
            .last_error_code
            .lock()
            .expect("provider health mutex poisoned") = Some(code.to_owned());
    }

    fn record_success(&self) {
        *self.status.lock().expect("provider health mutex poisoned") = CursorBackendHealth::Healthy;
        *self
            .last_error_code
            .lock()
            .expect("provider health mutex poisoned") = None;
    }

    fn record_outcome(&self, outcome: ProviderStreamOutcome) {
        match outcome {
            ProviderStreamOutcome::Completed => self.record_success(),
            ProviderStreamOutcome::IdleTimeout => self.record_error("provider_timeout"),
            ProviderStreamOutcome::Truncated => self.record_error("provider_protocol"),
            ProviderStreamOutcome::Failed => {
                let already_classified = self
                    .status
                    .lock()
                    .expect("provider health mutex poisoned")
                    .is_failure();
                if !already_classified {
                    self.record_error("provider_failed");
                }
            }
            ProviderStreamOutcome::Cancelled => {}
        }
    }

    fn snapshot(&self) -> (CursorBackendHealth, Option<String>) {
        (
            *self.status.lock().expect("provider health mutex poisoned"),
            self.last_error_code
                .lock()
                .expect("provider health mutex poisoned")
                .clone(),
        )
    }
}

pub struct CursorProtocolBackendRuntime {
    address: SocketAddr,
    health: Arc<ProviderHealthState>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), String>>>,
}

impl CursorProtocolBackend {
    pub fn new() -> Self {
        Self {
            registry: TransportRegistry::new(),
            route_changed: std::sync::Arc::new(Notify::new()),
            provider: None,
            health: Arc::new(ProviderHealthState::new(false)),
        }
    }

    /// Build a backend that forwards normalized requests to an explicitly supplied provider.
    ///
    /// The default constructor intentionally remains synthetic so the production harness cannot
    /// acquire network credentials or upstream behavior by accident.
    pub fn with_provider(provider: Arc<dyn CursorProvider>) -> Self {
        Self {
            registry: TransportRegistry::new(),
            route_changed: std::sync::Arc::new(Notify::new()),
            provider: Some(provider),
            health: Arc::new(ProviderHealthState::new(true)),
        }
    }

    /// Build an explicitly configured backend from the database's effective current provider.
    ///
    /// This is an opt-in constructor for integration tests and future harness wiring. The
    /// default `new()` constructor remains synthetic and does not read credentials or network
    /// configuration.
    pub fn with_current_provider(
        database: &crate::database::Database,
        app_type: &crate::app_config::AppType,
    ) -> super::error::Result<Self> {
        let provider =
            super::provider_factory::CursorProviderFactory::from_current(database, app_type)?;
        Ok(Self::with_provider(provider))
    }

    pub fn registry(&self) -> TransportRegistry {
        self.registry.clone()
    }

    pub fn router(&self) -> Router {
        let state = BackendState {
            registry: self.registry.clone(),
            route_changed: self.route_changed.clone(),
            provider: self.provider.clone(),
            health: self.health.clone(),
        };
        Router::new()
            .route("/health", get(handle_health))
            .route(
                "/aiserver.v1.BidiService/BidiAppend",
                post(handle_bidi_append),
            )
            .route("/agent.v1.AgentService/Run", post(handle_run))
            .route("/agent.v1.AgentService/RunSSE", post(handle_run_sse))
            .route(
                "/agent.v1.AgentService/GetNewChatNudgeParameterizedModelPicker",
                post(handle_empty_connect),
            )
            .route(
                "/aiserver.v1.NetworkService/IsConnected",
                post(handle_is_connected),
            )
            .route(
                "/aiserver.v1.AiService/AvailableModels",
                post(handle_available_models),
            )
            .route(
                "/agent.v1.AgentService/GetUsableModels",
                post(handle_get_usable_models),
            )
            .route(
                "/aiserver.v1.AiService/GetUsableModels",
                post(handle_get_usable_models),
            )
            .route(
                "/aiserver.v1.AiService/GetDefaultModel",
                post(handle_get_default_model),
            )
            .route(
                "/agent.v1.AgentService/GetDefaultModelForCli",
                post(handle_get_default_model_for_cli),
            )
            .route(
                "/aiserver.v1.AiService/GetDefaultModelForCli",
                post(handle_get_default_model_for_cli),
            )
            .route(
                "/aiserver.v1.ServerConfigService/GetServerConfig",
                post(handle_get_server_config),
            )
            .route(
                "/aiserver.v1.AiService/GetServerConfig",
                post(handle_get_server_config),
            )
            .with_state(state)
    }

    /// Run the provider's bodyless startup probe before any local listener or Cursor setting is
    /// committed. Synthetic backends have no upstream and therefore pass automatically.
    pub async fn preflight(&self) -> Result<(), ProviderStreamError> {
        match self.provider.as_ref() {
            Some(provider) => provider.preflight().await,
            None => Ok(()),
        }
    }

    /// Start the synthetic backend on an explicitly requested loopback address.
    ///
    /// This listener is intentionally separate from the Cursor MITM front door. A standalone
    /// backend is started only by an explicit caller; the database-backed application harness
    /// also uses this API after resolving the current provider and passes the actual address to
    /// the MITM runtime.
    pub async fn start(
        self,
        requested_address: SocketAddr,
    ) -> Result<CursorProtocolBackendRuntime, String> {
        if !requested_address.ip().is_loopback() {
            return Err("Cursor 合成 backend 仅允许绑定 loopback 地址".to_string());
        }
        let listener = TcpListener::bind(requested_address)
            .await
            .map_err(|error| format!("绑定 Cursor 合成 backend 失败: {error}"))?;
        let address = listener
            .local_addr()
            .map_err(|error| format!("读取 Cursor 合成 backend 地址失败: {error}"))?;
        let (stop, done) = oneshot::channel();
        let router = self.router();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = done.await;
                })
                .await
                .map_err(|error| format!("Cursor 合成 backend 异常停止: {error}"))
        });
        tokio::task::yield_now().await;
        if task.is_finished() {
            let error = match task.await {
                Ok(Err(error)) => error,
                Ok(Ok(())) => "Cursor 合成 backend 意外停止".to_string(),
                Err(error) => format!("Cursor 合成 backend 任务失败: {error}"),
            };
            return Err(error);
        }
        Ok(CursorProtocolBackendRuntime {
            address,
            health: self.health,
            stop: Some(stop),
            task: Some(task),
        })
    }
}

async fn handle_health(State(state): State<BackendState>) -> Response<Body> {
    let (status, provider) = if state.provider.is_some() {
        let (health, last_error_code) = state.health.snapshot();
        (
            if health.is_failure() {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::OK
            },
            serde_json::json!({
                "mode": "configured",
                "authentication": "configured",
                "upstream_probe": if health == CursorBackendHealth::NotObserved { "not_run" } else { "request_observed" },
                "health": health,
                "last_error_code": last_error_code,
            }),
        )
    } else {
        (
            StatusCode::OK,
            serde_json::json!({
                "mode": "synthetic",
                "authentication": "not_required",
                "upstream_probe": "not_run",
                "health": CursorBackendHealth::NotRequired,
            }),
        )
    };
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "status": if status == StatusCode::OK { "ok" } else { "degraded" },
                "protocol": "cursor-connect-subset-v1",
                "provider": provider,
                "capabilities": {
                    "single_turn_stream": true,
                    "cancellation": true,
                    "history": false,
                    "images": false,
                    "mcp_tools": false,
                    "reasoning": false,
                    "tool_execution": false
                }
            })
            .to_string(),
        ))
        .expect("static health response must be valid")
}

impl CursorProtocolBackendRuntime {
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    pub fn running(&self) -> bool {
        self.task.as_ref().is_some_and(|task| !task.is_finished())
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub fn health(&self) -> (CursorBackendHealth, Option<String>) {
        self.health.snapshot()
    }

    pub async fn stop(&mut self) -> Result<(), String> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(mut task) = self.task.take() {
            match tokio::time::timeout(Duration::from_secs(5), &mut task).await {
                Ok(Ok(Ok(()))) => {}
                Ok(Ok(Err(error))) => return Err(error),
                Ok(Err(error)) => return Err(format!("Cursor 合成 backend 任务失败: {error}")),
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    return Err("停止 Cursor 合成 backend 超时".to_string());
                }
            }
        }
        Ok(())
    }
}

async fn handle_bidi_append(
    State(state): State<BackendState>,
    request: Request<Body>,
) -> Response<Body> {
    let registry = &state.registry;
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::PAYLOAD_TOO_LARGE, error.to_string()),
    };
    let body = match decompress_complete(&parts.headers, body) {
        Ok(body) => body,
        Err((status, message)) => return protocol_error(status, message),
    };
    let append: ai::BidiAppendRequest = match decode_unary(&body) {
        Ok(append) => append,
        Err(error) => return protocol_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let decoded = match decode_append(&append) {
        Ok(decoded) => decoded,
        Err(error) => return protocol_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let request_id = decoded.request_id.clone();
    let seqno = decoded.seqno;
    if seqno != 0 {
        return transport_error(TransportError::Sequence {
            expected: 0,
            received: seqno,
        });
    }
    let payload = Bytes::from(decoded.message.encode_to_vec());

    let invocation = match CursorRequest::from_decoded_append(decoded) {
        Ok(request) => {
            if !request.field_dispositions.is_empty() {
                return protocol_error(
                    StatusCode::NOT_IMPLEMENTED,
                    format!(
                        "Cursor request fields are unsupported: {:?}",
                        request.field_dispositions
                    ),
                );
            }
            match ProviderInvocation::from_request(request) {
                Ok(invocation) => invocation,
                Err(error) => {
                    return protocol_error(StatusCode::BAD_REQUEST, error.reason.to_string())
                }
            }
        }
        Err(error) => {
            return protocol_error(
                StatusCode::NOT_IMPLEMENTED,
                format!("Cursor action is unsupported: {:?}", error.unsupported),
            )
        }
    };
    if registry.current(&request_id).is_some_and(|transport| {
        transport.is_started() || transport.is_terminal() || transport.is_disconnected()
    }) {
        return transport_error(TransportError::Closed);
    }
    let transport = match registry.get_or_create_for_append(&request_id) {
        Ok(transport) => transport,
        Err(error) => return transport_error(error),
    };
    let ready = match transport.append(seqno, payload) {
        Ok(ready) => ready,
        Err(error) => return transport_error(error),
    };
    state.route_changed.notify_waiters();

    if !ready.is_empty() {
        spawn_provider_stream(&state, transport.clone(), invocation);
    }

    let response = match encode_message(&ai::BidiAppendResponse {}) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    connect_response(StatusCode::OK, response)
}

async fn handle_empty_connect() -> Response<Body> {
    let body = match encode_message(&ai::IsConnectedResponse {}) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("empty Connect response headers are valid")
}

async fn handle_is_connected() -> Response<Body> {
    let body = match encode_message(&ai::IsConnectedResponse {}) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("static IsConnected response headers are valid")
}

async fn handle_available_models(State(state): State<BackendState>) -> Response<Body> {
    let model_names = provider_model_ids(state.provider.as_ref());
    let models = model_names
        .iter()
        .map(|name| ai::AvailableModel {
            name: name.clone(),
            default_on: true,
            supports_agent: Some(true),
            server_model_name: Some(name.clone()),
        })
        .collect();
    let body = match encode_message(&ai::AvailableModelsResponse {
        model_names,
        models,
    }) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("static AvailableModels response headers are valid")
}

async fn handle_get_usable_models(State(state): State<BackendState>) -> Response<Body> {
    let models = provider_model_ids(state.provider.as_ref())
        .into_iter()
        .map(|model_id| ai::ModelDetails {
            model_id,
            ..Default::default()
        })
        .collect();
    let body = match encode_message(&ai::GetUsableModelsResponse { models }) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("static GetUsableModels response headers are valid")
}

async fn handle_get_default_model(State(state): State<BackendState>) -> Response<Body> {
    let model = provider_model_ids(state.provider.as_ref())
        .into_iter()
        .next()
        .unwrap_or_default();
    let body = match encode_message(&ai::GetDefaultModelResponse {
        thinking_model: model.clone(),
        model,
        max_mode: false,
        next_default_set_date: String::new(),
    }) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("static GetDefaultModel response headers are valid")
}

async fn handle_get_default_model_for_cli(State(state): State<BackendState>) -> Response<Body> {
    let model = provider_model_ids(state.provider.as_ref())
        .into_iter()
        .next()
        .map(|model_id| agent::ModelDetails {
            model_id,
            ..Default::default()
        });
    let body = match encode_message(&agent::GetDefaultModelForCliResponse { model }) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("static GetDefaultModelForCli response headers are valid")
}

async fn handle_get_server_config() -> Response<Body> {
    let body = match encode_message(&ai::GetServerConfigResponse {
        config_version: "cc2cx-local-agent-v1".into(),
        http2_config: 1,
        cli_sandbox_default_enabled: Some(true),
    }) {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/proto")
        .body(Body::from(body))
        .expect("static GetServerConfig response headers are valid")
}

fn provider_model_ids(provider: Option<&Arc<dyn CursorProvider>>) -> Vec<String> {
    let Some(provider) = provider else {
        return Vec::new();
    };
    let mut ids = provider
        .model_ids()
        .into_iter()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

async fn handle_run(State(state): State<BackendState>, request: Request<Body>) -> Response<Body> {
    let (headers, body) = request.into_parts();
    let body = match decode_run_request_body(&headers.headers, body).await {
        Ok(body) => body,
        Err((status, message)) => return protocol_error(status, message),
    };
    let header_map = headers.headers;
    let stream = async_stream::stream! {
        // Flush HTTP response headers before the client finishes the request body.
        // Cursor's Connect client times out stream setup in ~2s if HEADERS never arrive.
        if let Ok(connect::EncodedCursorFrame::Data(frame)) =
            connect::encode_output_event(&CursorOutputEvent::Heartbeat)
        {
            yield Ok::<Bytes, Infallible>(frame);
        }
        let mut body_stream = body.into_data_stream();
        let mut buffered = BytesMut::new();
        let first = loop {
            let Some(chunk) = body_stream.next().await else {
                yield Ok::<Bytes, Infallible>(run_connect_error(
                    ConnectCode::InvalidArgument,
                    "Cursor Run 请求缺少 AgentClientMessage 数据帧",
                ));
                return;
            };
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    yield Ok(run_connect_error(
                        ConnectCode::Unavailable,
                        format!("读取 Cursor Run 请求失败: {error}"),
                    ));
                    return;
                }
            };
            buffered.extend_from_slice(&chunk);
            match take_connect_frame(&mut buffered) {
                Ok(Some(frame)) => break frame,
                Ok(None) => continue,
                Err(error) => {
                    yield Ok(run_connect_error(ConnectCode::InvalidArgument, error));
                    return;
                }
            }
        };
        let (flags, payload) = first;
        if flags & END_STREAM_FLAG != 0 {
            yield Ok(run_connect_error(
                ConnectCode::InvalidArgument,
                "Cursor Run 首帧不能是 terminal frame",
            ));
            return;
        }
        let message = match agent::AgentClientMessage::decode(payload.as_ref()) {
            Ok(message) => message,
            Err(error) => {
                yield Ok(run_connect_error(
                    ConnectCode::InvalidArgument,
                    format!("Cursor Run AgentClientMessage 解码失败: {error}"),
                ));
                return;
            }
        };
        let request_id = run_request_id(&header_map, &message);
        let decoded = DecodedAppend {
            request_id: request_id.clone(),
            seqno: 0,
            message,
        };
        let cursor_request = match CursorRequest::from_decoded_append(decoded) {
            Ok(request) => request,
            Err(error) => {
                yield Ok(run_connect_error(
                    ConnectCode::InvalidArgument,
                    format!("Cursor Run action is unsupported: {:?}", error.unsupported),
                ));
                return;
            }
        };
        if !cursor_request.field_dispositions.is_empty() {
            yield Ok(run_connect_error(
                ConnectCode::InvalidArgument,
                format!(
                    "Cursor Run request fields are unsupported: {:?}",
                    cursor_request.field_dispositions
                ),
            ));
            return;
        }
        let invocation = match ProviderInvocation::from_request(cursor_request) {
            Ok(invocation) => invocation,
            Err(error) => {
                yield Ok(run_connect_error(
                    ConnectCode::InvalidArgument,
                    error.reason,
                ));
                return;
            }
        };
        if state.registry.current(&request_id).is_some() {
            yield Ok(run_connect_error(
                ConnectCode::Unavailable,
                "Cursor Run request_id is already active",
            ));
            return;
        }
        let transport = match state.registry.get_or_create_for_append(&request_id) {
            Ok(transport) => transport,
            Err(error) => {
                yield Ok(run_connect_error(
                    ConnectCode::Unavailable,
                    error.to_string(),
                ));
                return;
            }
        };
        if let Err(error) = transport.append(0, payload) {
            yield Ok(run_connect_error(
                ConnectCode::Unavailable,
                error.to_string(),
            ));
            return;
        }
        state.route_changed.notify_waiters();
        spawn_provider_stream(&state, transport.clone(), invocation);

        let followup_transport = transport.clone();
        tokio::spawn(async move {
            let mut buffered = BytesMut::new();
            while let Some(chunk) = body_stream.next().await {
                let Ok(chunk) = chunk else {
                    close_run_with_error(
                        &followup_transport,
                        connect::ConnectCode::Unavailable,
                        "Cursor Run request stream failed",
                    );
                    return;
                };
                buffered.extend_from_slice(&chunk);
                loop {
                    let frame = match take_connect_frame(&mut buffered) {
                        Ok(Some(frame)) => frame,
                        Ok(None) => break,
                        Err(error) => {
                            close_run_with_error(
                                &followup_transport,
                                connect::ConnectCode::InvalidArgument,
                                &error,
                            );
                            return;
                        }
                    };
                    if frame.0 & END_STREAM_FLAG != 0 {
                        return;
                    }
                    let message = match agent::AgentClientMessage::decode(frame.1.as_ref()) {
                        Ok(message) => message,
                        Err(_) => {
                            close_run_with_error(
                                &followup_transport,
                                connect::ConnectCode::InvalidArgument,
                                "Cursor Run follow-up message is invalid",
                            );
                            return;
                        }
                    };
                    if is_cancel_action(&message) {
                        close_run_with_error(
                            &followup_transport,
                            connect::ConnectCode::Canceled,
                            "Cursor Run cancelled",
                        );
                        return;
                    }
                    if is_client_heartbeat(&message) {
                        emit_run_heartbeat(&followup_transport);
                        continue;
                    }
                    close_run_with_error(
                        &followup_transport,
                        connect::ConnectCode::InvalidArgument,
                        "Cursor Run follow-up message is unsupported",
                    );
                    return;
                }
            }
        });

        let receiver = transport.subscribe();
        let mut guard = RunSseDisconnectGuard::new(transport);
        let mut receiver = receiver;
        while let Some(frame) = receiver.recv().await {
            let terminal = frame
                .first()
                .is_some_and(|flags| flags & END_STREAM_FLAG != 0);
            if terminal {
                guard.complete();
            }
            yield Ok::<Bytes, Infallible>(frame);
            if terminal {
                return;
            }
        }
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/connect+proto")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("connect-protocol-version", "1")
        .body(Body::from_stream(stream))
        .expect("static Run response headers are valid")
}

fn run_connect_error(code: ConnectCode, message: impl Into<String>) -> Bytes {
    connect::encode_error_end_stream(&ConnectStreamError {
        code,
        message: message.into(),
        details: Vec::new(),
    })
    .unwrap_or_else(|_| connect::encode_end_stream())
}

fn is_cancel_action(message: &agent::AgentClientMessage) -> bool {
    matches!(
        message.message.as_ref(),
        Some(agent::agent_client_message::Message::ConversationAction(action))
            if matches!(
                action.action.as_ref(),
                Some(agent::conversation_action::Action::CancelAction(_))
            )
    )
}

fn is_client_heartbeat(message: &agent::AgentClientMessage) -> bool {
    matches!(
        message.message.as_ref(),
        Some(agent::agent_client_message::Message::ClientHeartbeat(_))
    )
}

fn emit_run_heartbeat(transport: &TransportHandle) {
    let Ok(connect::EncodedCursorFrame::Data(frame)) =
        connect::encode_output_event(&CursorOutputEvent::Heartbeat)
    else {
        return;
    };
    let _ = transport.emit_frame(frame);
}

fn close_run_with_error(transport: &TransportHandle, code: connect::ConnectCode, message: &str) {
    let frame = connect::encode_error_end_stream(&connect::ConnectStreamError {
        code,
        message: message.to_string(),
        details: Vec::new(),
    })
    .expect("static Cursor Run error frame must encode");
    let _ = transport.cancel_with_frame(frame);
}

fn content_encoding(headers: &http::HeaderMap) -> String {
    headers
        .get(header::CONTENT_ENCODING)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("identity")
        .to_ascii_lowercase()
}

fn decompress_complete(
    headers: &http::HeaderMap,
    body: Bytes,
) -> Result<Bytes, (StatusCode, String)> {
    match content_encoding(headers).as_str() {
        "identity" => Ok(body),
        "gzip" => {
            let mut decoder = flate2::read::GzDecoder::new(body.as_ref());
            let mut decoded = Vec::new();
            decoder
                .by_ref()
                .take((MAX_REQUEST_BODY_BYTES + 1) as u64)
                .read_to_end(&mut decoded)
                .map_err(|error| (StatusCode::BAD_REQUEST, format!("gzip 解压失败: {error}")))?;
            if decoded.len() > MAX_REQUEST_BODY_BYTES {
                return Err((
                    StatusCode::PAYLOAD_TOO_LARGE,
                    "解压后数据超过大小限制".to_string(),
                ));
            }
            Ok(Bytes::from(decoded))
        }
        encoding => Err((
            StatusCode::BAD_REQUEST,
            format!("不支持的 Content-Encoding: {encoding}"),
        )),
    }
}

async fn decode_run_request_body(
    headers: &http::HeaderMap,
    body: Body,
) -> Result<Body, (StatusCode, String)> {
    match content_encoding(headers).as_str() {
        "identity" => Ok(body),
        "gzip" => Ok(gzip_decode_stream(body)),
        encoding => Err((
            StatusCode::BAD_REQUEST,
            format!("Cursor Run 不支持的 Content-Encoding: {encoding}"),
        )),
    }
}

fn gzip_decode_stream(body: Body) -> Body {
    Body::from_stream(async_stream::stream! {
        let mut decoder = flate2::write::GzDecoder::new(Vec::new());
        let mut produced = 0usize;
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    yield Err(std::io::Error::other(error.to_string()));
                    return;
                }
            };
            if decoder.write_all(&chunk).is_err() {
                yield Err(std::io::Error::other("Cursor Run gzip 解压失败"));
                return;
            }
            let buf = decoder.get_ref();
            if buf.len() > MAX_REQUEST_BODY_BYTES {
                yield Err(std::io::Error::other(
                    "Cursor Run 解压后数据帧超过大小限制",
                ));
                return;
            }
            if buf.len() > produced {
                let next = Bytes::copy_from_slice(&buf[produced..]);
                produced = buf.len();
                yield Ok(next);
            }
        }
        if decoder.try_finish().is_err() {
            yield Err(std::io::Error::other("Cursor Run gzip 结束失败"));
            return;
        }
        let buf = decoder.get_ref();
        if buf.len() > produced {
            yield Ok(Bytes::copy_from_slice(&buf[produced..]));
        }
    })
}

fn run_request_id(headers: &http::HeaderMap, message: &agent::AgentClientMessage) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .or_else(|| match message.message.as_ref() {
            Some(agent::agent_client_message::Message::RunRequest(request)) => request
                .conversation_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            _ => None,
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn take_connect_frame(buffer: &mut BytesMut) -> Result<Option<(u8, Bytes)>, String> {
    if buffer.len() < 5 {
        return Ok(None);
    }
    let flags = buffer[0];
    let length = u32::from_be_bytes([buffer[1], buffer[2], buffer[3], buffer[4]]) as usize;
    if length > MAX_REQUEST_BODY_BYTES {
        return Err("Cursor Run 数据帧超过大小限制".to_string());
    }
    if buffer.len() < 5 + length {
        return Ok(None);
    }
    let _ = buffer.split_to(5);
    Ok(Some((flags, buffer.split_to(length).freeze())))
}

fn spawn_provider_stream(
    state: &BackendState,
    transport: TransportHandle,
    invocation: ProviderInvocation,
) {
    if !transport.start_once() {
        return;
    }
    let provider = state.provider.clone();
    let health = state.health.clone();
    tokio::spawn(async move {
        let result = match provider {
            Some(provider) => {
                let cancellation = transport.cancellation_token();
                let provider_stream = provider.stream(invocation, cancellation);
                let event_health = health.clone();
                let provider_stream = provider_stream.map(move |item| {
                    match &item {
                        Err(error) => event_health.record_error(&error.code),
                        Ok(super::adapter::ProviderEvent::Error { code, .. }) => {
                            event_health.record_error(code)
                        }
                        Ok(super::adapter::ProviderEvent::Done) => event_health.record_success(),
                        Ok(_) => {}
                    }
                    item
                });
                let outcome =
                    run_provider_stream(transport.clone(), provider_stream, Duration::ZERO).await;
                match outcome {
                    Ok(outcome) => {
                        health.record_outcome(outcome);
                        Ok(())
                    }
                    Err(error) => {
                        health.record_error("provider_protocol");
                        Err(error)
                    }
                }
            }
            None => run_synthetic_stream(transport.clone(), invocation).await,
        };
        if let Err(error) = result {
            let _ = transport.finish(
                super::protocol::connect::encode_error_end_stream(
                    &super::protocol::connect::ConnectStreamError {
                        code: super::protocol::connect::ConnectCode::Internal,
                        message: error.to_string(),
                        details: Vec::new(),
                    },
                )
                .unwrap_or_else(|_| super::protocol::connect::encode_end_stream()),
            );
        }
    });
}

async fn handle_run_sse(
    State(state): State<BackendState>,
    request: Request<Body>,
) -> Response<Body> {
    let body = match to_bytes(request.into_body(), MAX_REQUEST_BODY_BYTES).await {
        Ok(body) => body,
        Err(error) => return protocol_error(StatusCode::PAYLOAD_TOO_LARGE, error.to_string()),
    };
    let request_id = match run_sse::decode_request(&body) {
        Ok(request) => request.request_id,
        Err(error) => return protocol_error(StatusCode::BAD_REQUEST, error.to_string()),
    };
    let Some(transport) = wait_for_transport(&state, &request_id).await else {
        return protocol_error(
            StatusCode::REQUEST_TIMEOUT,
            format!(
                "等待 Cursor transport 超时（{} 秒）",
                RUN_SSE_ROUTE_WAIT.as_secs()
            ),
        );
    };
    let receiver = transport.subscribe();
    let mut guard = RunSseDisconnectGuard::new(transport);
    let stream = async_stream::stream! {
        let mut receiver = receiver;
        while let Some(frame) = receiver.recv().await {
            let terminal = frame
                .first()
                .is_some_and(|flags| flags & END_STREAM_FLAG != 0);
            if terminal {
                guard.complete();
            }
            yield Ok::<Bytes, Infallible>(frame);
            if terminal {
                return;
            }
        }
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("connect-protocol-version", "1")
        .body(Body::from_stream(stream))
        .expect("static RunSSE response headers are valid")
}

struct RunSseDisconnectGuard {
    transport: TransportHandle,
    completed: bool,
}

impl RunSseDisconnectGuard {
    fn new(transport: TransportHandle) -> Self {
        Self {
            transport,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for RunSseDisconnectGuard {
    fn drop(&mut self) {
        if !self.completed {
            self.transport.disconnect();
        }
    }
}

async fn wait_for_transport(
    state: &BackendState,
    request_id: &str,
) -> Option<super::transport::TransportHandle> {
    tokio::time::timeout(RUN_SSE_ROUTE_WAIT, async {
        loop {
            if let Some(transport) = state.registry.current(request_id) {
                return transport;
            }
            let notified = state.route_changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if let Some(transport) = state.registry.current(request_id) {
                return transport;
            }
            notified.await;
        }
    })
    .await
    .ok()
}

fn connect_response(status: StatusCode, body: Bytes) -> Response<Body> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/connect+proto")
        .body(Body::from(body))
        .expect("static Connect response headers are valid")
}

fn protocol_error(status: StatusCode, message: impl Into<String>) -> Response<Body> {
    let error = message.into();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(error))
        .expect("static protocol error response headers are valid")
}

fn transport_error(error: TransportError) -> Response<Body> {
    let status = match error {
        TransportError::EmptyRequestId | TransportError::InvalidRequestId => {
            StatusCode::BAD_REQUEST
        }
        TransportError::Sequence { .. }
        | TransportError::Closed
        | TransportError::SequenceExhausted => StatusCode::CONFLICT,
        TransportError::GenerationExhausted => StatusCode::INTERNAL_SERVER_ERROR,
        TransportError::NotFound => StatusCode::NOT_FOUND,
    };
    protocol_error(status, error.to_string())
}
