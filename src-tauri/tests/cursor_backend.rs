use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
    routing::post,
    Router,
};
use bytes::Bytes;
use cc_launch_lib::cursor::{
    adapter::{ProviderEvent, ProviderInvocation, ProviderStreamError},
    protocol::{
        connect::{decode_unary, encode_message},
        proto::{agent::v1 as agent, aiserver::v1 as ai},
        run_sse::{decode_response_body, RunSseFrame},
    },
    protocol_backend::CursorProtocolBackend,
    provider::{CursorProvider, ProviderStream},
};
use cc_launch_lib::{AppType, Database, Provider, ProviderMeta};
use flate2::{write::GzEncoder, Compression};
use futures::stream;
use http_body_util::BodyExt;
use prost::Message;
use serde_json::json;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};
use tower::Service;

struct FixtureProvider;
struct FailingProvider;

struct CancellationProvider {}

impl CursorProvider for CancellationProvider {
    fn stream(
        &self,
        _invocation: ProviderInvocation,
        cancellation: tokio_util::sync::CancellationToken,
    ) -> ProviderStream {
        Box::pin(async_stream::stream! {
            cancellation.cancelled().await;
            yield Ok(ProviderEvent::Done);
        })
    }
}

#[tokio::test]
async fn health_reports_explicit_protocol_capabilities_without_credentials() {
    let mut router = CursorProtocolBackend::new().router();
    let response = router
        .call(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("health request should route");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["status"], "ok");
    assert_eq!(value["capabilities"]["single_turn_stream"], true);
    assert_eq!(value["capabilities"]["history"], false);
    assert!(body
        .windows(b"token".len())
        .all(|window| window != b"token"));
    assert_eq!(value["provider"]["mode"], "synthetic");
    assert_eq!(value["provider"]["authentication"], "not_required");
}

#[tokio::test]
async fn is_connected_returns_an_empty_connect_protobuf_without_provider_auth() {
    let mut router = CursorProtocolBackend::new().router();
    let response = call(
        &mut router,
        Request::post("/aiserver.v1.NetworkService/IsConnected")
            .header("content-type", "application/connect+proto")
            .body(Body::from(
                encode_message(&ai::IsConnectedResponse {}).unwrap(),
            ))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/proto")
    );
    let body = to_bytes(response.into_body(), 4096)
        .await
        .expect("read IsConnected response");
    let decoded: ai::IsConnectedResponse = decode_unary(&body).expect("decode IsConnected");
    assert_eq!(decoded, ai::IsConnectedResponse {});
}

#[tokio::test]
async fn health_reports_provider_binding_without_exposing_credentials() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let response = call(
        &mut router,
        Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["provider"]["mode"], "configured");
    assert_eq!(value["provider"]["authentication"], "configured");
    assert_eq!(value["provider"]["upstream_probe"], "not_run");
    assert!(!body
        .windows(b"fixture".len())
        .any(|window| window == b"fixture"));
}

#[tokio::test]
async fn model_catalog_returns_provider_ids_without_credentials() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let response = call(
        &mut router,
        Request::post("/aiserver.v1.AiService/AvailableModels")
            .header("content-type", "application/connect+proto")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/proto")
    );
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read AvailableModels response");
    let catalog: ai::AvailableModelsResponse =
        decode_unary(&body).expect("decode AvailableModels response");
    assert_eq!(
        catalog.model_names,
        vec!["fallback-model", "injected-model"]
    );
    assert_eq!(catalog.models.len(), 2);
    assert!(catalog
        .models
        .iter()
        .all(|model| model.supports_agent == Some(true)));
    assert!(!body
        .windows(b"token".len())
        .any(|window| window == b"token"));
}

#[tokio::test]
async fn usable_model_alias_returns_the_same_provider_ids() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/GetUsableModels")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read GetUsableModels response");
    let catalog: ai::GetUsableModelsResponse =
        decode_unary(&body).expect("decode GetUsableModels response");
    let ids = catalog
        .models
        .into_iter()
        .map(|model| model.model_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["fallback-model", "injected-model"]);
}

#[tokio::test]
async fn default_model_endpoints_return_the_first_configured_provider_model() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();

    for path in [
        "/aiserver.v1.AiService/GetDefaultModel",
        "/agent.v1.AgentService/GetDefaultModelForCli",
        "/aiserver.v1.AiService/GetDefaultModelForCli",
    ] {
        let response = call(
            &mut router,
            Request::post(path)
                .header("content-type", "application/connect+proto")
                .body(Body::empty())
                .unwrap(),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read default model response");
        assert!(!body.is_empty(), "{path} must return a protobuf response");
        assert!(
            body.windows(b"fallback-model".len())
                .any(|window| window == b"fallback-model"),
            "{path} must expose the configured default model"
        );
    }
}

