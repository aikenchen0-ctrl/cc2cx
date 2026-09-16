use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use bytes::Bytes;
use cc_launch_lib::cursor::{
    adapter::{
        CursorAction, CursorRequest, ProviderEvent, ProviderInvocation, ProviderStreamError,
    },
    provider::{
        AnthropicAuth, AnthropicMessagesConfig, AnthropicMessagesProvider, CursorModelPolicy,
        OpenAiChatConfig, OpenAiChatProvider,
    },
    provider_factory::CursorProviderFactory,
};
use cc_launch_lib::{AppType, Database, Provider, ProviderMeta};
use futures::StreamExt;
use serde_json::Value;
use std::{net::SocketAddr, time::Duration};
use tokio::{
    net::TcpListener,
    sync::{oneshot, Mutex},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

#[path = "support.rs"]
mod support;

fn invocation() -> ProviderInvocation {
    ProviderInvocation::from_request(CursorRequest {
        request_id: "request-provider-fixture".into(),
        append_seqno: 0,
        conversation_id: Some("conversation-provider-fixture".into()),
        model_id: Some("model-provider-fixture".into()),
        user_message: Some("hello provider fixture".into()),
        user_message_id: Some("message-provider-fixture".into()),
        action: CursorAction::Run,
        field_dispositions: Vec::new(),
    })
    .expect("fixture invocation must be valid")
}

struct MockServer {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl MockServer {
    async fn start(
        status: StatusCode,
        content_type: Option<&str>,
        response_body: impl Into<Bytes>,
        request_tx: Option<oneshot::Sender<Value>>,
    ) -> Self {
        Self::start_with_delay(status, content_type, response_body, request_tx, None).await
    }

    async fn start_delayed(
        status: StatusCode,
        content_type: Option<&str>,
        response_body: impl Into<Bytes>,
        request_tx: Option<oneshot::Sender<Value>>,
        delay: Duration,
    ) -> Self {
        Self::start_with_delay(status, content_type, response_body, request_tx, Some(delay)).await
    }

    async fn start_with_delay(
        status: StatusCode,
        content_type: Option<&str>,
        response_body: impl Into<Bytes>,
        request_tx: Option<oneshot::Sender<Value>>,
        delay: Option<Duration>,
    ) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("mock server should bind");
        let address = listener.local_addr().expect("mock address");
        let (stop, stopped) = oneshot::channel();
        let response_body = response_body.into();
        let request_tx = std::sync::Arc::new(Mutex::new(request_tx));
        let content_type = content_type.map(str::to_owned);
        let route = any(move |request: Request<Body>| {
            let request_tx = request_tx.clone();
            let response_body = response_body.clone();
            let content_type = content_type.clone();
            async move {
                if let Some(delay) = delay {
                    tokio::time::sleep(delay).await;
                }
                if let Some(request_tx) = request_tx.lock().await.take() {
                    let body = to_bytes(request.into_body(), 1024 * 1024)
                        .await
                        .expect("request body");
                    let value = serde_json::from_slice(&body).expect("request JSON");
                    let _ = request_tx.send(value);
                }
                let mut response = Response::builder().status(status);
                if let Some(content_type) = content_type {
                    response = response.header("content-type", content_type);
                }
                response
                    .body(Body::from(response_body))
                    .expect("mock response")
            }
        });
        let router = Router::new().route("/chat", route);
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
        format!("http://{}/chat", self.address)
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}

fn provider(server: &MockServer) -> OpenAiChatProvider {
    OpenAiChatProvider::new(OpenAiChatConfig {
        request_url: server.url(),
        api_key: "test-key".into(),
        custom_headers: Default::default(),
        timeout: Duration::from_secs(2),
    })
    .expect("provider config")
}

fn configured_provider(
    base_url: &str,
    api_key: Option<&str>,
    api_format: Option<&str>,
) -> Provider {
    configured_provider_with_full_url(base_url, api_key, api_format, true)
}

fn configured_provider_with_full_url(
    base_url: &str,
    api_key: Option<&str>,
    api_format: Option<&str>,
    is_full_url: bool,
) -> Provider {
    let mut provider = Provider::with_id(
        "provider-factory-fixture".into(),
        "Provider factory fixture".into(),
        serde_json::json!({
            "env": {
                "ANTHROPIC_BASE_URL": base_url,
                "ANTHROPIC_AUTH_TOKEN": api_key.unwrap_or("")
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: api_format.map(str::to_owned),
        is_full_url: Some(is_full_url),
        ..Default::default()
    });
    provider
}

fn configured_provider_with_cursor_model_policy(
    base_url: &str,
    api_key: &str,
    routes: &[(&str, &str)],
    default_model: Option<&str>,
) -> Provider {
    let mut provider =
        configured_provider_with_full_url(base_url, Some(api_key), Some("openai_chat"), true);
    let meta = provider.meta.get_or_insert_with(ProviderMeta::default);
    meta.cursor_model_routes = routes
        .iter()
        .map(|(requested, upstream)| ((*requested).to_string(), (*upstream).to_string()))
        .collect();
    meta.cursor_default_model = default_model.map(str::to_owned);
    provider
}

fn save_current_provider(db: &Database, app_type: AppType, provider: Provider) {
    db.save_provider(app_type.as_str(), &provider)
        .expect("save current provider fixture");
    db.set_current_provider(app_type.as_str(), &provider.id)
        .expect("select current provider fixture");
}

#[test]
fn readonly_current_provider_prefers_valid_local_selection_over_database_marker() {
    let _guard = support::test_mutex().lock().expect("acquire test mutex");
    support::reset_test_fs();

    let db = Database::memory().expect("memory database");
    let mut local_provider = configured_provider(
        "https://local-provider.example/chat",
        Some("fixture-local-key"),
        Some("openai_chat"),
    );
    local_provider.id = "local-provider".into();
    let mut database_provider = configured_provider(
        "https://database-provider.example/chat",
        Some("fixture-database-key"),
        Some("openai_chat"),
    );
    database_provider.id = "database-provider".into();
    db.save_provider(AppType::ClaudeDesktop.as_str(), &local_provider)
        .expect("save local provider fixture");
    save_current_provider(&db, AppType::ClaudeDesktop, database_provider);

    cc_launch_lib::update_settings(cc_launch_lib::AppSettings {
        current_provider_claude_desktop: Some("local-provider".into()),
        ..cc_launch_lib::AppSettings::default()
    })
    .expect("write local provider selection");

    let selected =
        cc_launch_lib::get_effective_current_provider_readonly(&db, &AppType::ClaudeDesktop)
            .expect("resolve current provider");

    assert_eq!(selected.as_deref(), Some("local-provider"));
}

#[test]
fn readonly_current_provider_falls_back_from_stale_local_selection_without_clearing_it() {
    let _guard = support::test_mutex().lock().expect("acquire test mutex");
    support::reset_test_fs();
    let home = support::ensure_test_home();

    cc_launch_lib::update_settings(cc_launch_lib::AppSettings {
        current_provider_claude_desktop: Some("stale-local-provider".into()),
        ..cc_launch_lib::AppSettings::default()
    })
    .expect("write stale local provider selection");

    let db = Database::memory().expect("memory database");
    let mut database_provider = configured_provider(
        "https://database-provider.example/chat",
        Some("fixture-database-key"),
        Some("openai_chat"),
    );
    database_provider.id = "database-provider".into();
    save_current_provider(&db, AppType::ClaudeDesktop, database_provider);

    let selected =
        cc_launch_lib::get_effective_current_provider_readonly(&db, &AppType::ClaudeDesktop)
            .expect("resolve current provider");

    assert_eq!(selected.as_deref(), Some("database-provider"));
    let settings_path = home.join(".cc-launch").join("settings.json");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path).expect("read settings"))
            .expect("parse settings");
    assert_eq!(
        settings
            .get("currentProviderClaudeDesktop")
            .and_then(serde_json::Value::as_str),
        Some("stale-local-provider")
    );
}

#[test]
fn readonly_current_provider_uses_database_marker_when_local_selection_is_absent() {
    let _guard = support::test_mutex().lock().expect("acquire test mutex");
    support::reset_test_fs();

    let db = Database::memory().expect("memory database");
    let mut database_provider = configured_provider(
        "https://database-provider.example/chat",
        Some("fixture-database-key"),
        Some("openai_chat"),
    );
    database_provider.id = "database-provider".into();
    save_current_provider(&db, AppType::ClaudeDesktop, database_provider);

    let selected =
        cc_launch_lib::get_effective_current_provider_readonly(&db, &AppType::ClaudeDesktop)
            .expect("resolve current provider");

    assert_eq!(selected.as_deref(), Some("database-provider"));
}

#[test]
fn cursor_provider_factory_does_not_clear_stale_local_provider_setting() {
    let _guard = support::test_mutex().lock().expect("acquire test mutex");
    support::reset_test_fs();
    let home = support::ensure_test_home();

    cc_launch_lib::update_settings(cc_launch_lib::AppSettings {
        current_provider_claude_desktop: Some("stale-local-provider".into()),
        ..cc_launch_lib::AppSettings::default()
    })
    .expect("write isolated stale local provider setting");

    let db = Database::memory().expect("memory database");
    let provider = configured_provider(
        "https://provider.example/chat",
        Some("fixture-current-key"),
        Some("openai_chat"),
    );
    save_current_provider(&db, AppType::ClaudeDesktop, provider);

    CursorProviderFactory::from_current(&db, &AppType::ClaudeDesktop)
        .expect("database current provider should be used without mutating local settings");

    let settings_path = home.join(".cc-launch").join("settings.json");
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path).expect("read settings"))
            .expect("parse settings");
    assert_eq!(
        settings
            .get("currentProviderClaudeDesktop")
            .and_then(serde_json::Value::as_str),
        Some("stale-local-provider"),
        "Cursor provider selection must not clear a stale local setting"
    );
}

