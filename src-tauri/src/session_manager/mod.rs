pub mod providers;
pub mod terminal;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use providers::{claude, codex, gemini, grokbuild, hermes, openclaw, opencode, pi};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub provider_id: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_command: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionRequest {
    pub provider_id: String,
    pub session_id: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteSessionOutcome {
    pub provider_id: String,
    pub session_id: String,
    pub source_path: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn scan_sessions() -> Vec<SessionMeta> {
    let (r1, r2, r3, r4, r5, r6, r7, r8, casr_discovered) = std::thread::scope(|s| {
        let h1 = s.spawn(codex::scan_sessions);
        let h2 = s.spawn(claude::scan_sessions);
        let h3 = s.spawn(opencode::scan_sessions);
        let h4 = s.spawn(openclaw::scan_sessions);
        let h5 = s.spawn(gemini::scan_sessions);
        let h6 = s.spawn(hermes::scan_sessions);
        let h7 = s.spawn(grokbuild::scan_sessions);
        let h8 = s.spawn(pi::scan_sessions);
        let casr_handle =
            s.spawn(|| casr::ProviderRegistry::default_registry().discover_sessions());
        (
            h1.join().unwrap_or_default(),
            h2.join().unwrap_or_default(),
            h3.join().unwrap_or_default(),
            h4.join().unwrap_or_default(),
            h5.join().unwrap_or_default(),
            h6.join().unwrap_or_default(),
            h7.join().unwrap_or_default(),
            h8.join().unwrap_or_default(),
            casr_handle.join().unwrap_or_default(),
        )
    });

    let mut sessions = Vec::new();
    sessions.extend(r1);
    sessions.extend(r2);
    sessions.extend(r3);
    sessions.extend(r4);
    sessions.extend(r5);
    sessions.extend(r6);
    sessions.extend(r7);
    sessions.extend(r8);

    let casr_sessions = casr_discovered_to_session_meta(casr_discovered);
    merge_session_results(sessions, casr_sessions)
        .into_iter()
        .map(|mut session| {
            if let Some(command) = session.resume_command.take() {
                session.resume_command = terminal::validate_resume_command(&command)
                    .ok()
                    .map(|_| command);
            }
            session
        })
        .collect()
}

fn casr_discovered_to_session_meta(discovered: Vec<casr::DiscoveredSession>) -> Vec<SessionMeta> {
    let registry = casr::ProviderRegistry::default_registry();
    discovered
        .into_iter()
        .map(|entry| {
            let session_id = entry.session.session_id.clone();
            let provider_id =
                crate::session_transfer::ui_provider_id_for_slug(&entry.provider_slug);
            let resume_command = registry
                .find_by_alias(&entry.provider_alias)
                .map(|provider| provider.resume_command(&session_id))
                .and_then(|command| {
                    terminal::validate_resume_command(&command)
                        .ok()
                        .map(|_| command)
                });
            SessionMeta {
                provider_id,
                session_id,
                title: entry.session.title.clone(),
                summary: entry.session.title,
                project_dir: entry
                    .session
                    .workspace
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                created_at: entry.session.started_at,
                last_active_at: entry.session.ended_at.or(entry.session.started_at),
                // Preserve CASR's provider-native source path. OpenCode uses
                // a virtual database/session-id path that supports arbitrary
                // IDs; converting it to the legacy sqlite spelling would
                // silently reject non-ses_ IDs.
                source_path: Some(entry.path.to_string_lossy().into_owned()),
                resume_command,
            }
        })
        .collect()
}

/// Merge the legacy fast-path sessions with CASR's canonical discovery.
///
/// A normalized path is the strongest identity. The provider/session key is
/// also indexed because a provider can expose the same session through a
/// database virtual path and a legacy transcript path. CASR entries are
/// inserted last and replace a legacy duplicate so the richer canonical
/// metadata and CASR path win without changing existing scanner behavior for
/// sessions CASR cannot parse.
fn merge_session_results(
    legacy: Vec<SessionMeta>,
    casr_sessions: Vec<SessionMeta>,
) -> Vec<SessionMeta> {
    let mut merged: Vec<(SessionMeta, bool)> = Vec::new();
    let mut indexes = HashMap::<String, usize>::new();

    for (session, from_casr) in legacy
        .into_iter()
        .map(|session| (session, false))
        .chain(casr_sessions.into_iter().map(|session| (session, true)))
    {
        let keys = session_identity_keys(&session);
        let existing = keys.iter().find_map(|key| indexes.get(key).copied());
        if let Some(index) = existing {
            if from_casr && !merged[index].1 {
                merged[index] = (session, true);
            }
            for key in keys {
                indexes.insert(key, index);
            }
        } else {
            let index = merged.len();
            for key in keys {
                indexes.insert(key, index);
            }
            merged.push((session, from_casr));
        }
    }

    let mut sessions: Vec<SessionMeta> = merged.into_iter().map(|(session, _)| session).collect();
    sessions.sort_by(|a, b| {
        let a_ts = a.last_active_at.or(a.created_at).unwrap_or(0);
        let b_ts = b.last_active_at.or(b.created_at).unwrap_or(0);
        b_ts.cmp(&a_ts)
            .then_with(|| a.provider_id.cmp(&b.provider_id))
            .then_with(|| a.session_id.cmp(&b.session_id))
            .then_with(|| a.source_path.cmp(&b.source_path))
    });
    sessions
}

fn session_identity_keys(session: &SessionMeta) -> Vec<String> {
    let provider_id = session.provider_id.trim().to_ascii_lowercase();
    if let Some(source_path) = session.source_path.as_deref() {
        let normalized_path =
            crate::session_transfer::normalize_source_path_for_provider(&provider_id, source_path)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|_| source_path.to_string());
        return vec![format!(
            "path:{}",
            crate::session_transfer::session_path_identity(&provider_id, &normalized_path)
        )];
    }

    // A path is the authoritative identity for file/database-backed sessions.
    // Fall back to provider + session ID only for records that genuinely have
    // no source path to avoid collapsing distinct files that reuse an ID.
    vec![format!(
        "provider-session:{provider_id}:{}",
        session.session_id
    )]
}

pub fn load_messages(provider_id: &str, source_path: &str) -> Result<Vec<SessionMessage>, String> {
    // SQLite sessions use a "sqlite:" prefixed source_path
    if provider_id == "opencode" && source_path.starts_with("sqlite:") {
        return opencode::load_messages_sqlite(source_path);
    }
    if provider_id == "opencode" && opencode::parse_virtual_path(source_path).is_some() {
        return load_messages_via_casr(provider_id, source_path);
    }
    if provider_id == "hermes" && source_path.starts_with("sqlite:") {
        return hermes::load_messages_sqlite(source_path);
    }

    let path = Path::new(source_path);
    match provider_id {
        "codex" => codex::load_messages(path),
        "claude" => claude::load_messages(path),
        "opencode" => opencode::load_messages(path),
        "openclaw" => openclaw::load_messages(path),
        "gemini" => gemini::load_messages(path),
        "grokbuild" => grokbuild::load_messages(path),
        "hermes" => hermes::load_messages(path),
        "pi" => pi::load_messages(path),
        _ => load_messages_via_casr(provider_id, source_path),
    }
}

fn load_messages_via_casr(
    provider_id: &str,
    source_path: &str,
) -> Result<Vec<SessionMessage>, String> {
    let alias = crate::session_transfer::casr_alias_for_provider(provider_id)
        .ok_or_else(|| format!("Unsupported provider: {provider_id}"))?;
    let path =
        crate::session_transfer::normalize_source_path_for_provider(provider_id, source_path)
            .map_err(|error| error.to_string())?;
    let registry = casr::ProviderRegistry::default_registry();
    let provider = registry
        .find_by_alias(&alias)
        .ok_or_else(|| format!("Unsupported provider: {provider_id}"))?;
    let session = provider
        .read_session(&path)
        .map_err(|_| "The selected session could not be read.".to_string())?;
    Ok(canonical_messages_to_session_messages(&session.messages))
}

fn canonical_messages_to_session_messages(
    messages: &[casr::model::CanonicalMessage],
) -> Vec<SessionMessage> {
    messages
        .iter()
        .map(|message| SessionMessage {
            role: match &message.role {
                casr::model::MessageRole::User => "user".to_string(),
                casr::model::MessageRole::Assistant => "assistant".to_string(),
                casr::model::MessageRole::Tool => "tool".to_string(),
                casr::model::MessageRole::System => "system".to_string(),
                casr::model::MessageRole::Other(role) => role.clone(),
            },
            content: message.content.clone(),
            ts: message.timestamp,
        })
        .collect()
}

pub fn delete_session(
    provider_id: &str,
    session_id: &str,
    source_path: &str,
) -> Result<bool, String> {
    // SQLite sessions bypass the file-based deletion path
    if provider_id == "opencode" && source_path.starts_with("sqlite:") {
        return opencode::delete_session_sqlite(session_id, source_path);
    }
    if provider_id == "opencode" && opencode::parse_virtual_path(source_path).is_some() {
        return opencode::delete_session_virtual_path(session_id, source_path);
    }
    if provider_id == "hermes" && source_path.starts_with("sqlite:") {
        return hermes::delete_session_sqlite(session_id, source_path);
    }

    let roots = provider_roots(provider_id)?;
    delete_session_with_roots(provider_id, session_id, Path::new(source_path), &roots)
}

pub fn delete_sessions(requests: &[DeleteSessionRequest]) -> Vec<DeleteSessionOutcome> {
    collect_delete_session_outcomes(requests, |request| {
        delete_session(
            &request.provider_id,
            &request.session_id,
            &request.source_path,
        )
    })
}

fn delete_session_with_roots(
    provider_id: &str,
    session_id: &str,
    source_path: &Path,
    roots: &[PathBuf],
) -> Result<bool, String> {
    let validated_source = canonicalize_existing_path(source_path, "session source")?;

    let mut saw_existing_root = false;
    for root in roots {
        if !root.exists() {
            continue;
        }

        saw_existing_root = true;
        let validated_root = canonicalize_existing_path(root, "session root")?;
        if validated_source.starts_with(&validated_root) {
            return match provider_id {
                "codex" => codex::delete_session(&validated_root, &validated_source, session_id),
                "claude" => claude::delete_session(&validated_root, &validated_source, session_id),
                "opencode" => {
                    opencode::delete_session(&validated_root, &validated_source, session_id)
                }
                "openclaw" => {
                    openclaw::delete_session(&validated_root, &validated_source, session_id)
                }
                "gemini" => gemini::delete_session(&validated_root, &validated_source, session_id),
                "grokbuild" => {
                    grokbuild::delete_session(&validated_root, &validated_source, session_id)
                }
                "hermes" => hermes::delete_session(&validated_root, &validated_source, session_id),
                "pi" => pi::delete_session(&validated_root, &validated_source, session_id),
                _ => Err(format!("Unsupported provider: {provider_id}")),
            };
        }
    }

    if !saw_existing_root {
        return Err(format!(
            "Session root not found for provider {provider_id}: {}",
            roots
                .first()
                .map(|root| root.display().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        ));
    }

    Err(format!(
        "Session source path is outside provider roots: {}",
        source_path.display()
    ))
}

fn provider_roots(provider_id: &str) -> Result<Vec<PathBuf>, String> {
    let roots = match provider_id {
        "codex" => codex::session_roots(),
        "claude" => vec![crate::config::get_claude_config_dir().join("projects")],
        "opencode" => vec![opencode::get_opencode_data_dir()],
        "openclaw" => vec![crate::openclaw_config::get_openclaw_dir().join("agents")],
        "gemini" => vec![crate::gemini_config::get_gemini_dir().join("tmp")],
        "grokbuild" => grokbuild::session_roots(),
        "hermes" => vec![crate::hermes_config::get_hermes_dir().join("sessions")],
        "pi" => pi::session_roots(),
        _ => return Err(format!("Unsupported provider: {provider_id}")),
    };

    Ok(roots)
}

fn canonicalize_existing_path(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.exists() {
        return Err(format!("{label} not found: {}", path.display()));
    }

    path.canonicalize()
        .map_err(|e| format!("Failed to resolve {label} {}: {e}", path.display()))
}

fn collect_delete_session_outcomes<F>(
    requests: &[DeleteSessionRequest],
    mut deleter: F,
) -> Vec<DeleteSessionOutcome>
where
    F: FnMut(&DeleteSessionRequest) -> Result<bool, String>,
{
    requests
        .iter()
        .map(|request| match deleter(request) {
            Ok(true) => DeleteSessionOutcome {
                provider_id: request.provider_id.clone(),
                session_id: request.session_id.clone(),
                source_path: request.source_path.clone(),
                success: true,
                error: None,
            },
            Ok(false) => DeleteSessionOutcome {
                provider_id: request.provider_id.clone(),
                session_id: request.session_id.clone(),
                source_path: request.source_path.clone(),
                success: false,
                error: Some("Session was not deleted".to_string()),
            },
            Err(error) => DeleteSessionOutcome {
                provider_id: request.provider_id.clone(),
                session_id: request.session_id.clone(),
                source_path: request.source_path.clone(),
                success: false,
                error: Some(error),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serial_test::serial;
    use tempfile::tempdir;

    fn write_codex_session(path: &Path, session_id: &str) {
        std::fs::write(
            path,
            format!(
                "{{\"timestamp\":\"2026-03-06T21:50:12Z\",\"type\":\"session_meta\",\"payload\":{{\"id\":\"{session_id}\",\"cwd\":\"/tmp/project\"}}}}\n\
                 {{\"timestamp\":\"2026-03-06T21:50:13Z\",\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":\"hello\"}}}}\n",
            ),
        )
        .expect("write source");
    }

    #[test]
    fn accepts_source_path_under_any_allowed_provider_root() {
        let active_root = tempdir().expect("active root");
        let archived_root = tempdir().expect("archived root");
        let source = archived_root.path().join("session.jsonl");
        write_codex_session(&source, "archived-session");

        let deleted = delete_session_with_roots(
            "codex",
            "archived-session",
            &source,
            &[
                active_root.path().to_path_buf(),
                archived_root.path().to_path_buf(),
            ],
        )
        .expect("delete archived session");

        assert!(deleted);
        assert!(!source.exists());
    }

    #[test]
    fn rejects_source_path_outside_provider_root() {
        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("tempdir");
        let source = outside.path().join("session.jsonl");
        std::fs::write(&source, "{}").expect("write source");

        let err =
            delete_session_with_roots("codex", "session-1", &source, &[root.path().to_path_buf()])
                .expect_err("expected outside-root path to be rejected");

        assert!(err.contains("outside provider roots"));
    }

    #[test]
    fn rejects_missing_source_path() {
        let root = tempdir().expect("tempdir");
        let missing = root.path().join("missing.jsonl");

        let err =
            delete_session_with_roots("codex", "session-1", &missing, &[root.path().to_path_buf()])
                .expect_err("expected missing source path to fail");

        assert!(err.contains("session source not found"));
    }

    #[test]
    fn batch_delete_collects_successes_and_failures_in_order() {
        let requests = vec![
            DeleteSessionRequest {
                provider_id: "codex".to_string(),
                session_id: "s1".to_string(),
                source_path: "/tmp/s1".to_string(),
            },
            DeleteSessionRequest {
                provider_id: "claude".to_string(),
                session_id: "s2".to_string(),
                source_path: "/tmp/s2".to_string(),
            },
            DeleteSessionRequest {
                provider_id: "gemini".to_string(),
                session_id: "s3".to_string(),
                source_path: "/tmp/s3".to_string(),
            },
        ];

        let outcomes = collect_delete_session_outcomes(&requests, |request| {
            match request.session_id.as_str() {
                "s1" => Ok(true),
                "s2" => Err("boom".to_string()),
                _ => Ok(false),
            }
        });

        assert_eq!(outcomes.len(), 3);
        assert!(outcomes[0].success);
        assert_eq!(outcomes[0].error, None);
        assert!(!outcomes[1].success);
        assert_eq!(outcomes[1].error.as_deref(), Some("boom"));
        assert!(!outcomes[2].success);
        assert_eq!(
            outcomes[2].error.as_deref(),
            Some("Session was not deleted")
        );
    }

    #[test]
    fn casr_discovery_wins_duplicate_legacy_session() {
        let temp = tempdir().expect("tempdir");
        let source = temp.path().join("session.jsonl");
        std::fs::write(&source, "fixture").expect("write source");

        let legacy = SessionMeta {
            provider_id: "codex".to_string(),
            session_id: "session-1".to_string(),
            title: Some("legacy".to_string()),
            summary: None,
            project_dir: None,
            created_at: Some(1),
            last_active_at: Some(2),
            source_path: Some(source.to_string_lossy().into_owned()),
            resume_command: Some("codex resume session-1".to_string()),
        };
        let casr = SessionMeta {
            title: Some("CASR title".to_string()),
            summary: Some("CASR summary".to_string()),
            last_active_at: Some(3),
            ..legacy.clone()
        };

        let merged = merge_session_results(vec![legacy], vec![casr]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].title.as_deref(), Some("CASR title"));
        assert_eq!(merged[0].summary.as_deref(), Some("CASR summary"));
    }

    #[test]
    fn casr_discovery_suppresses_unsafe_resume_command() {
        let discovered = vec![casr::DiscoveredSession {
            provider_slug: "codex".to_string(),
            provider_alias: "codex".to_string(),
            path: PathBuf::from("/tmp/session.jsonl"),
            session: casr::model::CanonicalSession {
                session_id: "bad;touch /tmp/pwned".to_string(),
                provider_slug: "codex".to_string(),
                workspace: None,
                title: Some("unsafe".to_string()),
                started_at: None,
                ended_at: None,
                messages: vec![casr::model::CanonicalMessage {
                    idx: 0,
                    role: casr::model::MessageRole::User,
                    content: "hello".to_string(),
                    timestamp: None,
                    author: None,
                    tool_calls: Vec::new(),
                    tool_results: Vec::new(),
                    extra: serde_json::Value::Null,
                }],
                metadata: serde_json::Value::Null,
                source_path: PathBuf::from("/tmp/session.jsonl"),
                model_name: None,
            },
        }];

        let sessions = casr_discovered_to_session_meta(discovered);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].resume_command, None);
    }

    #[test]
    fn merge_deduplicates_legacy_sqlite_and_casr_virtual_paths() {
        let temp = tempdir().expect("tempdir");
        let database = temp.path().join("opencode.db");
        std::fs::write(&database, "fixture").expect("write database");
        let legacy_path = format!("sqlite:{}:ses/with spaces", database.display());
        let virtual_path = database.join("ses%2Fwith%20spaces");

        let make_meta = |source_path: String, title: &str| SessionMeta {
            provider_id: "opencode".to_string(),
            session_id: "ses/with spaces".to_string(),
            title: Some(title.to_string()),
            summary: None,
            project_dir: None,
            created_at: Some(1),
            last_active_at: Some(1),
            source_path: Some(source_path),
            resume_command: None,
        };

        let merged = merge_session_results(
            vec![make_meta(legacy_path, "legacy")],
            vec![make_meta(
                virtual_path.to_string_lossy().into_owned(),
                "CASR",
            )],
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].title.as_deref(), Some("CASR"));
    }

    #[test]
    #[serial]
    #[allow(deprecated)]
    fn load_messages_reads_merged_casr_opencode_session() {
        let temp = tempdir().expect("tempdir");
        let database = temp.path().join("opencode.db");
        let connection = Connection::open(&database).expect("open database");
        connection
            .execute_batch(
                r#"
                CREATE TABLE session (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    directory TEXT NOT NULL,
                    time_created INTEGER NOT NULL,
                    time_updated INTEGER NOT NULL
                );
                CREATE TABLE message (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    time_created INTEGER NOT NULL,
                    data TEXT NOT NULL
                );
                CREATE TABLE part (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    message_id TEXT NOT NULL,
                    time_created INTEGER NOT NULL,
                    data TEXT NOT NULL
                );
                INSERT INTO session (id, title, directory, time_created, time_updated)
                    VALUES ('ses/with spaces', 'Session', '/tmp/project', 1, 2);
                INSERT INTO message (id, session_id, time_created, data)
                    VALUES ('msg_1', 'ses/with spaces', 1, '{"role":"user"}');
                INSERT INTO part (id, session_id, message_id, time_created, data)
                    VALUES ('part_1', 'ses/with spaces', 'msg_1', 1,
                            '{"type":"text","text":"hello"}');
                "#,
            )
            .expect("create OpenCode fixture");
        drop(connection);

        let original_db = std::env::var_os("OPENCODE_DB_PATH");
        std::env::set_var("OPENCODE_DB_PATH", &database);

        let discovered = casr::DiscoveredSession {
            provider_slug: "opencode".to_string(),
            provider_alias: "opc".to_string(),
            path: database.join("ses%2Fwith%20spaces"),
            session: casr::model::CanonicalSession {
                session_id: "ses/with spaces".to_string(),
                provider_slug: "opencode".to_string(),
                workspace: None,
                title: Some("Session".to_string()),
                started_at: Some(1),
                ended_at: Some(2),
                messages: vec![casr::model::CanonicalMessage {
                    idx: 0,
                    role: casr::model::MessageRole::User,
                    content: "hello".to_string(),
                    timestamp: Some(1),
                    author: None,
                    tool_calls: Vec::new(),
                    tool_results: Vec::new(),
                    extra: serde_json::Value::Null,
                }],
                metadata: serde_json::Value::Null,
                source_path: database.join("ses%2Fwith%20spaces"),
                model_name: None,
            },
        };
        let session = casr_discovered_to_session_meta(vec![discovered])
            .pop()
            .expect("CASR session metadata");
        let source_path = session.source_path.expect("CASR virtual source path");
        assert_eq!(
            source_path,
            database.join("ses%2Fwith%20spaces").to_string_lossy()
        );
        let messages = load_messages("opencode", &source_path)
            .expect("merged CASR session should load through virtual-path adapter");

        if let Some(value) = original_db {
            std::env::set_var("OPENCODE_DB_PATH", value);
        } else {
            std::env::remove_var("OPENCODE_DB_PATH");
        }

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
    }

    #[test]
    #[serial]
    #[allow(deprecated)]
    fn delete_session_removes_merged_casr_opencode_session() {
        let temp = tempdir().expect("tempdir");
        let original_xdg = std::env::var_os("XDG_DATA_HOME");
        std::env::set_var("XDG_DATA_HOME", temp.path());

        let database_dir = temp.path().join("opencode");
        std::fs::create_dir_all(&database_dir).expect("create database dir");
        let database = database_dir.join("opencode.db");
        let session_id = "ses_delete_1";
        let connection = Connection::open(&database).expect("open database");
        connection
            .execute_batch(
                r#"
                CREATE TABLE sessions (
                    id TEXT PRIMARY KEY,
                    parent_session_id TEXT,
                    title TEXT NOT NULL,
                    message_count INTEGER NOT NULL DEFAULT 0,
                    prompt_tokens INTEGER NOT NULL DEFAULT 0,
                    completion_tokens INTEGER NOT NULL DEFAULT 0,
                    cost REAL NOT NULL DEFAULT 0.0,
                    updated_at INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    summary_message_id TEXT
                );
                CREATE TABLE messages (
                    id TEXT PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    parts TEXT NOT NULL DEFAULT '[]',
                    model TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    finished_at INTEGER
                );
                "#,
            )
            .expect("create OpenCode fixture");
        connection
            .execute(
                "INSERT INTO sessions (id, title, updated_at, created_at) VALUES (?1, 'Session', 2, 1)",
                [session_id],
            )
            .expect("insert session");
        drop(connection);

        let discovered = casr::DiscoveredSession {
            provider_slug: "opencode".to_string(),
            provider_alias: "opc".to_string(),
            path: database.join(session_id),
            session: casr::model::CanonicalSession {
                session_id: session_id.to_string(),
                provider_slug: "opencode".to_string(),
                workspace: None,
                title: Some("Session".to_string()),
                started_at: Some(1),
                ended_at: Some(2),
                messages: vec![casr::model::CanonicalMessage {
                    idx: 0,
                    role: casr::model::MessageRole::User,
                    content: "hello".to_string(),
                    timestamp: Some(1),
                    author: None,
                    tool_calls: Vec::new(),
                    tool_results: Vec::new(),
                    extra: serde_json::Value::Null,
                }],
                metadata: serde_json::Value::Null,
                source_path: database.join(session_id),
                model_name: None,
            },
        };
        let session = casr_discovered_to_session_meta(vec![discovered])
            .pop()
            .expect("CASR session metadata");
        let source_path = session.source_path.expect("CASR virtual source path");

        let deleted = delete_session("opencode", session_id, &source_path)
            .expect("merged CASR session should delete through virtual-path adapter");

        if let Some(value) = original_xdg {
            std::env::set_var("XDG_DATA_HOME", value);
        } else {
            std::env::remove_var("XDG_DATA_HOME");
        }

        assert!(deleted);
        let connection = Connection::open(&database).expect("re-open database");
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .expect("count remaining sessions");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn merge_keeps_same_session_id_at_distinct_paths() {
        let first_dir = tempdir().expect("first tempdir");
        let second_dir = tempdir().expect("second tempdir");
        let first_path = first_dir.path().join("first.jsonl");
        let second_path = second_dir.path().join("second.jsonl");
        std::fs::write(&first_path, "first").expect("write first");
        std::fs::write(&second_path, "second").expect("write second");

        let make_meta = |path: &Path, title: &str| SessionMeta {
            provider_id: "codex".to_string(),
            session_id: "same-id".to_string(),
            title: Some(title.to_string()),
            summary: None,
            project_dir: None,
            created_at: Some(1),
            last_active_at: Some(1),
            source_path: Some(path.to_string_lossy().into_owned()),
            resume_command: None,
        };

        let merged = merge_session_results(
            vec![make_meta(&first_path, "first")],
            vec![make_meta(&second_path, "second")],
        );

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn canonical_messages_map_to_ui_messages_without_losing_role_or_timestamp() {
        let messages = vec![
            casr::model::CanonicalMessage {
                idx: 0,
                role: casr::model::MessageRole::User,
                content: "hello".to_string(),
                timestamp: Some(42),
                author: None,
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                extra: serde_json::Value::Null,
            },
            casr::model::CanonicalMessage {
                idx: 1,
                role: casr::model::MessageRole::Assistant,
                content: "world".to_string(),
                timestamp: None,
                author: None,
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
                extra: serde_json::Value::Null,
            },
        ];

        let mapped = canonical_messages_to_session_messages(&messages);

        assert_eq!(mapped.len(), 2);
        assert_eq!(mapped[0].role, "user");
        assert_eq!(mapped[0].content, "hello");
        assert_eq!(mapped[0].ts, Some(42));
        assert_eq!(mapped[1].role, "assistant");
        assert_eq!(mapped[1].content, "world");
    }
}