#[tokio::test]
async fn server_config_returns_safe_local_agent_defaults_without_identity_fields() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();
    let response = call(
        &mut router,
        Request::post("/aiserver.v1.ServerConfigService/GetServerConfig")
            .body(Body::empty())
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/proto")
    );
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("read GetServerConfig response");
    let config: ai::GetServerConfigResponse =
        decode_unary(&body).expect("decode GetServerConfig response");
    assert_eq!(config.config_version, "cc2cx-local-agent-v1");
    assert_eq!(config.http2_config, 1);
    assert_eq!(config.cli_sandbox_default_enabled, Some(true));
    assert!(!body
        .windows(b"token".len())
        .any(|window| window == b"token"));
}

#[tokio::test]
async fn health_reports_observed_provider_auth_failure_without_exposing_error_text() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FailingProvider));
    let mut router = backend.router();
    let request_id = "request-health-auth-failure";

    let append_body = encode_message(&bidi_request(request_id, 0)).expect("encode append");
    let append_response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(append_body))
            .unwrap(),
    )
    .await;
    assert_eq!(append_response.status(), StatusCode::OK);
    let _ = to_bytes(append_response.into_body(), 4096)
        .await
        .expect("read append response");

    let run_sse_body = encode_message(&ai::BidiRequestId {
        request_id: request_id.into(),
    })
    .expect("encode RunSSE request");
    let run_response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .body(Body::from(run_sse_body))
            .unwrap(),
    )
    .await;
    let _ = to_bytes(run_response.into_body(), 1024 * 1024)
        .await
        .expect("read failed RunSSE response");

    let health = call(
        &mut router,
        Request::get("/health").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(health.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(health.into_body(), 4096)
        .await
        .expect("read health response");
    let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["status"], "degraded");
    assert_eq!(value["provider"]["health"], "auth_failed");
    assert_eq!(value["provider"]["last_error_code"], "provider_auth");
    assert!(!body
        .windows(b"fixture authentication failed".len())
        .any(|window| window == b"fixture authentication failed"));
}

struct OpenAiFixtureServer {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl OpenAiFixtureServer {
    async fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("OpenAI fixture server should bind");
        let address = listener
            .local_addr()
            .expect("OpenAI fixture server address");
        let (stop, stopped) = oneshot::channel();
        let router = Router::new().route(
            "/chat/completions",
            post(|| async {
                Response::builder()
                    .status(StatusCode::OK)
                    .header("content-type", "text/event-stream")
                    .body(Body::from(
                        "data: {\"choices\":[{\"delta\":{\"content\":\"database response\"}}]}\n\n"
                            .to_string()
                            + "data: [DONE]\n\n",
                    ))
                    .expect("OpenAI fixture response")
            }),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = stopped.await;
                })
                .await
        });
        Self {
            address,
            stop: Some(stop),
            task,
        }
    }

    fn url(&self) -> String {
        format!("http://{}/chat/completions", self.address)
    }
}

impl Drop for OpenAiFixtureServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}

impl CursorProvider for FixtureProvider {
    fn model_ids(&self) -> Vec<String> {
        vec!["injected-model".into(), "fallback-model".into()]
    }

    fn stream(
        &self,
        _invocation: ProviderInvocation,
        _cancellation: tokio_util::sync::CancellationToken,
    ) -> ProviderStream {
        Box::pin(stream::iter([
            Ok(ProviderEvent::TextDelta("injected response".into())),
            Ok(ProviderEvent::Done),
        ]))
    }
}

impl CursorProvider for FailingProvider {
    fn stream(
        &self,
        _invocation: ProviderInvocation,
        _cancellation: tokio_util::sync::CancellationToken,
    ) -> ProviderStream {
        Box::pin(stream::once(async {
            Err(ProviderStreamError {
                code: "provider_auth".into(),
                message: "fixture authentication failed".into(),
            })
        }))
    }
}

fn bidi_request(request_id: &str, seqno: i64) -> ai::BidiAppendRequest {
    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-backend".into(),
                    ..Default::default()
                }),
                conversation_id: Some("conversation-backend".into()),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "hello backend".into(),
                                message_id: "message-backend".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };

    bidi_request_with_message(request_id, seqno, message)
}