#[tokio::test]
async fn cursor_provider_factory_builds_source_from_database_current_provider() {
    let server = MockServer::start(
        StatusCode::OK,
        Some("text/event-stream"),
        "data: [DONE]\n\n",
        None,
    )
    .await;
    let db = Database::memory().expect("memory database");
    let mut provider = configured_provider(
        server.url().as_str(),
        Some("fixture-current-key"),
        Some("openai_chat"),
    );
    provider.settings_config["env"]["ANTHROPIC_MODEL"] =
        serde_json::json!("model-provider-fixture");
    save_current_provider(&db, AppType::ClaudeDesktop, provider);

    let source = CursorProviderFactory::from_current(&db, &AppType::ClaudeDesktop)
        .expect("database current provider should build a source");
    let events: Vec<_> = source
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    assert_eq!(events, vec![Ok(ProviderEvent::Done)]);
}

#[test]
fn cursor_provider_factory_rejects_missing_database_current_provider() {
    let db = Database::memory().expect("memory database");

    let error = CursorProviderFactory::from_current(&db, &AppType::ClaudeDesktop)
        .err()
        .expect("missing database current provider must fail explicitly");

    assert!(error.to_string().contains("current provider"));
}

#[test]
fn cursor_provider_factory_rejects_incompatible_database_current_provider() {
    let db = Database::memory().expect("memory database");
    let provider = configured_provider(
        "https://provider.example/messages",
        Some("fixture-current-key"),
        Some("gemini"),
    );
    save_current_provider(&db, AppType::ClaudeDesktop, provider);

    let error = CursorProviderFactory::from_current(&db, &AppType::ClaudeDesktop)
        .err()
        .expect("incompatible current provider must fail explicitly");

    assert!(
        error.to_string().contains("api_format") || error.to_string().contains("不支持"),
        "got {error}"
    );
}

