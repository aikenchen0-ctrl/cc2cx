use axum::{
    body::Body,
    http::{Response, StatusCode},
    routing::post,
    Router,
};
use cc_launch_lib::cursor::protocol::{
    connect::encode_message,
    proto::{agent::v1 as agent, aiserver::v1 as ai},
    run_sse::decode_response_body,
};
use cc_launch_lib::{AppState, AppType, Database, Provider, ProviderMeta};
use prost::Message;
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tempfile::tempdir;
use tokio::{net::TcpListener, sync::oneshot, task::JoinHandle};

use cc_launch_lib::cursor::{
    harness::{CursorEntryMode, CursorHarness, CursorIntegrationState},
    mitm::CursorProxyConfig,
};

struct UnauthorizedProviderFixture {
    address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), std::io::Error>>,
}

impl UnauthorizedProviderFixture {
    async fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("provider fixture should bind");
        let address = listener.local_addr().expect("provider fixture address");
        let (stop, stopped) = oneshot::channel();
        let router = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                Response::builder()
                    .status(StatusCode::UNAUTHORIZED)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"error":{"message":"fixture unauthorized"}}"#,
                    ))
                    .expect("fixture response")
            })
            .get(|| async { StatusCode::METHOD_NOT_ALLOWED }),
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
        format!("http://{}/v1/chat/completions", self.address)
    }
}

impl Drop for UnauthorizedProviderFixture {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.task.abort();
    }
}

fn harness_bidi_request(request_id: &str) -> ai::BidiAppendRequest {
    let message = agent::AgentClientMessage {
        message: Some(agent::agent_client_message::Message::RunRequest(
            agent::AgentRunRequest {
                requested_model: Some(agent::RequestedModel {
                    model_id: "fixture-model".into(),
                    ..Default::default()
                }),
                conversation_id: Some("fixture-conversation".into()),
                action: Some(agent::ConversationAction {
                    action: Some(agent::conversation_action::Action::UserMessageAction(
                        agent::UserMessageAction {
                            user_message: Some(agent::UserMessage {
                                text: "fixture request".into(),
                                message_id: "fixture-message".into(),
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
    ai::BidiAppendRequest {
        data: hex::encode(message.encode_to_vec()),
        request_id: Some(ai::BidiRequestId {
            request_id: request_id.into(),
        }),
        append_seqno: 0,
        ..Default::default()
    }
}

#[tokio::test]
async fn app_state_exposes_cursor_harness_disabled_by_default() {
    let db = Arc::new(Database::memory().expect("memory database"));
    let state = AppState::new(db);
    let status = state.cursor_harness.status().await.expect("cursor status");
    assert_eq!(
        status.state,
        cc_launch_lib::cursor::harness::CursorIntegrationState::Disabled
    );
}

#[tokio::test]
async fn initialize_ca_does_not_start_proxy_or_modify_settings() {
    let dir = tempdir().expect("temp dir");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings.clone(),
        backup.clone(),
        CursorProxyConfig::default(),
    );

    let status = harness.initialize_ca().await.expect("initialize CA");

    assert_eq!(status.state, CursorIntegrationState::Disabled);
    assert!(matches!(
        status.ca,
        cc_launch_lib::cursor::ca::CaState::Untrusted | cc_launch_lib::cursor::ca::CaState::Ready
    ));
    assert!(status.ca_install_command.is_some());
    assert!(status.ca_uninstall_command.is_some());
    assert!(!status.settings_applied);
    assert!(!status.settings_backup_present);
    assert!(!settings.exists());
    assert!(!backup.exists());
}

#[tokio::test]
async fn concurrent_ca_initialization_keeps_a_matching_pair() {
    let dir = tempdir().expect("temp dir");
    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig::default(),
    );

    let (left, right) = tokio::join!(harness.initialize_ca(), harness.initialize_ca());
    left.expect("first initialization");
    right.expect("second initialization");
    harness
        .ca_manager()
        .load()
        .expect("CA pair must remain loadable");
}

#[tokio::test]
async fn uninstall_ca_requires_stopped_integration() {
    let dir = tempdir().expect("temp dir");
    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            ..CursorProxyConfig::default()
        },
    );
    harness.start().await.expect("start proxy");
    let error = harness
        .uninstall_ca()
        .await
        .expect_err("running integration must retain CA");
    assert!(error.to_string().contains("停止 Cursor"));
    harness.stop().await.expect("stop proxy");
    #[cfg(target_os = "windows")]
    harness.ca_manager().uninstall().expect("remove test CA");
}

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn start_does_not_require_ca_trust_installation() {
    let dir = tempdir().expect("temp dir");
    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            ..CursorProxyConfig::default()
        },
    );

    let status = harness
        .start()
        .await
        .expect("untrusted CA must not block local proxy startup");
    assert_eq!(status.state, CursorIntegrationState::Degraded);
    assert_eq!(status.ca, cc_launch_lib::cursor::ca::CaState::Untrusted);
    assert!(status.settings_applied);

    harness.stop().await.expect("stop proxy");
}

#[tokio::test]
async fn start_does_not_install_ca_without_explicit_action() {
    let dir = tempdir().expect("temp dir");
    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            ..CursorProxyConfig::default()
        },
    );

    let status = harness.start().await.expect("start proxy");
    assert_eq!(status.ca, cc_launch_lib::cursor::ca::CaState::Untrusted);
    assert_eq!(status.state, CursorIntegrationState::Degraded);
    harness.stop().await.expect("stop proxy");
    assert_eq!(
        harness.ca_manager().state().expect("read CA state"),
        cc_launch_lib::cursor::ca::CaState::Untrusted
    );
}

#[tokio::test]
async fn stale_settings_transaction_is_recovered_before_a_new_session() {
    let dir = tempdir().expect("temp dir");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, br#"{"editor.fontSize":14}"#).unwrap();

    let first = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings.clone(),
        backup.clone(),
        CursorProxyConfig::default(),
    );
    first
        .ca_manager()
        .initialize()
        .expect("initialize CA material");
    let store =
        cc_launch_lib::cursor::settings::CursorSettingsStore::new(settings.clone(), backup.clone());
    store
        .apply_proxy("http://127.0.0.1:43121")
        .expect("apply proxy");
    assert!(backup.exists());

    let second = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings.clone(),
        backup.clone(),
        CursorProxyConfig::default(),
    );
    second
        .recover_stale_settings()
        .await
        .expect("recover stale settings");
    assert_eq!(
        std::fs::read(&settings).unwrap(),
        br#"{"editor.fontSize":14}"#
    );
    assert!(!backup.exists());
}

#[tokio::test]
async fn explicit_backend_lifecycle_is_rolled_back_with_harness_stop() {
    let dir = tempdir().expect("temp dir");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);
    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );

    let started = harness.start().await.expect("start explicit backend");
    assert!(started.settings_applied);
    assert!(started.proxy_port.is_some());

    harness.stop().await.expect("stop explicit backend");
    let released = TcpListener::bind(("127.0.0.1", backend_port)).await;
    assert!(released.is_ok(), "backend port must be released on stop");
}