fn bidi_request_with_message(
    request_id: &str,
    seqno: i64,
    message: agent::AgentClientMessage,
) -> ai::BidiAppendRequest {
    ai::BidiAppendRequest {
        data: hex::encode(message.encode_to_vec()),
        request_id: Some(ai::BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: seqno,
        ..Default::default()
    }
}

async fn call(router: &mut Router, request: Request<Body>) -> axum::response::Response {
    <Router as Service<Request<Body>>>::call(router, request)
        .await
        .expect("backend router is infallible")
}

#[tokio::test]
async fn bidi_append_starts_synthetic_run_sse_replays_and_terminates() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();

    let append_body = encode_message(&bidi_request("request-backend", 0)).expect("encode append");
    let append_response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .header("content-type", "application/connect+proto")
            .body(Body::from(append_body))
            .unwrap(),
    )
    .await;
    assert_eq!(append_response.status(), StatusCode::OK);
    let append_bytes = axum::body::to_bytes(append_response.into_body(), 1024 * 1024)
        .await
        .expect("read append response");
    let append_response: ai::BidiAppendResponse =
        decode_unary(&append_bytes).expect("decode append response");
    assert_eq!(append_response, ai::BidiAppendResponse {});

    let run_sse_body = encode_message(&ai::BidiRequestId {
        request_id: "request-backend".into(),
    })
    .expect("encode RunSSE request");
    let run_response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .header("content-type", "application/connect+proto")
            .body(Body::from(run_sse_body.clone()))
            .unwrap(),
    )
    .await;
    assert_eq!(run_response.status(), StatusCode::OK);
    assert_eq!(
        run_response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let response_bytes = axum::body::to_bytes(run_response.into_body(), 1024 * 1024)
        .await
        .expect("read RunSSE response");
    let frames = decode_response_body(&response_bytes).expect("decode RunSSE frames");
    assert_eq!(frames.len(), 3);
    assert!(matches!(frames[0], RunSseFrame::Data { .. }));
    assert!(matches!(frames[1], RunSseFrame::Data { .. }));
    assert!(matches!(frames[2], RunSseFrame::End { error: None }));
    let first_payload = match &frames[0] {
        RunSseFrame::Data { payload, .. } => payload,
        RunSseFrame::End { .. } => unreachable!(),
    };
    let first_message = agent::AgentServerMessage::decode(first_payload.as_ref())
        .expect("decode text output message");
    assert!(matches!(
        first_message.message,
        Some(agent::agent_server_message::Message::InteractionUpdate(
            agent::InteractionUpdate {
                message: Some(agent::interaction_update::Message::TextDelta(_))
            }
        ))
    ));

    let replay_response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .header("content-type", "application/connect+proto")
            .body(Body::from(run_sse_body))
            .unwrap(),
    )
    .await;
    let replay_bytes = axum::body::to_bytes(replay_response.into_body(), 1024 * 1024)
        .await
        .expect("read replay response");
    let replay_frames = decode_response_body(&replay_bytes).expect("decode replay frames");
    assert_eq!(replay_frames, frames);
    let transport = backend
        .registry()
        .current("request-backend")
        .expect("request transport should remain addressable for replay");
    assert!(transport.is_terminal());
    assert!(!transport.is_disconnected());
}

#[tokio::test]
async fn injected_provider_stream_is_bridged_to_cursor_run_sse() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();

    let append_body =
        encode_message(&bidi_request("request-injected-provider", 0)).expect("encode append");
    let append_response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .header("content-type", "application/connect+proto")
            .body(Body::from(append_body))
            .unwrap(),
    )
    .await;
    assert_eq!(append_response.status(), StatusCode::OK);
    let _ = axum::body::to_bytes(append_response.into_body(), 1024 * 1024)
        .await
        .expect("read append response");

    let run_sse_body = encode_message(&ai::BidiRequestId {
        request_id: "request-injected-provider".into(),
    })
    .expect("encode RunSSE request");
    let run_response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .header("content-type", "application/connect+proto")
            .body(Body::from(run_sse_body))
            .unwrap(),
    )
    .await;
    assert_eq!(run_response.status(), StatusCode::OK);
    let response_bytes = axum::body::to_bytes(run_response.into_body(), 1024 * 1024)
        .await
        .expect("read RunSSE response");
    let frames = decode_response_body(&response_bytes).expect("decode RunSSE frames");
    assert_eq!(frames.len(), 3);

    let payload = match &frames[0] {
        RunSseFrame::Data { payload, .. } => payload,
        RunSseFrame::End { .. } => panic!("provider text must be a data frame"),
    };
    let message = agent::AgentServerMessage::decode(payload.as_ref()).expect("decode text frame");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("provider text must map to an interaction update")
    };
    assert!(matches!(
        update.message,
        Some(agent::interaction_update::Message::TextDelta(delta))
            if delta.text == "injected response"
    ));
    assert!(matches!(frames[1], RunSseFrame::Data { .. }));
    assert!(matches!(frames[2], RunSseFrame::End { error: None }));
}

