use std::collections::HashMap;

use cc_launch_lib::session_transfer::{
    build_launch_spec, casr_alias_for_provider, list_session_transfer_targets_sync,
};

#[test]
fn capability_matrix_exposes_all_registered_providers_and_static_boundaries() {
    let expected = [
        ("claude", "cc", "Claude Code", "supported"),
        ("codex", "cod", "Codex", "supported"),
        ("gemini", "gmi", "Gemini CLI", "supported"),
        ("antigravity", "agy", "Antigravity CLI", "readOnly"),
        ("cursor", "cur", "Cursor", "supported"),
        ("cline", "cln", "Cline", "supported"),
        ("aider", "aid", "Aider", "supported"),
        ("amp", "amp", "Amp", "supported"),
        ("opencode", "opc", "OpenCode", "conditional"),
        ("chatgpt", "gpt", "ChatGPT", "supported"),
        ("clawdbot", "cwb", "ClawdBot", "supported"),
        ("vibe", "vib", "Vibe", "supported"),
        ("factory", "fac", "Factory", "supported"),
        ("openclaw", "ocl", "OpenClaw", "supported"),
        ("pi", "pi", "Pi-Agent", "supported"),
        ("kiro", "kr", "Kiro CLI", "supported"),
        ("grokbuild", "grk", "Grok Build", "supported"),
    ];

    let targets = list_session_transfer_targets_sync();
    assert_eq!(targets.len(), expected.len());

    let by_id: HashMap<_, _> = targets
        .iter()
        .map(|target| (target.provider_id.as_str(), target))
        .collect();
    assert_eq!(by_id.len(), expected.len(), "provider IDs must be unique");

    for (provider_id, alias, name, write_kind) in expected {
        let target = by_id
            .get(provider_id)
            .unwrap_or_else(|| panic!("missing capability for {provider_id}"));
        assert_eq!(target.alias, alias, "alias mismatch for {provider_id}");
        assert_eq!(target.name, name, "name mismatch for {provider_id}");
        assert_eq!(
            target.write_support.kind, write_kind,
            "write capability mismatch for {provider_id}"
        );
        assert_eq!(casr_alias_for_provider(provider_id).as_deref(), Some(alias));
    }

    let antigravity = by_id.get("antigravity").expect("Antigravity capability");
    assert_eq!(antigravity.write_support.kind, "readOnly");
    assert!(antigravity
        .write_support
        .reason
        .as_deref()
        .is_some_and(|reason| !reason.is_empty()));

    for provider_id in ["cursor", "cline", "aider", "chatgpt", "antigravity"] {
        let target = by_id
            .get(provider_id)
            .unwrap_or_else(|| panic!("missing launch capability for {provider_id}"));
        assert_eq!(target.launch_support.kind, "unsupported");
        assert!(target
            .launch_support
            .reason
            .as_deref()
            .is_some_and(|reason| !reason.is_empty()));
    }

    for provider_id in [
        "claude",
        "codex",
        "gemini",
        "amp",
        "opencode",
        "clawdbot",
        "vibe",
        "factory",
        "openclaw",
        "pi",
        "kiro",
        "grokbuild",
    ] {
        let target = by_id
            .get(provider_id)
            .unwrap_or_else(|| panic!("missing launch capability for {provider_id}"));
        if cfg!(windows) || cfg!(target_os = "macos") {
            assert!(
                matches!(
                    target.launch_support.kind.as_str(),
                    "supported" | "executableMissing"
                ),
                "unexpected launch capability for {provider_id}: {}",
                target.launch_support.kind
            );
        } else {
            assert_eq!(target.launch_support.kind, "unsupported");
            assert!(target
                .launch_support
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("visible terminal")));
        }
    }

    let opencode = by_id.get("opencode").expect("OpenCode capability");
    assert_eq!(opencode.write_support.kind, "conditional");
    assert!(opencode
        .write_support
        .reason
        .as_deref()
        .is_some_and(|reason| !reason.is_empty()));
}

#[test]
fn launch_matrix_is_structured_and_marks_missing_resume_contracts() {
    let unsupported = ["cursor", "cline", "aider", "chatgpt", "antigravity"];
    for provider_id in unsupported {
        let error = build_launch_spec(provider_id, "session-matrix", None)
            .expect_err("provider without a session-specific launcher must be explicit");
        assert_eq!(error.code, "launch_unsupported");
    }

    let supported = [
        ("claude", "claude"),
        ("codex", "codex"),
        ("gemini", "gemini"),
        ("amp", "amp"),
        ("opencode", "opencode"),
        ("clawdbot", "clawdbot"),
        ("vibe", "vibe"),
        ("factory", "factory"),
        ("openclaw", "openclaw"),
        ("pi", "pi"),
        ("kiro", "kiro-cli"),
        ("grokbuild", "grok"),
    ];
    for (provider_id, program) in supported {
        let spec = build_launch_spec(provider_id, "session-matrix", None)
            .unwrap_or_else(|error| panic!("launcher missing for {provider_id}: {error}"));
        assert_eq!(spec.program, program);
        assert!(!spec.args.is_empty());
        assert!(spec.args.iter().all(|argument| !argument.contains('\0')));

        let hostile_id = "session;echo PWNED";
        let hostile_spec =
            build_launch_spec(provider_id, hostile_id, None).unwrap_or_else(|error| {
                panic!("launcher rejected a valid argv value for {provider_id}: {error}")
            });
        assert_eq!(
            hostile_spec
                .args
                .iter()
                .filter(|argument| argument.contains(hostile_id))
                .count(),
            1,
            "session ID must stay in one structured argv argument for {provider_id}"
        );
    }

    let workspace = tempfile::tempdir().expect("workspace fixture");
    let workspace_text = workspace.path().to_string_lossy().into_owned();
    let spec = build_launch_spec("codex", "session-matrix", Some(&workspace_text))
        .expect("workspace should be propagated to launch spec");
    assert_eq!(
        spec.workspace,
        Some(casr::model::normalize_workspace_for_cli(
            &workspace
                .path()
                .canonicalize()
                .expect("canonical workspace"),
        ))
    );
}