#[test]
fn cursor_provider_factory_builds_default_claude_provider_as_anthropic_messages() {
    let mut provider = configured_provider(
        "https://provider.example/messages",
        Some("fixture-anthropic-key"),
        None,
    );
    provider.settings_config["env"]["ANTHROPIC_MODEL"] = serde_json::json!("claude-sonnet-fixture");
    provider.meta = Some(ProviderMeta::default());
    let db = Database::memory().expect("memory database");
    save_current_provider(&db, AppType::Claude, provider);

    let source = CursorProviderFactory::from_current(&db, &AppType::Claude)
        .expect("Claude providers should use the Anthropic Messages bridge");
    assert!(!source.model_ids().is_empty());
}

#[test]
fn cursor_provider_factory_rejects_database_current_provider_without_auth() {
    let db = Database::memory().expect("memory database");
    let provider = configured_provider("https://provider.example/chat", None, Some("openai_chat"));
    save_current_provider(&db, AppType::ClaudeDesktop, provider);

    let error = CursorProviderFactory::from_current(&db, &AppType::ClaudeDesktop)
        .err()
        .expect("current provider without auth must fail explicitly");

    assert!(error.to_string().contains("authentication"));
}

#[test]
fn cursor_model_mapping_prefers_explicit_alias_over_provider_default() {
    let provider = configured_provider_with_cursor_model_policy(
        "https://provider.example/chat",
        "fixture-key",
        &[("claude-sonnet", "upstream-sonnet")],
        Some("upstream-default"),
    );
    let source = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect("explicit model policy should build");

    let resolved = source
        .prepare_invocation(invocation())
        .expect("explicit alias should resolve");
    assert_eq!(resolved.requested_model, "model-provider-fixture");
    assert_eq!(resolved.model_id, "upstream-default");

    let mut aliased = invocation();
    aliased.model_id = "claude-sonnet".into();
    let resolved = source
        .prepare_invocation(aliased)
        .expect("explicit alias should resolve");
    assert_eq!(resolved.requested_model, "claude-sonnet");
    assert_eq!(resolved.model_id, "upstream-sonnet");
}