#[tokio::test]
async fn bidi_run_accepts_agent_client_frame_and_streams_provider_output() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-bidi-run".into(),
                    ..Default::default()
                }),
                conversation_id: Some("conversation-bidi-run".into()),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "hello bidi run".into(),
                                message_id: "message-bidi-run".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let request_body = encode_message(&message).expect("encode Run client frame");

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .body(Body::from(request_body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/connect+proto")
    );
    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read bidi Run response");
    let frames = decode_response_body(&response_body).expect("decode bidi Run response frames");
    assert!(frames
        .iter()
        .any(|frame| matches!(frame, RunSseFrame::End { error: None })));
    let decoded_messages = frames
        .iter()
        .filter_map(|frame| match frame {
            RunSseFrame::Data { payload, .. } => Some(
                agent::AgentServerMessage::decode(payload.as_ref())
                    .expect("decode bidi Run server frame"),
            ),
            RunSseFrame::End { .. } => None,
        })
        .collect::<Vec<_>>();
    assert!(decoded_messages.iter().any(|message| {
        matches!(
            &message.message,
            Some(agent::agent_server_message::Message::InteractionUpdate(
                agent::InteractionUpdate {
                    message: Some(agent::interaction_update::Message::Heartbeat(_))
                }
            ))
        )
    }));
    assert!(decoded_messages.iter().any(|message| {
        matches!(
            &message.message,
            Some(agent::agent_server_message::Message::InteractionUpdate(
                agent::InteractionUpdate {
                    message: Some(agent::interaction_update::Message::TextDelta(delta))
                }
            )) if delta.text == "injected response"
        )
    }));
}

#[tokio::test]
async fn bidi_run_cancel_action_cancels_provider_and_emits_connect_cancelled_frame() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(CancellationProvider {}));
    let mut router = backend.router();
    let request = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-cancel-run".into(),
                    ..Default::default()
                }),
                conversation_id: Some("conversation-cancel-run".into()),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "cancel me".into(),
                                message_id: "message-cancel-run".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let cancel = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::ConversationAction(
            agent::ConversationAction {
                action: Some(agent::conversation_action::Action::CancelAction(
                    agent::CancelAction {
                        reason: "user_cancelled".into(),
                    },
                )),
            },
        )),
    };
    let first = encode_message(&request).expect("encode Run request");
    let second = encode_message(&cancel).expect("encode cancel action");
    let body = stream::iter([
        Ok::<Bytes, std::io::Error>(first),
        Ok::<Bytes, std::io::Error>(second),
    ]);

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .body(Body::from_stream(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read cancelled Run response");
    let frames = decode_response_body(&response_body).expect("decode cancelled Run response");
    assert!(frames.iter().any(|frame| {
        matches!(
            frame,
            RunSseFrame::End {
                error: Some(error)
            } if error.code.as_deref() == Some("canceled")
        )
    }));
    let transport = backend
        .registry()
        .current("conversation-cancel-run")
        .expect("cancelled transport should remain inspectable");
    assert!(transport.cancellation_token().is_cancelled());
}

#[tokio::test]
async fn bidi_run_client_end_stream_is_a_half_close_not_a_cancellation() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let request = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-half-close-run".into(),
                    ..Default::default()
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "half close".into(),
                                message_id: "message-half-close-run".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let first = encode_message(&request).expect("encode Run request");
    let body = stream::iter([
        Ok::<Bytes, std::io::Error>(first),
        Ok::<Bytes, std::io::Error>(cc_launch_lib::cursor::protocol::connect::encode_end_stream()),
    ]);

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .body(Body::from_stream(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read half-close Run response");
    let frames = decode_response_body(&response_body).expect("decode half-close Run response");
    assert!(frames
        .iter()
        .any(|frame| matches!(frame, RunSseFrame::End { error: None })));
}

#[tokio::test]
async fn bidi_run_rejects_unsupported_follow_up_messages_explicitly() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(CancellationProvider {}));
    let mut router = backend.router();
    let request = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-unsupported-follow-up".into(),
                    ..Default::default()
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "unsupported follow-up".into(),
                                message_id: "message-unsupported-follow-up".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let unsupported = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::ExecClientMessage(
            agent::EmptyMessage {},
        )),
    };
    let body = stream::iter([
        Ok::<Bytes, std::io::Error>(encode_message(&request).expect("encode Run request")),
        Ok::<Bytes, std::io::Error>(
            encode_message(&unsupported).expect("encode unsupported follow-up"),
        ),
    ]);

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .body(Body::from_stream(body))
            .unwrap(),
    )
    .await;

    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read unsupported follow-up response");
    let frames =
        decode_response_body(&response_body).expect("decode unsupported follow-up response");
    assert!(frames.iter().any(|frame| {
        matches!(
            frame,
            RunSseFrame::End {
                error: Some(error)
            } if error.code.as_deref() == Some("invalid_argument")
                && error.message.as_deref().is_some_and(|message| message.contains("unsupported"))
        )
    }));
}