#[tokio::test]
async fn database_backed_harness_starts_provider_backend_from_current_provider() {
    let fixture = UnauthorizedProviderFixture::start().await;
    let dir = tempdir().expect("temp dir");
    let db = Arc::new(Database::memory().expect("memory database"));
    let mut provider = Provider::with_id(
        "cursor-current".into(),
        "Cursor current provider".into(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": fixture.url(),
                "ANTHROPIC_AUTH_TOKEN": "fixture-key"
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: Some("openai_chat".into()),
        is_full_url: Some(false),
        ..Default::default()
    });
    db.save_provider(AppType::Claude.as_str(), &provider)
        .expect("save current Cursor provider");
    db.set_current_provider(AppType::Claude.as_str(), &provider.id)
        .expect("select current Cursor provider");

    let harness = CursorHarness::new_with_database(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            ..CursorProxyConfig::default()
        },
        db,
    );

    let status = harness
        .start()
        .await
        .expect("database-backed Cursor harness should start");
    assert!(
        status.backend_port.is_some(),
        "a configured current provider must start the local protocol backend"
    );
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn database_backed_harness_reports_provider_auth_failure_through_real_backend() {
    let fixture = UnauthorizedProviderFixture::start().await;
    let dir = tempdir().expect("temp dir");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    let original_settings = br#"{"editor.fontSize":14}
"#;
    std::fs::write(&settings, original_settings).expect("seed Cursor settings");

    let db = Arc::new(Database::memory().expect("memory database"));
    let mut provider = Provider::with_id(
        "cursor-auth-failure".into(),
        "Cursor auth failure fixture".into(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": fixture.url(),
                "ANTHROPIC_AUTH_TOKEN": "fixture-only-key",
                "ANTHROPIC_MODEL": "fixture-model"
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: Some("openai_chat".into()),
        is_full_url: Some(true),
        ..Default::default()
    });
    db.save_provider(AppType::Claude.as_str(), &provider)
        .expect("save auth failure provider");
    db.set_current_provider(AppType::Claude.as_str(), &provider.id)
        .expect("select auth failure provider");

    let harness = CursorHarness::new_with_database(
        dir.path().join("cursor-ca"),
        settings.clone(),
        backup.clone(),
        CursorProxyConfig {
            requested_port: 0,
            ..Default::default()
        },
        db,
    );

    let started = harness.start().await.expect("start Cursor harness");
    let backend_port = started.backend_port.expect("backend port");
    assert!(started.settings_applied);
    assert!(backup.exists());

    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("loopback test client");
    let request_id = "harness-auth-failure";
    let append = client
        .post(format!(
            "http://127.0.0.1:{backend_port}/aiserver.v1.BidiService/BidiAppend"
        ))
        .header("content-type", "application/connect+proto")
        .body(encode_message(&harness_bidi_request(request_id)).expect("encode append"))
        .send()
        .await
        .expect("send BidiAppend");
    assert_eq!(append.status(), reqwest::StatusCode::OK);
    let _ = append.bytes().await.expect("read append");

    let run = client
        .post(format!(
            "http://127.0.0.1:{backend_port}/agent.v1.AgentService/RunSSE"
        ))
        .header("content-type", "application/connect+proto")
        .body(
            encode_message(&ai::BidiRequestId {
                request_id: request_id.into(),
            })
            .expect("encode RunSSE"),
        )
        .send()
        .await
        .expect("send RunSSE");
    assert_eq!(run.status(), reqwest::StatusCode::OK);
    let run_body = run.bytes().await.expect("read RunSSE");
    let frames = decode_response_body(&run_body).expect("decode RunSSE");
    assert!(
        !frames.is_empty(),
        "provider auth failure should terminate stream"
    );

    let health = client
        .get(format!("http://127.0.0.1:{backend_port}/health"))
        .send()
        .await
        .expect("query backend health");
    assert_eq!(health.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let health_body: serde_json::Value = health.json().await.expect("decode health");
    assert_eq!(health_body["provider"]["health"], "auth_failed");
    assert_eq!(health_body["provider"]["last_error_code"], "provider_auth");

    let degraded = harness.status().await.expect("query harness status");
    assert_eq!(degraded.state, CursorIntegrationState::Degraded);
    assert_eq!(
        degraded.backend_health,
        Some(cc_launch_lib::cursor::protocol_backend::CursorBackendHealth::AuthFailed)
    );
    assert_eq!(
        degraded.backend_error_code.as_deref(),
        Some("provider_auth")
    );

    harness.stop().await.expect("stop Cursor harness");
    assert_eq!(
        std::fs::read(&settings).expect("read restored settings"),
        original_settings
    );
    assert!(
        !backup.exists(),
        "settings backup must be removed after stop"
    );
    assert!(
        TcpListener::bind(("127.0.0.1", backend_port)).await.is_ok(),
        "backend listener must be released after stop"
    );
}

#[tokio::test]
async fn database_backed_harness_rolls_back_when_startup_probe_fails() {
    let dir = tempdir().expect("temp dir");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    let original_settings = br#"{"editor.fontSize":14}
"#;
    std::fs::write(&settings, original_settings).expect("seed Cursor settings");

    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);
    let proxy_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let proxy_port = proxy_probe.local_addr().unwrap().port();
    drop(proxy_probe);

    let db = Arc::new(Database::memory().expect("memory database"));
    let mut provider = Provider::with_id(
        "cursor-startup-probe-failure".into(),
        "Cursor startup probe failure fixture".into(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": "http://127.0.0.1:9/chat/completions",
                "ANTHROPIC_AUTH_TOKEN": "fixture-only-key",
                "ANTHROPIC_MODEL": "fixture-model"
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: Some("openai_chat".into()),
        is_full_url: Some(true),
        ..Default::default()
    });
    db.save_provider(AppType::Claude.as_str(), &provider)
        .expect("save startup probe provider");
    db.set_current_provider(AppType::Claude.as_str(), &provider.id)
        .expect("select startup probe provider");

    let harness = CursorHarness::new_with_database(
        dir.path().join("cursor-ca"),
        settings.clone(),
        backup.clone(),
        CursorProxyConfig {
            requested_port: proxy_port,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..Default::default()
        },
        db,
    );

    let error = harness
        .start()
        .await
        .expect_err("startup probe failure must block Cursor integration");
    assert!(error.to_string().contains("启动前探测"));
    assert_eq!(
        harness.status().await.unwrap().state,
        CursorIntegrationState::Disabled
    );
    assert_eq!(std::fs::read(&settings).unwrap(), original_settings);
    assert!(!backup.exists());
    assert!(TcpListener::bind(("127.0.0.1", backend_port)).await.is_ok());
    assert!(TcpListener::bind(("127.0.0.1", proxy_port)).await.is_ok());
}

