use cc_launch_lib::session_transfer::{
    build_launch_spec, casr_alias_for_provider, launch_transferred_session_sync,
    normalize_source_path, normalize_source_path_for_provider, normalize_workspace_path,
    transfer_session_sync, LaunchTransferredSessionRequest, SessionTransferError,
    SessionTransferRequest,
};
use serde_json::Value;
use std::path::PathBuf;

#[test]
fn provider_id_mapping_covers_all_casr_aliases() {
    let expected = [
        ("claude", "cc"),
        ("codex", "cod"),
        ("gemini", "gmi"),
        ("antigravity", "agy"),
        ("cursor", "cur"),
        ("cline", "cln"),
        ("aider", "aid"),
        ("amp", "amp"),
        ("opencode", "opc"),
        ("chatgpt", "gpt"),
        ("clawdbot", "cwb"),
        ("vibe", "vib"),
        ("factory", "fac"),
        ("openclaw", "ocl"),
        ("pi", "pi"),
        ("kiro", "kr"),
        ("grokbuild", "grk"),
    ];

    for (provider_id, alias) in expected {
        assert_eq!(
            casr_alias_for_provider(provider_id).as_deref(),
            Some(alias),
            "provider mapping missing for {provider_id}"
        );
    }
}

#[test]
fn provider_id_mapping_accepts_casr_slugs_and_aliases() {
    assert_eq!(
        casr_alias_for_provider("claude-code").as_deref(),
        Some("cc")
    );
    assert_eq!(
        casr_alias_for_provider("claude-code").as_deref(),
        Some("cc")
    );
    assert_eq!(casr_alias_for_provider("pi-agent").as_deref(), Some("pi"));
    assert_eq!(casr_alias_for_provider("opc").as_deref(), Some("opc"));
}

#[test]
fn source_path_validation_rejects_empty_relative_and_missing_paths() {
    assert!(normalize_source_path("").is_err());
    assert!(normalize_source_path("relative/session.jsonl").is_err());

    let missing = if cfg!(windows) {
        r"C:\cc2cx\missing-session.jsonl"
    } else {
        "/cc2cx/missing-session.jsonl"
    };
    assert!(normalize_source_path(missing).is_err());
}

#[test]
fn source_path_validation_canonicalizes_real_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let nested = dir.path().join("nested");
    std::fs::create_dir(&nested).expect("nested directory");
    let source = nested.join("session.jsonl");
    std::fs::write(&source, "fixture").expect("session fixture");

    let normalized = normalize_source_path(&source.to_string_lossy()).expect("valid source");
    assert!(normalized.is_absolute());
    assert_eq!(normalized, source.canonicalize().expect("canonical source"));
}

#[test]
fn workspace_validation_requires_directory_and_returns_absolute_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path().join("project");
    std::fs::create_dir(&workspace).expect("workspace directory");

    let normalized = normalize_workspace_path(Some(&workspace.to_string_lossy()))
        .expect("valid workspace")
        .expect("workspace present");
    assert!(normalized.is_absolute());
    assert_eq!(
        normalized,
        casr::model::normalize_workspace_for_cli(
            &workspace.canonicalize().expect("canonical workspace")
        )
    );
    assert_eq!(
        normalize_workspace_path(None).expect("optional workspace"),
        None
    );

    let file = dir.path().join("not-a-directory");
    std::fs::write(&file, "fixture").expect("file fixture");
    assert!(normalize_workspace_path(Some(&file.to_string_lossy())).is_err());
}

#[cfg(windows)]
#[test]
fn workspace_validation_returns_cli_compatible_windows_path() {
    let dir = tempfile::tempdir().expect("tempdir");

    let normalized = normalize_workspace_path(Some(&dir.path().to_string_lossy()))
        .expect("workspace should be valid")
        .expect("workspace should be present");

    assert!(!normalized.to_string_lossy().starts_with(r"\\?\"));
}

#[test]
fn serialized_errors_have_stable_envelope_without_session_content() {
    let error = SessionTransferError::source_read_failed(
        "codex",
        "C:\\Users\\fixture\\session.jsonl",
        "secret prompt and api-key=super-secret",
    );
    let value: Value = serde_json::to_value(&error).expect("serialize error envelope");

    assert_eq!(
        value.get("code").and_then(Value::as_str),
        Some("source_read_failed")
    );
    assert!(value.get("message").and_then(Value::as_str).is_some());
    let serialized = serde_json::to_string(&value).expect("serialize value");
    assert!(!serialized.contains("secret prompt"));
    assert!(!serialized.contains("super-secret"));
}

#[test]
fn transfer_error_does_not_echo_untrusted_renderer_command() {
    let command = "powershell -enc secret-payload";
    let error = SessionTransferError::invalid_request(command);
    let serialized = serde_json::to_string(&error).expect("serialize error envelope");
    assert!(!serialized.contains(command));
    assert!(!serialized.contains("secret-payload"));
}

#[test]
fn virtual_database_source_path_is_accepted_when_database_exists() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("opencode.db");
    std::fs::write(&db, "fixture").expect("database fixture");
    let virtual_path = db.join("ses_fixture");

    let normalized = normalize_source_path(&virtual_path.to_string_lossy())
        .expect("virtual database path should be accepted");
    assert_eq!(
        normalized.parent(),
        Some(db.canonicalize().expect("canonical db").as_path())
    );
    assert_eq!(normalized.file_name(), Some("ses_fixture".as_ref()));
}

#[test]
fn legacy_opencode_sqlite_path_is_normalized_to_casr_virtual_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("opencode.db");
    std::fs::write(&db, "fixture").expect("database fixture");
    let legacy = format!("sqlite:{}:ses/with spaces", db.display());

    let normalized = normalize_source_path_for_provider("opencode", &legacy)
        .expect("legacy OpenCode source should be accepted");
    assert_eq!(
        normalized.parent(),
        Some(db.canonicalize().expect("canonical db").as_path())
    );
    assert_eq!(
        normalized.file_name().and_then(|name| name.to_str()),
        Some("ses%2Fwith%20spaces")
    );
}