#[tokio::test]
async fn bidi_run_keeps_client_heartbeat_in_the_active_stream() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(CancellationProvider {}));
    let mut router = backend.router();
    let request = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-heartbeat-follow-up".into(),
                    ..Default::default()
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "heartbeat follow-up".into(),
                                message_id: "message-heartbeat-follow-up".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let heartbeat = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::ClientHeartbeat(
            agent::EmptyMessage {},
        )),
    };
    let cancel = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::ConversationAction(
            agent::ConversationAction {
                action: Some(agent::conversation_action::Action::CancelAction(
                    agent::CancelAction {
                        reason: "user_cancelled".into(),
                    },
                )),
            },
        )),
    };
    let body = stream::iter([
        Ok::<Bytes, std::io::Error>(encode_message(&request).expect("encode Run request")),
        Ok::<Bytes, std::io::Error>(encode_message(&heartbeat).expect("encode heartbeat")),
        Ok::<Bytes, std::io::Error>(encode_message(&cancel).expect("encode cancel")),
    ]);

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .body(Body::from_stream(body))
            .unwrap(),
    )
    .await;

    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read heartbeat response");
    let frames = decode_response_body(&response_body).expect("decode heartbeat response");
    let heartbeat_payload = frames.iter().find_map(|frame| match frame {
        RunSseFrame::Data { payload, .. } => Some(payload),
        RunSseFrame::End { .. } => None,
    });
    let heartbeat_message = agent::AgentServerMessage::decode(
        heartbeat_payload
            .expect("heartbeat should produce a data frame")
            .as_ref(),
    )
    .expect("decode heartbeat server message");
    assert!(matches!(
        heartbeat_message.message,
        Some(agent::agent_server_message::Message::InteractionUpdate(update))
            if matches!(update.message, Some(agent::interaction_update::Message::Heartbeat(_)))
    ));
    assert!(frames.iter().any(|frame| {
        matches!(frame, RunSseFrame::End { error: Some(error) } if error.code.as_deref() == Some("canceled"))
    }));
}

#[tokio::test]
async fn bidi_run_accepts_a_gzip_encoded_connect_request_frame() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-gzip-run".into(),
                    ..Default::default()
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "gzip request".into(),
                                message_id: "message-gzip-run".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let framed = encode_message(&message).expect("encode Run client frame");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::Write::write_all(&mut encoder, &framed).expect("gzip request frame");
    let compressed = encoder.finish().expect("finish gzip request frame");

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .header("content-encoding", "gzip")
            .body(Body::from(compressed))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read gzip Run response");
    let frames = decode_response_body(&response_body).expect("decode gzip Run response");
    assert!(frames
        .iter()
        .any(|frame| matches!(frame, RunSseFrame::End { error: None })));
}