#[tokio::test]
async fn harness_rolls_back_hosts_when_backend_preflight_fails() {
    let dir = tempdir().expect("temp dir");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    let hosts = dir.path().join("hosts");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    let original_settings = br#"{"editor.fontSize":14}
"#;
    let original_hosts = b"127.0.0.1 localhost\n";
    std::fs::write(&settings, original_settings).expect("seed Cursor settings");
    std::fs::write(&hosts, original_hosts).expect("seed hosts");

    let db = Arc::new(Database::memory().expect("memory database"));
    let mut provider = Provider::with_id(
        "cursor-hosts-probe-failure".into(),
        "Cursor hosts probe failure fixture".into(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": "http://127.0.0.1:9/chat/completions",
                "ANTHROPIC_AUTH_TOKEN": "fixture-only-key",
                "ANTHROPIC_MODEL": "fixture-model"
            }
        }),
        None,
    );
    provider.meta = Some(ProviderMeta {
        api_format: Some("openai_chat".into()),
        is_full_url: Some(true),
        ..Default::default()
    });
    db.save_provider(AppType::Claude.as_str(), &provider)
        .expect("save startup probe provider");
    db.set_current_provider(AppType::Claude.as_str(), &provider.id)
        .expect("select startup probe provider");

    let harness = CursorHarness::new_with_database(
        dir.path().join("cursor-ca"),
        settings.clone(),
        backup.clone(),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some("127.0.0.1:0".parse().unwrap()),
            ..Default::default()
        },
        db,
    );
    harness.enable_transparent_entry(hosts.clone(), 0);

    let error = harness
        .start()
        .await
        .expect_err("startup probe failure must roll back hosts");
    assert!(error.to_string().contains("启动前探测"));
    assert_eq!(std::fs::read(&hosts).unwrap(), original_hosts);
    assert_eq!(std::fs::read(&settings).unwrap(), original_settings);
    assert!(!harness.status().await.unwrap().managed_hosts);
}