#[test]
fn legacy_opencode_sqlite_path_handles_windows_drive_and_colon_session_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db = dir.path().join("opencode.db");
    std::fs::write(&db, "fixture").expect("database fixture");
    let legacy = format!("sqlite:{}:session:part", db.display());

    let normalized = normalize_source_path_for_provider("opc", &legacy)
        .expect("legacy OpenCode source should preserve colon-bearing IDs");
    assert_eq!(
        normalized.file_name().and_then(|name| name.to_str()),
        Some("session%3Apart")
    );
}

#[test]
fn launch_specs_use_casr_slugs_and_structured_unsupported_cases() {
    let grok = build_launch_spec("grokbuild", "session-1", None).expect("Grok launch spec");
    assert_eq!(grok.program, "grok");
    assert_eq!(grok.args, vec!["--resume", "session-1"]);

    let kiro = build_launch_spec("kiro", "session-1", None).expect("Kiro launch spec");
    assert_eq!(kiro.program, "kiro-cli");
    assert_eq!(kiro.args, vec!["--resume-id", "session-1"]);

    let cursor = build_launch_spec("cursor", "composer-1", None)
        .expect_err("Cursor has no session-id-specific launcher");
    assert_eq!(cursor.code, "launch_unsupported");

    let opencode =
        build_launch_spec("opencode", "session-1", None).expect("OpenCode supports session resume");
    assert_eq!(opencode.program, "opencode");
    assert_eq!(opencode.args, vec!["--session", "session-1"]);
}

#[cfg(windows)]
#[test]
fn launch_program_resolution_prefers_native_exe_over_cmd_shim() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cmd = dir.path().join("codex.cmd");
    let exe = dir.path().join("codex.exe");
    std::fs::write(&cmd, "@echo off").expect("cmd fixture");
    std::fs::write(&exe, "fixture").expect("exe fixture");

    let resolved = cc_launch_lib::session_transfer::find_program_in_paths(
        "codex",
        &[dir.path().to_path_buf()],
    )
    .expect("program should resolve");
    assert_eq!(resolved, exe);
}

#[test]
fn launch_program_resolution_returns_none_for_missing_program() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        cc_launch_lib::session_transfer::find_program_in_paths(
            "missing-agent",
            &[dir.path().to_path_buf()],
        ),
        None
    );
}

#[test]
fn pi_launch_rejects_path_traversal_session_ids() {
    for hostile_id in ["../escape", ".", "..", "nested/escape", "C:escape"] {
        let error = build_launch_spec("pi", hostile_id, None)
            .expect_err("Pi session IDs must stay within the sessions directory");
        assert_eq!(error.code, "invalid_request", "hostile ID: {hostile_id}");
    }
}

#[test]
fn unknown_provider_mapping_returns_none() {
    assert_eq!(casr_alias_for_provider("not-a-provider"), None);
}

#[test]
fn unsupported_launcher_keeps_transfer_result_non_fatal() {
    let result = launch_transferred_session_sync(LaunchTransferredSessionRequest {
        target_provider: "cursor".to_string(),
        session_id: "session-1".to_string(),
        workspace: None,
    })
    .expect("unsupported launch should not fail the written transfer");

    assert!(!result.launched);
    assert!(result.warning.is_some());
}

#[test]
fn same_provider_transfer_is_rejected_before_source_access() {
    let error = transfer_session_sync(SessionTransferRequest {
        source_provider_id: "codex".to_string(),
        source_session_id: "session-1".to_string(),
        source_path: "C:\\missing\\session.jsonl".to_string(),
        target_provider: "cod".to_string(),
        workspace: None,
        force: false,
        enrich: false,
        max_context_tokens: 0,
        max_tool_output: 0,
        keep_reasoning: true,
    })
    .expect_err("same-provider transfer must not create a duplicate session");

    assert_eq!(error.code, "same_provider");
}

#[allow(dead_code)]
fn _path_type_is_available_for_windows_compile() -> PathBuf {
    PathBuf::new()
}