#[tokio::test]
async fn bidi_append_accepts_a_gzip_encoded_connect_request() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let framed = encode_message(&bidi_request("req-gzip-bidi", 0)).expect("encode bidi");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    std::io::Write::write_all(&mut encoder, &framed).expect("gzip bidi");
    let compressed = encoder.finish().expect("finish gzip bidi");

    let response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .header("content-type", "application/connect+proto")
            .header("content-encoding", "gzip")
            .body(Body::from(compressed))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn run_returns_ok_headers_before_request_body_arrives() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let (_tx, rx) = futures::channel::mpsc::channel::<Result<Bytes, std::io::Error>>(1);

    let response = tokio::time::timeout(
        Duration::from_millis(800),
        call(
            &mut router,
            Request::post("/agent.v1.AgentService/Run")
                .header("content-type", "application/connect+proto")
                .body(Body::from_stream(rx))
                .unwrap(),
        ),
    )
    .await
    .expect("Run must send HTTP 200 before the client body arrives");

    assert_eq!(response.status(), StatusCode::OK);
    let first = tokio::time::timeout(Duration::from_millis(800), response.into_body().frame())
        .await
        .expect("Run must flush a response frame before the client body arrives")
        .expect("response frame")
        .expect("data frame")
        .into_data()
        .expect("heartbeat data");
    assert!(!first.is_empty());
}

#[tokio::test]
async fn bidi_run_reassembles_a_connect_frame_split_across_body_chunks() {
    let backend = CursorProtocolBackend::with_provider(Arc::new(FixtureProvider));
    let mut router = backend.router();
    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-split-run".into(),
                    ..Default::default()
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "split request".into(),
                                message_id: "message-split-run".into(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let framed = encode_message(&message).expect("encode Run client frame");
    let chunks = vec![
        Ok::<Bytes, std::io::Error>(framed.slice(..2)),
        Ok(framed.slice(2..7)),
        Ok(framed.slice(7..)),
    ];

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/Run")
            .header("content-type", "application/connect+proto")
            .body(Body::from_stream(stream::iter(chunks)))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response_body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("read split Run response");
    let frames = decode_response_body(&response_body).expect("decode split Run response");
    assert!(frames
        .iter()
        .any(|frame| matches!(frame, RunSseFrame::End { error: None })));
}

#[tokio::test]
async fn explicit_database_provider_backend_bridges_current_provider() {
    let server = OpenAiFixtureServer::start().await;
    let db = Database::memory().expect("memory database");
    let mut provider = Provider::with_id(
        "provider-backend-current".into(),
        "Database current provider".into(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": server.url(),
                "ANTHROPIC_AUTH_TOKEN": "fixture-current-key",
                "ANTHROPIC_MODEL": "model-backend-fixture"
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: Some("openai_chat".into()),
        is_full_url: Some(true),
        ..Default::default()
    });
    db.save_provider(AppType::ClaudeDesktop.as_str(), &provider)
        .expect("save current provider");
    db.set_current_provider(AppType::ClaudeDesktop.as_str(), &provider.id)
        .expect("select current provider");

    let backend = CursorProtocolBackend::with_current_provider(&db, &AppType::ClaudeDesktop)
        .expect("explicit current provider backend should build");
    let mut router = backend.router();
    let append_body =
        encode_message(&bidi_request("request-current-provider", 0)).expect("encode append");
    let append_response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(append_body))
            .unwrap(),
    )
    .await;
    assert_eq!(append_response.status(), StatusCode::OK);
    let _ = to_bytes(append_response.into_body(), 1024 * 1024)
        .await
        .expect("read append response");

    let run_sse_body = encode_message(&ai::BidiRequestId {
        request_id: "request-current-provider".into(),
    })
    .expect("encode RunSSE request");
    let run_response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .body(Body::from(run_sse_body))
            .unwrap(),
    )
    .await;
    assert_eq!(run_response.status(), StatusCode::OK);
    let response_bytes = to_bytes(run_response.into_body(), 1024 * 1024)
        .await
        .expect("read RunSSE response");
    let frames = decode_response_body(&response_bytes).expect("decode RunSSE frames");
    let payload = match &frames[0] {
        RunSseFrame::Data { payload, .. } => payload,
        RunSseFrame::End { .. } => panic!("provider text must be a data frame"),
    };
    let message = agent::AgentServerMessage::decode(payload.as_ref()).expect("decode text frame");
    let Some(agent::agent_server_message::Message::InteractionUpdate(update)) = message.message
    else {
        panic!("provider text must map to an interaction update")
    };
    assert!(matches!(
        update.message,
        Some(agent::interaction_update::Message::TextDelta(delta))
            if delta.text == "database response"
    ));
    assert!(matches!(
        frames.last(),
        Some(RunSseFrame::End { error: None })
    ));
}

#[tokio::test]
async fn malformed_bidi_append_is_rejected_without_creating_transport() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();
    let response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(vec![0xff, 0x00]))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(backend.registry().current("request-backend").is_none());
}