#[tokio::test]
async fn harness_status_exposes_transparent_entry_state() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    std::fs::write(&hosts, b"127.0.0.1 localhost\n").expect("seed hosts");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts, 0);

    let status = harness.start().await.expect("start transparent harness");
    assert!(status.transparent_entry.is_some());
    assert!(!status.fake_ip_entries.is_empty());
    assert!(status.managed_hosts);
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn transparent_mode_does_not_start_http_proxy_or_write_proxy_settings() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    let settings = dir.path().join("Cursor/User/settings.json");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    std::fs::write(&hosts, b"127.0.0.1 localhost\n").expect("seed hosts");
    std::fs::write(&settings, b"{}\n").expect("seed settings");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings.clone(),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts, 0);

    let status = harness.start().await.expect("start transparent mode");
    assert!(status.managed_hosts);
    assert!(status.transparent_entry.is_some());
    assert!(
        status.proxy_url.is_none(),
        "mode T must not start MITM, got {:?}",
        status.proxy_url
    );
    assert!(!status.settings_applied);
    let settings_text = std::fs::read_to_string(&settings).expect("read settings");
    assert!(
        !settings_text.contains("http.proxy"),
        "mode T must not write http.proxy: {settings_text}"
    );
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn transparent_mode_clears_leftover_proxy_settings() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    let settings = dir.path().join("Cursor/User/settings.json");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    std::fs::write(&hosts, b"127.0.0.1 localhost\n").expect("seed hosts");
    std::fs::write(
        &settings,
        br#"{
  "window.autoDetectColorScheme": true,
  "http.proxy": "http://127.0.0.1:4387",
  "http.proxyKerberosServicePrincipal": "http://127.0.0.1:4387",
  "http.proxySupport": "on",
  "http.experimental.systemCertificatesV2": true,
  "cursor.general.disableHttp2": true
}
"#,
    )
    .expect("seed leftover proxy settings");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings.clone(),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts, 0);

    let status = harness.start().await.expect("start transparent mode");
    assert!(status.proxy_url.is_none());
    let settings_text = std::fs::read_to_string(&settings).expect("read settings");
    assert!(
        !settings_text.contains("http.proxy"),
        "mode T must strip leftover http.proxy: {settings_text}"
    );
    assert!(
        !settings_text.contains("disableHttp2"),
        "mode T must strip leftover disableHttp2: {settings_text}"
    );
    assert!(
        settings_text.contains("window.autoDetectColorScheme"),
        "mode T must keep unrelated settings: {settings_text}"
    );
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn transparent_mode_starts_when_settings_backup_is_unreadable() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    std::fs::write(&hosts, b"127.0.0.1 localhost\n").expect("seed hosts");
    std::fs::write(&settings, b"{}\n").expect("seed settings");
    std::fs::create_dir(&backup).expect("unreadable backup as directory");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings,
        backup,
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts, 0);

    let status = harness
        .start()
        .await
        .expect("mode T must start even if leftover backup cannot be read");
    assert!(status.transparent_entry.is_some());
    assert!(status.proxy_url.is_none());
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn proxy_mode_does_not_write_hosts() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    let original_hosts = b"127.0.0.1 localhost\n";
    std::fs::write(&hosts, original_hosts).expect("seed hosts");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );

    let status = harness.start().await.expect("start proxy mode");
    assert!(!status.managed_hosts);
    assert!(status.transparent_entry.is_none());
    assert!(status.proxy_url.is_some());
    assert_eq!(std::fs::read(&hosts).unwrap(), original_hosts);
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn harness_maps_agent_run_host_to_fake_ip() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    std::fs::write(&hosts, b"127.0.0.1 localhost\n").expect("seed hosts");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts, 0);

    let status = harness.start().await.expect("start transparent harness");
    let mapped: Vec<_> = status
        .fake_ip_entries
        .iter()
        .map(|entry| entry.hostname.as_str())
        .collect();
    assert!(
        mapped.contains(&"agentn.global.api5.cursor.sh"),
        "Agent Run host must have a fake-IP, got {mapped:?}"
    );
    assert!(
        mapped.contains(&"agentn.api5.cursor.sh"),
        "Agent default host must have a fake-IP, got {mapped:?}"
    );
    harness.stop().await.expect("stop Cursor harness");
}