#[test]
fn cursor_model_mapping_keeps_exact_configured_upstream_model() {
    let provider = configured_provider_with_cursor_model_policy(
        "https://provider.example/chat",
        "fixture-key",
        &[("claude-sonnet", "upstream-sonnet")],
        Some("upstream-default"),
    );
    let source = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect("explicit model policy should build");

    let mut exact = invocation();
    exact.model_id = "upstream-sonnet".into();
    let resolved = source
        .prepare_invocation(exact)
        .expect("configured upstream model should remain unchanged");
    assert_eq!(resolved.model_id, "upstream-sonnet");
}

#[test]
fn cursor_model_mapping_rejects_unknown_model_without_default() {
    let provider = configured_provider_with_cursor_model_policy(
        "https://provider.example/chat",
        "fixture-key",
        &[("claude-sonnet", "upstream-sonnet")],
        None,
    );
    let source = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect("explicit alias-only policy should build");
    let mut unknown = invocation();
    unknown.model_id = "unknown-model".into();

    let error = source
        .prepare_invocation(unknown)
        .expect_err("unknown model must be rejected without a configured default");
    assert_eq!(error.code, "provider_model");
    assert!(error.message.contains("unknown-model"));
}

#[tokio::test]
async fn cursor_provider_factory_builds_explicit_openai_chat_source() {
    let server = MockServer::start(
        StatusCode::OK,
        Some("text/event-stream"),
        "data: [DONE]\n\n",
        None,
    )
    .await;
    let mut provider = configured_provider(
        server.url().as_str(),
        Some("fixture-key"),
        Some("openai_chat"),
    );
    provider.settings_config["env"]["ANTHROPIC_MODEL"] =
        serde_json::json!("model-provider-fixture");

    let source = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect("explicit OpenAI Chat provider should build");
    let events: Vec<_> = source
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    assert_eq!(events, vec![Ok(ProviderEvent::Done)]);
}

#[test]
fn cursor_provider_factory_rejects_missing_url_without_exposing_credentials() {
    let provider = configured_provider("", Some("fixture-secret-key"), Some("openai_chat"));

    let error = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect_err("missing endpoint must fail before provider construction");
    let message = error.to_string();

    assert!(message.contains("base_url"));
    assert!(!message.contains("fixture-secret-key"));
}

#[test]
fn cursor_provider_factory_rejects_missing_auth_explicitly() {
    let provider = configured_provider("https://provider.example/chat", None, Some("openai_chat"));

    let error = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect_err("missing authentication must fail before provider construction");
    let message = error.to_string();

    assert!(message.contains("authentication"));
}

#[test]
fn cursor_provider_factory_rejects_endpoint_userinfo() {
    let provider = configured_provider(
        "https://fixture-user:fixture-password@provider.example/chat",
        Some("fixture-key"),
        Some("openai_chat"),
    );

    let error = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect_err("endpoint userinfo must not be accepted as provider configuration");

    assert!(error.to_string().contains("userinfo"));
}

#[test]
fn cursor_provider_factory_builds_endpoint_for_origin_provider_url() {
    let provider = configured_provider_with_full_url(
        "https://provider.example",
        Some("fixture-key"),
        Some("openai_chat"),
        false,
    );

    let source = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect("origin provider URL should build an OpenAI Chat source");

    let debug = format!("{source:?}");
    assert!(debug.contains("https://provider.example/v1/chat/completions"));
}