#[tokio::test]
async fn unknown_run_sse_request_is_rejected_without_creating_phantom_transport() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();
    let body = encode_message(&ai::BidiRequestId {
        request_id: "request-unknown".into(),
    })
    .expect("encode RunSSE request");

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    let error = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .expect("read timeout response");
    assert!(String::from_utf8_lossy(&error).contains("超时"));
    assert!(backend.registry().current("request-unknown").is_none());
}

#[tokio::test]
async fn run_sse_waits_for_bidi_append_that_arrives_later() {
    let backend = CursorProtocolBackend::new();
    let router = backend.router();
    let request_id = "request-run-first";
    let run_sse_body = encode_message(&ai::BidiRequestId {
        request_id: request_id.into(),
    })
    .expect("encode RunSSE request");

    let run_router = router.clone();
    let run_task = tokio::spawn(async move {
        let mut run_router = run_router;
        call(
            &mut run_router,
            Request::post("/agent.v1.AgentService/RunSSE")
                .header("content-type", "application/connect+proto")
                .body(Body::from(run_sse_body))
                .unwrap(),
        )
        .await
    });

    tokio::time::sleep(Duration::from_millis(25)).await;
    let mut append_router = router.clone();
    let append_body = encode_message(&bidi_request(request_id, 0)).expect("encode append");
    let append_response = call(
        &mut append_router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .header("content-type", "application/connect+proto")
            .body(Body::from(append_body))
            .unwrap(),
    )
    .await;
    assert_eq!(append_response.status(), StatusCode::OK);

    let run_response = run_task.await.expect("RunSSE task should complete");
    assert_eq!(run_response.status(), StatusCode::OK);
    let response_bytes = axum::body::to_bytes(run_response.into_body(), 1024 * 1024)
        .await
        .expect("read RunSSE response");
    let frames = decode_response_body(&response_bytes).expect("decode RunSSE frames");
    assert_eq!(frames.len(), 3);
}

#[tokio::test]
async fn run_sse_client_disconnect_marks_transport_disconnected() {
    let backend = CursorProtocolBackend::new();
    let transport = backend
        .registry()
        .get_or_create("request-client-disconnect")
        .expect("create transport");
    let mut router = backend.router();
    let body = encode_message(&ai::BidiRequestId {
        request_id: "request-client-disconnect".into(),
    })
    .expect("encode RunSSE request");

    let response = call(
        &mut router,
        Request::post("/agent.v1.AgentService/RunSSE")
            .header("content-type", "application/connect+proto")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut response_body = response.into_body();

    transport.emit_frame(Bytes::from_static(b"partial response"));
    let frame = response_body
        .frame()
        .await
        .expect("read partial RunSSE frame")
        .expect("partial RunSSE frame should exist");
    assert!(frame.is_data());

    drop(response_body);
    assert!(transport.is_disconnected());
}

#[tokio::test]
async fn second_single_turn_append_is_rejected_instead_of_dropped() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();

    let first = encode_message(&bidi_request("request-single-turn", 0)).expect("encode first");
    let first_response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(first))
            .unwrap(),
    )
    .await;
    assert_eq!(first_response.status(), StatusCode::OK);

    let second = encode_message(&bidi_request("request-single-turn", 1)).expect("encode second");
    let second_response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(second))
            .unwrap(),
    )
    .await;
    assert_eq!(second_response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn unsupported_action_is_rejected_without_creating_transport() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();
    let request_id = "request-unsupported-action";
    let append = bidi_request_with_message(
        request_id,
        0,
        agent::AgentClientMessage {
            message: Some(agent::agent_client_message::Message::ClientHeartbeat(
                agent::EmptyMessage {},
            )),
        },
    );
    let body = encode_message(&append).expect("encode unsupported append");

    let response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    assert!(backend.registry().current(request_id).is_none());
}

#[tokio::test]
async fn missing_model_is_rejected_without_creating_transport() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();
    let request_id = "request-missing-model";
    let message = agent::AgentClientMessage::decode(
        hex::decode(bidi_request(request_id, 0).data)
            .expect("decode fixture hex")
            .as_slice(),
    )
    .expect("decode fixture message");
    let mut run = match message.message {
        Some(agent::agent_client_message::Message::RunRequest(run)) => run,
        _ => panic!("fixture must contain a run request"),
    };
    run.requested_model = Some(agent::RequestedModel {
        model_id: String::new(),
        ..Default::default()
    });
    let body = encode_message(&bidi_request_with_message(
        request_id,
        0,
        agent::AgentClientMessage {
            message: Some(agent::agent_client_message::Message::RunRequest(run)),
        },
    ))
    .expect("encode missing-model append");

    let response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert!(backend.registry().current(request_id).is_none());
}