#[tokio::test]
async fn harness_maps_dns_cname_aliases_used_by_live_cursor() {
    let dir = tempdir().expect("temp dir");
    let hosts = dir.path().join("hosts");
    std::fs::write(&hosts, b"127.0.0.1 localhost\n").expect("seed hosts");
    let backend_probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let backend_port = backend_probe.local_addr().unwrap().port();
    drop(backend_probe);

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        dir.path().join("Cursor/User/settings.json"),
        dir.path().join("cursor-settings-backup.json"),
        CursorProxyConfig {
            requested_port: 0,
            backend: Some(format!("127.0.0.1:{backend_port}").parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts, 0);

    let status = harness.start().await.expect("start transparent harness");
    let mapped: Vec<_> = status
        .fake_ip_entries
        .iter()
        .map(|entry| entry.hostname.as_str())
        .collect();
    for host in [
        "api2geo.cursor.sh",
        "api2direct.cursor.sh",
        "agentn.global.api5geo.cursor.sh",
        "agentn.global.api5lat.cursor.sh",
    ] {
        assert!(
            mapped.contains(&host),
            "CNAME alias {host} must have a fake-IP, got {mapped:?}"
        );
    }
    harness.stop().await.expect("stop Cursor harness");
}

#[test]
fn default_harness_uses_proxy_mode_without_system_hosts() {
    let harness = CursorHarness::default_for_current_user();
    assert_eq!(harness.entry_mode(), CursorEntryMode::Proxy);
    assert_eq!(harness.transparent_hosts_path(), None);
    assert_eq!(harness.transparent_listen_port(), 0);
}

#[tokio::test]
async fn harness_start_fails_closed_when_hosts_not_writable() {
    let dir = tempdir().expect("temp dir");
    let settings = dir.path().join("Cursor/User/settings.json");
    let backup = dir.path().join("cursor-settings-backup.json");
    let hosts = dir.path().join("hosts");
    std::fs::create_dir_all(settings.parent().expect("settings parent")).expect("settings dir");
    let original_hosts = b"127.0.0.1 localhost\n";
    std::fs::write(&hosts, original_hosts).expect("seed hosts");
    let mut permissions = std::fs::metadata(&hosts).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&hosts, permissions).unwrap();

    let harness = CursorHarness::new(
        dir.path().join("cursor-ca"),
        settings,
        backup,
        CursorProxyConfig {
            requested_port: 0,
            backend: Some("127.0.0.1:1".parse().unwrap()),
            ..CursorProxyConfig::default()
        },
    );
    harness.enable_transparent_entry(hosts.clone(), 0);

    let error = harness
        .start()
        .await
        .expect_err("unwritable hosts must fail closed");
    assert!(error.to_string().contains("提升权限"));
    assert_eq!(std::fs::read(&hosts).unwrap(), original_hosts);
    let status = harness.status().await.unwrap();
    assert!(!status.managed_hosts);
    assert!(status.transparent_entry.is_none());
}