#[test]
fn cursor_provider_factory_rejects_non_chat_and_non_compatible_provider_shapes() {
    let anthropic = configured_provider(
        "https://provider.example/messages",
        Some("fixture-key"),
        Some("anthropic"),
    );
    let error = CursorProviderFactory::openai_chat(&anthropic, &AppType::Claude)
        .expect_err("Anthropic wire format must not be treated as Chat Completions");
    assert!(error.to_string().contains("openai_chat"));

    let gemini = configured_provider(
        "https://provider.example/chat",
        Some("fixture-key"),
        Some("openai_chat"),
    );
    let error = CursorProviderFactory::openai_chat(&gemini, &AppType::Gemini)
        .expect_err("Gemini provider must not be treated as OpenAI Chat");
    assert!(error.to_string().contains("不支持"));
}

#[test]
fn cursor_provider_factory_rejects_claude_legacy_openrouter_compat_without_explicit_format() {
    let mut provider =
        configured_provider("https://provider.example/chat", Some("fixture-key"), None);
    provider.settings_config["openrouter_compat_mode"] = serde_json::json!(true);

    let error = CursorProviderFactory::openai_chat(&provider, &AppType::Claude)
        .expect_err("legacy openrouter compatibility flag must not imply OpenAI Chat format");

    assert!(error.to_string().contains("api_format=openai_chat"));
}

#[tokio::test]
async fn openai_chat_stream_builds_request_and_emits_text_usage_and_done() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"hello\"}}]}\n\n",
        "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":7}}\n\n",
        "data: [DONE]\n\n"
    );
    let (request_tx, request_rx) = oneshot::channel();
    let server = MockServer::start(
        StatusCode::OK,
        Some("text/event-stream"),
        sse,
        Some(request_tx),
    )
    .await;

    let events: Vec<_> = provider(&server)
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;
    let request = request_rx.await.expect("provider request fixture");

    assert_eq!(request["model"], "model-provider-fixture");
    assert_eq!(request["messages"][0]["role"], "user");
    assert_eq!(request["messages"][0]["content"], "hello provider fixture");
    assert_eq!(request["stream"], true);
    assert_eq!(request["stream_options"]["include_usage"], true);
    assert_eq!(
        events,
        vec![
            Ok(ProviderEvent::TextDelta("hello".into())),
            Ok(ProviderEvent::Usage {
                input_tokens: 11,
                output_tokens: 7,
                cache_read_tokens: None,
                cache_write_tokens: None,
                reasoning_tokens: None,
            }),
            Ok(ProviderEvent::Done),
        ]
    );
}

#[tokio::test]
async fn openai_chat_stream_accepts_null_error_field_and_crlf_sse() {
    let sse = concat!(
        "data: {\"error\":null,\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\r\n\r\n",
        "data: [DONE]\r\n\r\n"
    );
    let server = MockServer::start(StatusCode::OK, Some("text/event-stream"), sse, None).await;

    let events: Vec<_> = provider(&server)
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    assert_eq!(
        events,
        vec![
            Ok(ProviderEvent::TextDelta("ok".into())),
            Ok(ProviderEvent::Done),
        ]
    );
}

#[tokio::test]
async fn openai_chat_stream_does_not_invent_done_on_eof_without_done_marker() {
    let sse = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"partial\"}}]}\n\n";
    let server = MockServer::start(StatusCode::OK, Some("text/event-stream"), sse, None).await;

    let events: Vec<_> = provider(&server)
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    assert_eq!(events, vec![Ok(ProviderEvent::TextDelta("partial".into()))]);
}

#[tokio::test]
async fn openai_chat_stream_preserves_tool_argument_delta_for_explicit_unsupported_mapping() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"function\":{\"name\":\"lookup\",\"arguments\":\"{\\\"q\\\":\\\"x\\\"}\"}}]}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let server = MockServer::start(StatusCode::OK, Some("text/event-stream"), sse, None).await;

    let events: Vec<_> = provider(&server)
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    assert_eq!(
        events,
        vec![
            Ok(ProviderEvent::ToolCallStart {
                call_id: "call-1".into(),
                name: "lookup".into(),
            }),
            Ok(ProviderEvent::ToolCallArgumentsDelta {
                call_id: "call-1".into(),
                delta: "{\"q\":\"x\"}".into(),
            }),
            Ok(ProviderEvent::Done),
        ]
    );
}