#[tokio::test]
async fn unsupported_request_fields_are_rejected_before_provider_start() {
    let backend = CursorProtocolBackend::new();
    let mut router = backend.router();
    let request_id = "request-unsupported-fields";
    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "model-backend".into(),
                    max_mode: true,
                    parameters: vec![agent::ModelParameterValue {
                        id: "reasoning_effort".into(),
                        value: "high".into(),
                    }],
                }),
                mcp_tools: Some(agent::McpTools {
                    mcp_tools: vec![agent::McpToolDefinition {
                        name: "fixture-tool".into(),
                        ..Default::default()
                    }],
                }),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "hello backend".into(),
                                message_id: "message-backend".into(),
                                selected_context: Some(agent::SelectedContext {
                                    selected_images: vec![agent::SelectedImage {
                                        uuid: "fixture-image".into(),
                                        mime_type: "image/png".into(),
                                        data: b"fixture-image".to_vec(),
                                        ..Default::default()
                                    }],
                                }),
                            }),
                            conversation_history: Some(agent::ConversationHistory {
                                messages: vec![agent::ConversationHistoryMessage {
                                    message: Some(
                                        agent::conversation_history_message::Message::User(
                                            agent::ConversationHistoryUserMessage {
                                                content: vec![agent::ConversationHistoryUserContent {
                                                    content: Some(
                                                        agent::conversation_history_user_content::Content::Text(
                                                            agent::ConversationHistoryTextContent {
                                                                text: "old fixture".into(),
                                                            },
                                                        ),
                                                    ),
                                                }],
                                            },
                                        ),
                                    ),
                                }],
                            }),
                            ..Default::default()
                        },
                    )),
                }),
                ..Default::default()
            },
        )),
    };
    let append = bidi_request_with_message(request_id, 0, message);
    let body = encode_message(&append).expect("encode unsupported fields append");

    let response = call(
        &mut router,
        Request::post("/aiserver.v1.BidiService/BidiAppend")
            .body(Body::from(body))
            .unwrap(),
    )
    .await;

    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    let error = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .expect("read unsupported fields response");
    let error = String::from_utf8_lossy(&error);
    assert!(error.contains("History"));
    assert!(error.contains("Images"));
    assert!(error.contains("McpTools"));
    assert!(error.contains("ReasoningEffort"));
    assert!(backend.registry().current(request_id).is_none());
}

#[tokio::test]
async fn explicit_backend_listener_rejects_non_loopback_bind_address() {
    let backend = CursorProtocolBackend::new();
    let address = SocketAddr::from(([192, 0, 2, 1], 0));

    let error = match backend.start(address).await {
        Ok(_) => panic!("synthetic backend must be loopback-only"),
        Err(error) => error,
    };

    assert!(error.contains("loopback"));
}

#[tokio::test]
async fn explicit_backend_listener_binds_loopback_and_stops_idempotently() {
    let backend = CursorProtocolBackend::new();
    let mut runtime = backend
        .start(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("synthetic backend should bind loopback");
    let address = runtime.address();

    assert_eq!(address.ip(), std::net::IpAddr::from([127, 0, 0, 1]));
    assert_ne!(address.port(), 0);
    assert_eq!(runtime.url(), format!("http://{address}"));
    tokio::net::TcpStream::connect(address)
        .await
        .expect("listener should accept loopback connections");

    runtime.stop().await.expect("first stop should succeed");
    runtime
        .stop()
        .await
        .expect("second stop should be idempotent");

    tokio::net::TcpListener::bind(address)
        .await
        .expect("stopping the backend should release its port");
}

#[tokio::test]
async fn dropping_backend_runtime_releases_listener() {
    let address = {
        let backend = CursorProtocolBackend::new();
        let runtime = backend
            .start(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("synthetic backend should bind loopback");
        let address = runtime.address();
        drop(runtime);
        address
    };

    let listener = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match tokio::net::TcpListener::bind(address).await {
                Ok(listener) => break listener,
                Err(_) => tokio::task::yield_now().await,
            }
        }
    })
    .await
    .expect("dropped backend runtime should release its listener");
    drop(listener);
}