#[tokio::test]
async fn openai_chat_stream_reassembles_tool_identity_and_arguments_across_sse_events() {
    let sse = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"q\\\":\\\"\"}}]}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-2\",\"function\":{\"name\":\"lookup\",\"arguments\":\"x\\\"}\"}}]}}]}\n\n",
        "data: [DONE]\n\n"
    );
    let server = MockServer::start(StatusCode::OK, Some("text/event-stream"), sse, None).await;

    let events: Vec<_> = provider(&server)
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    assert_eq!(
        events,
        vec![
            Ok(ProviderEvent::ToolCallStart {
                call_id: "call-2".into(),
                name: "lookup".into(),
            }),
            Ok(ProviderEvent::ToolCallArgumentsDelta {
                call_id: "call-2".into(),
                delta: "{\"q\":\"x\"}".into(),
            }),
            Ok(ProviderEvent::Done),
        ]
    );
}

#[tokio::test]
async fn openai_chat_stream_classifies_http_failure_without_echoing_response_body() {
    let (request_tx, request_rx) = oneshot::channel();
    let server = MockServer::start(
        StatusCode::TOO_MANY_REQUESTS,
        None,
        "fixture-secret-response-body",
        Some(request_tx),
    )
    .await;

    let events: Vec<_> = provider(&server)
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;
    let _ = request_rx.await.expect("provider request fixture");

    let [Err(error)] = events.as_slice() else {
        panic!("HTTP errors must be one classified provider error");
    };
    assert_eq!(error.code, "provider_http");
    assert!(!error.message.contains("fixture-secret-response-body"));
    assert!(error.message.contains("429"));
}

#[tokio::test]
async fn openai_chat_stream_classifies_authentication_statuses_separately() {
    for status in [StatusCode::UNAUTHORIZED, StatusCode::FORBIDDEN] {
        let server = MockServer::start(status, None, "fixture-secret-response-body", None).await;

        let events: Vec<_> = provider(&server)
            .stream(invocation(), CancellationToken::new())
            .collect()
            .await;

        let [Err(error)] = events.as_slice() else {
            panic!("authentication failures must be one classified provider error");
        };
        assert_eq!(error.code, "provider_auth");
        assert!(error.message.contains(status.as_str()));
        assert!(!error.message.contains("fixture-secret-response-body"));
    }
}

#[tokio::test]
async fn anthropic_stream_classifies_forbidden_as_auth() {
    let server = MockServer::start(
        StatusCode::FORBIDDEN,
        None,
        "fixture-secret-response-body",
        None,
    )
    .await;
    let provider = AnthropicMessagesProvider::new(AnthropicMessagesConfig {
        request_url: server.url(),
        api_key: "fixture-key".into(),
        auth: AnthropicAuth::Bearer,
        ..Default::default()
    })
    .expect("anthropic fixture provider");
    let events: Vec<_> = provider
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;
    let [Err(error)] = events.as_slice() else {
        panic!("403 must be one classified provider error");
    };
    assert_eq!(error.code, "provider_auth", "{}", error.message);
    assert!(error.message.contains("403"));
    assert!(!error.message.contains("fixture-secret-response-body"));
}

#[tokio::test]
async fn anthropic_stream_maps_unknown_cursor_model_to_default() {
    let (request_tx, request_rx) = oneshot::channel();
    let server = MockServer::start(
        StatusCode::OK,
        Some("text/event-stream"),
        "data: {\"type\":\"message_stop\"}\n\n",
        Some(request_tx),
    )
    .await;
    let policy = CursorModelPolicy::strict(
        Vec::<(String, String)>::new(),
        ["claude-opus-4-6".to_string()],
        Some("claude-opus-4-6".to_string()),
    )
    .expect("strict policy");
    let provider = AnthropicMessagesProvider::new_with_model_policy(
        AnthropicMessagesConfig {
            request_url: server.url(),
            api_key: "fixture-key".into(),
            auth: AnthropicAuth::Bearer,
            ..Default::default()
        },
        policy,
    )
    .expect("anthropic fixture provider");
    let mut invocation = invocation();
    invocation.model_id = "cursor-composer".into();
    let _events: Vec<_> = provider
        .stream(invocation, CancellationToken::new())
        .collect()
        .await;
    let body = request_rx.await.expect("anthropic request body");
    assert_eq!(
        body.get("model").and_then(Value::as_str),
        Some("claude-opus-4-6")
    );
}

#[tokio::test]
async fn openai_chat_stream_classifies_request_timeout_separately() {
    let server = MockServer::start_delayed(
        StatusCode::OK,
        Some("text/event-stream"),
        "data: [DONE]\n\n",
        None,
        Duration::from_millis(100),
    )
    .await;
    let provider = OpenAiChatProvider::new(OpenAiChatConfig {
        request_url: server.url(),
        api_key: "test-key".into(),
        custom_headers: Default::default(),
        timeout: Duration::from_millis(20),
    })
    .expect("provider config");

    let events: Vec<_> = provider
        .stream(invocation(), CancellationToken::new())
        .collect()
        .await;

    let [Err(error)] = events.as_slice() else {
        panic!("request timeout must be one classified provider error");
    };
    assert_eq!(error.code, "provider_timeout");
    assert!(!error.message.contains("test-key"));
}

#[tokio::test]
async fn openai_chat_stream_stops_before_request_when_cancelled() {
    let (request_tx, request_rx) = oneshot::channel();
    let server = MockServer::start(
        StatusCode::OK,
        Some("text/event-stream"),
        "data: [DONE]\n\n",
        Some(request_tx),
    )
    .await;
    let cancellation = CancellationToken::new();
    cancellation.cancel();

    let events: Vec<_> = provider(&server)
        .stream(invocation(), cancellation)
        .collect()
        .await;

    assert!(events.is_empty());
    assert!(tokio::time::timeout(Duration::from_millis(50), request_rx)
        .await
        .is_err());
}

#[tokio::test]
async fn openai_chat_preflight_accepts_reachable_method_not_allowed() {
    let server = MockServer::start(
        StatusCode::METHOD_NOT_ALLOWED,
        Some("text/plain"),
        "method not allowed",
        None,
    )
    .await;
    let provider = provider(&server);

    provider
        .preflight()
        .await
        .expect("an HTTP response proves the endpoint is reachable");
}

#[tokio::test]
async fn openai_chat_preflight_classifies_authentication_failure_without_body() {
    let server = MockServer::start(
        StatusCode::UNAUTHORIZED,
        Some("application/json"),
        "fixture-secret-response-body",
        None,
    )
    .await;
    let provider = provider(&server);

    let error = provider
        .preflight()
        .await
        .expect_err("401 must fail the startup health gate");
    assert_eq!(error.code, "provider_auth");
    assert!(!error.message.contains("fixture-secret-response-body"));
}

#[tokio::test]
async fn openai_chat_preflight_classifies_transport_failure() {
    let provider = OpenAiChatProvider::new(OpenAiChatConfig {
        request_url: "http://127.0.0.1:1/chat/completions".into(),
        api_key: "test-key".into(),
        custom_headers: Default::default(),
        timeout: Duration::from_millis(50),
    })
    .expect("provider config");

    let error = provider
        .preflight()
        .await
        .expect_err("an unavailable endpoint must fail the startup health gate");
    assert!(matches!(
        error.code.as_str(),
        "provider_transport" | "provider_timeout"
    ));
    assert!(!error.message.contains("test-key"));
}

#[tokio::test]
async fn openai_chat_preflight_bypasses_env_proxy_for_loopback() {
    let server = MockServer::start(StatusCode::METHOD_NOT_ALLOWED, None, "", None).await;
    let previous_http = std::env::var("HTTP_PROXY").ok();
    let previous_https = std::env::var("HTTPS_PROXY").ok();
    std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:9");
    let result = provider(&server).preflight().await;
    match previous_http {
        Some(value) => std::env::set_var("HTTP_PROXY", value),
        None => std::env::remove_var("HTTP_PROXY"),
    }
    match previous_https {
        Some(value) => std::env::set_var("HTTPS_PROXY", value),
        None => std::env::remove_var("HTTPS_PROXY"),
    }
    result.expect("loopback preflight must not be sent through HTTP_PROXY");
}

#[allow(dead_code)]
fn _assert_error_type(_: ProviderStreamError) {}
