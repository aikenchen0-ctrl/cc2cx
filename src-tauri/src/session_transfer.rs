//! CASR-backed cross-provider session transfer.
//!
//! This module is the product boundary around the vendored CASR library.  It
//! owns renderer-facing validation, cc2cx provider IDs, stable IPC errors and
//! structured process launching.  Session contents stay inside CASR and are
//! never included in DTOs or error messages.

use std::path::{Component, Path, PathBuf};
#[cfg(any(windows, target_os = "macos"))]
use std::process::Command;

use casr::providers::{ProviderCapability, WriteSupport};
use casr::{ConversionPipeline, ConvertOptions, ProviderRegistry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const MAX_SESSION_ID_CHARS: usize = 512;

/// Request submitted by the session-management UI.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransferRequest {
    pub source_provider_id: String,
    pub source_session_id: String,
    pub source_path: String,
    pub target_provider: String,
    pub workspace: Option<String>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub enrich: bool,
    #[serde(default)]
    pub max_context_tokens: usize,
    #[serde(default)]
    pub max_tool_output: usize,
    #[serde(default = "default_keep_reasoning")]
    pub keep_reasoning: bool,
}

fn default_keep_reasoning() -> bool {
    true
}

/// Structured request for opening a transferred session.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTransferredSessionRequest {
    pub target_provider: String,
    pub session_id: String,
    pub workspace: Option<String>,
}

/// Write capability exposed to the UI.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransferWriteSupport {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Describes whether cc2cx can open a written session automatically.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransferLaunchSupport {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Target-provider capability shown before a transfer starts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransferTarget {
    pub provider_id: String,
    pub alias: String,
    pub name: String,
    pub installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub write_support: SessionTransferWriteSupport,
    pub launch_support: SessionTransferLaunchSupport,
}

/// Result returned after CASR has written and read back the target session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransferResult {
    pub source_provider_id: String,
    pub source_session_id: String,
    pub source_path: String,
    pub target_provider_id: String,
    pub target_session_id: String,
    pub workspace: Option<String>,
    pub written_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
    pub resume_command: String,
    pub warnings: Vec<String>,
    pub lossy: bool,
}

/// Result returned after a structured target launch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTransferredSessionResult {
    pub launched: bool,
    pub provider_id: String,
    pub session_id: String,
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Stable, renderer-safe error envelope.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionTransferError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl std::fmt::Display for SessionTransferError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SessionTransferError {}

impl SessionTransferError {
    fn new(code: impl Into<String>, message: impl Into<String>, details: Option<Value>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
        }
    }

    pub fn invalid_request(_untrusted_input: &str) -> Self {
        Self::new(
            "invalid_request",
            "The session transfer request is invalid.",
            None,
        )
    }

    fn invalid_source_path() -> Self {
        Self::new(
            "invalid_source_path",
            "Select an absolute session file or a supported virtual database session.",
            None,
        )
    }

    fn source_not_found() -> Self {
        Self::new(
            "source_not_found",
            "The selected source session no longer exists or cannot be read.",
            None,
        )
    }

    fn invalid_workspace() -> Self {
        Self::new(
            "invalid_workspace",
            "The workspace must be an existing directory.",
            None,
        )
    }

    fn unknown_provider() -> Self {
        Self::new(
            "unknown_provider",
            "The selected provider is not supported by this build.",
            None,
        )
    }

    fn same_provider() -> Self {
        Self::new(
            "same_provider",
            "The source and target tools must be different.",
            None,
        )
    }

    fn source_session_mismatch() -> Self {
        Self::new(
            "source_session_mismatch",
            "The selected session ID does not match the source session.",
            None,
        )
    }

    fn target_read_only() -> Self {
        Self::new(
            "target_read_only",
            "The selected provider is available for reading but cannot accept converted sessions.",
            None,
        )
    }

    fn target_schema_mismatch() -> Self {
        Self::new(
            "target_schema_mismatch",
            "The target provider storage schema is not writable by this build.",
            None,
        )
    }

    pub fn source_read_failed(_provider: &str, _path: &str, _detail: &str) -> Self {
        Self::new(
            "source_read_failed",
            "The source session could not be parsed.",
            None,
        )
    }

    fn conversion_failed(phase: &str) -> Self {
        Self::new(
            "conversion_failed",
            "The session conversion could not be completed.",
            Some(json!({ "phase": phase })),
        )
    }

    fn target_conflict() -> Self {
        Self::new(
            "target_conflict",
            "A target session already exists. Enable overwrite and retry if appropriate.",
            None,
        )
    }

    fn verify_failed() -> Self {
        Self::new(
            "verify_failed",
            "The target session failed read-back verification and was rolled back when possible.",
            None,
        )
    }

    fn rollback_failed() -> Self {
        Self::new(
            "rollback_failed",
            "The target session failed verification and cleanup also failed; manual cleanup may be required.",
            None,
        )
    }

    fn launch_unsupported() -> Self {
        Self::new(
            "launch_unsupported",
            "This provider has no structured local resume launcher. The session was still written.",
            None,
        )
    }

    fn target_executable_unavailable() -> Self {
        Self::new(
            "target_executable_unavailable",
            "The target provider executable was not found. Install it or launch it manually.",
            None,
        )
    }

    fn visible_terminal_unavailable() -> Self {
        Self::new(
            "visible_terminal_unavailable",
            "Automatic resume is unavailable because cc2cx cannot open a supported visible terminal on this platform. The session was still written.",
            None,
        )
    }

    fn launch_failed() -> Self {
        Self::new(
            "launch_failed",
            "The target provider process could not be started.",
            None,
        )
    }
}

/// Map a cc2cx provider ID, CASR slug or CASR alias to the canonical CASR alias.
pub fn casr_alias_for_provider(provider_id: &str) -> Option<String> {
    let registry = ProviderRegistry::default_registry();
    let token = normalize_provider_token(provider_id);
    registry.capabilities().into_iter().find_map(|cap| {
        let ui_id = ui_provider_id_for_slug(&cap.slug);
        let slug = normalize_provider_token(&cap.slug);
        let alias = normalize_provider_token(&cap.alias);
        (token == normalize_provider_token(&ui_id) || token == slug || token == alias)
            .then_some(cap.alias)
    })
}

/// Map a CASR slug to the stable provider ID used by cc2cx session metadata.
pub fn ui_provider_id_for_slug(slug: &str) -> String {
    match slug {
        "claude-code" => "claude".to_string(),
        "grok" => "grokbuild".to_string(),
        "pi-agent" => "pi".to_string(),
        other => other.to_string(),
    }
}

/// Validate and canonicalize a source path selected by the UI.
///
/// CASR uses virtual paths for database-backed sessions (`db-file/session-id`).
/// Such a path has a real database file as its parent and is accepted without
/// requiring the virtual leaf to exist on disk.
pub fn normalize_source_path(value: &str) -> Result<PathBuf, SessionTransferError> {
    let raw = value.trim();
    if raw.is_empty() || raw.contains('\0') {
        return Err(SessionTransferError::invalid_source_path());
    }

    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(SessionTransferError::invalid_source_path());
    }

    if path.is_file() {
        return path
            .canonicalize()
            .map_err(|_| SessionTransferError::source_not_found());
    }

    let Some(parent) = path.parent().filter(|parent| parent.is_file()) else {
        return Err(SessionTransferError::source_not_found());
    };
    let parent = parent
        .canonicalize()
        .map_err(|_| SessionTransferError::source_not_found())?;
    let Some(leaf) = path.file_name() else {
        return Err(SessionTransferError::invalid_source_path());
    };
    if matches!(leaf.to_str(), Some(".") | Some("..")) {
        return Err(SessionTransferError::invalid_source_path());
    }
    Ok(parent.join(leaf))
}

/// Normalize a source path from a particular legacy provider.
///
/// The legacy cc2cx scanner represented database-backed sessions as
/// `sqlite:<database>:<session-id>` (OpenCode) or
/// `sqlite:<database>#<session-id>` (Hermes). CASR uses a virtual path whose
/// parent is the real database and whose leaf is the URL-encoded session ID.
/// Splitting by the last colon is incorrect for both Windows drive letters and
/// session IDs that themselves contain colons, so the database boundary is
/// located by checking which prefix is an existing file.
pub fn normalize_source_path_for_provider(
    provider_id: &str,
    value: &str,
) -> Result<PathBuf, SessionTransferError> {
    let raw = value.trim();
    if raw.is_empty() || raw.contains('\0') {
        return Err(SessionTransferError::invalid_source_path());
    }
    if !raw.starts_with("sqlite:") {
        return normalize_source_path(raw);
    }

    let provider_token = normalize_provider_token(provider_id);
    let provider_alias = casr_alias_for_provider(provider_id);
    let rest = raw.strip_prefix("sqlite:").unwrap_or_default();
    let (db_path, session_id) = if provider_alias.as_deref() == Some("opc") {
        split_existing_database_reference(rest, ':')
            .ok_or_else(SessionTransferError::source_not_found)?
    } else if provider_token == "hermes" {
        split_existing_database_reference(rest, '#')
            .ok_or_else(SessionTransferError::source_not_found)?
    } else if provider_alias.is_none() {
        return Err(SessionTransferError::unknown_provider());
    } else {
        return Err(SessionTransferError::invalid_source_path());
    };

    if session_id.is_empty() || session_id.contains('\0') || matches!(session_id, "." | "..") {
        return Err(SessionTransferError::invalid_source_path());
    }

    let db = Path::new(db_path);
    if !db.is_absolute() {
        return Err(SessionTransferError::invalid_source_path());
    }
    if !db.is_file() {
        return Err(SessionTransferError::source_not_found());
    }
    let db = db
        .canonicalize()
        .map_err(|_| SessionTransferError::source_not_found())?;
    Ok(db.join(percent_encode_path_component(session_id)))
}

fn split_existing_database_reference(value: &str, separator: char) -> Option<(&str, &str)> {
    for (index, character) in value.char_indices() {
        if character != separator {
            continue;
        }
        let database = &value[..index];
        if Path::new(database).is_file() {
            let session_id = &value[index + separator.len_utf8()..];
            if !session_id.is_empty() {
                return Some((database, session_id));
            }
        }
    }
    None
}

fn percent_encode_path_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

/// Validate and canonicalize an optional workspace directory.
pub fn normalize_workspace_path(
    value: Option<&str>,
) -> Result<Option<PathBuf>, SessionTransferError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let raw = value.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    if raw.contains('\0') {
        return Err(SessionTransferError::invalid_workspace());
    }

    let path = PathBuf::from(raw);
    if !path.is_dir() {
        return Err(SessionTransferError::invalid_workspace());
    }
    path.canonicalize()
        .map(|canonical| Some(casr::model::normalize_workspace_for_cli(&canonical)))
        .map_err(|_| SessionTransferError::invalid_workspace())
}

/// Discover target capabilities for all providers in CASR's default registry.
pub fn list_session_transfer_targets_sync() -> Vec<SessionTransferTarget> {
    let launch_paths = launch_search_paths();
    ProviderRegistry::default_registry()
        .capabilities()
        .into_iter()
        .map(|capability| target_from_capability(capability, &launch_paths))
        .collect()
}

fn target_from_capability(
    capability: ProviderCapability,
    launch_paths: &[PathBuf],
) -> SessionTransferTarget {
    let write_support = match capability.write_support {
        WriteSupport::Supported => SessionTransferWriteSupport {
            kind: "supported".to_string(),
            reason: None,
        },
        WriteSupport::ReadOnly { reason } => SessionTransferWriteSupport {
            kind: "readOnly".to_string(),
            reason: Some(reason),
        },
        WriteSupport::Conditional { reason } => SessionTransferWriteSupport {
            kind: "conditional".to_string(),
            reason: Some(reason),
        },
    };

    let provider_id = ui_provider_id_for_slug(&capability.slug);
    let launch_support = launch_support_for_provider(&capability.slug, launch_paths);

    SessionTransferTarget {
        provider_id,
        alias: capability.alias,
        name: capability.name,
        installed: capability.detection.installed,
        version: capability.detection.version,
        write_support,
        launch_support,
    }
}

fn launch_support_for_provider(
    provider_slug: &str,
    launch_paths: &[PathBuf],
) -> SessionTransferLaunchSupport {
    let spec = match build_launch_spec_for_slug(provider_slug, "capability-probe", None) {
        Ok(spec) => spec,
        Err(error) if error.code == "launch_unsupported" => {
            return SessionTransferLaunchSupport {
                kind: "unsupported".to_string(),
                reason: Some(error.message),
            };
        }
        Err(error) => {
            return SessionTransferLaunchSupport {
                kind: "unsupported".to_string(),
                reason: Some(error.message),
            };
        }
    };

    if !platform_supports_visible_launch() {
        return SessionTransferLaunchSupport {
            kind: "unsupported".to_string(),
            reason: Some(SessionTransferError::visible_terminal_unavailable().message),
        };
    }

    let executable_found = launch_program_candidates(&spec.program)
        .into_iter()
        .any(|program| find_program_in_paths(program, launch_paths).is_some());
    if executable_found {
        SessionTransferLaunchSupport {
            kind: "supported".to_string(),
            reason: None,
        }
    } else {
        SessionTransferLaunchSupport {
            kind: "executableMissing".to_string(),
            reason: Some(SessionTransferError::target_executable_unavailable().message),
        }
    }
}

/// Perform a validated CASR conversion for a renderer request.
pub fn transfer_session_sync(
    request: SessionTransferRequest,
) -> Result<SessionTransferResult, SessionTransferError> {
    if request.source_session_id.trim().is_empty()
        || request.source_session_id.chars().count() > MAX_SESSION_ID_CHARS
        || request.source_session_id.contains('\0')
        || request.source_provider_id.trim().is_empty()
        || request.target_provider.trim().is_empty()
    {
        return Err(SessionTransferError::invalid_request("session transfer"));
    }

    let source_alias = casr_alias_for_provider(&request.source_provider_id)
        .ok_or_else(SessionTransferError::unknown_provider)?;
    let target_alias = casr_alias_for_provider(&request.target_provider)
        .ok_or_else(SessionTransferError::unknown_provider)?;
    if source_alias == target_alias {
        return Err(SessionTransferError::same_provider());
    }

    let source_path =
        normalize_source_path_for_provider(&request.source_provider_id, &request.source_path)?;
    let workspace = normalize_workspace_path(request.workspace.as_deref())?;
    let registry = ProviderRegistry::default_registry();

    let source_session = {
        let resolved = registry
            .resolve_source_path(&source_alias, &source_path)
            .map_err(|_| SessionTransferError::source_not_found())?;
        let session = resolved
            .provider
            .read_session(&source_path)
            .map_err(|_error| {
                log::warn!(
                    "CASR source read failed provider={} phase=source_read error_type={}",
                    source_alias,
                    "parse_error"
                );
                SessionTransferError::source_read_failed(
                    &source_alias,
                    &source_path.to_string_lossy(),
                    "parse_error",
                )
            })?;
        session
    };
    if source_session.session_id != request.source_session_id {
        return Err(SessionTransferError::source_session_mismatch());
    }
    let source_session_id = source_session.session_id.clone();

    if let Some(capability) = registry
        .capabilities()
        .into_iter()
        .find(|cap| cap.alias == target_alias)
    {
        if matches!(capability.write_support, WriteSupport::ReadOnly { .. }) {
            return Err(SessionTransferError::target_read_only());
        }
    }

    let pipeline = ConversionPipeline { registry };
    let options = ConvertOptions {
        force: request.force,
        enrich: request.enrich,
        max_context_tokens: request.max_context_tokens,
        max_tool_output: request.max_tool_output,
        keep_reasoning: request.keep_reasoning,
        workspace_override: workspace.clone(),
        ..ConvertOptions::default()
    };
    let converted = pipeline
        .convert_canonical_source_path(
            &source_alias,
            &source_path,
            source_session,
            &target_alias,
            options,
        )
        .map_err(|error| {
            log::warn!(
                "CASR session conversion failed source={} target={} phase=convert error_type={}",
                source_alias,
                target_alias,
                casr_error_type(error.as_ref())
            );
            classify_conversion_error(error.as_ref())
        })?;

    let written = converted
        .written
        .ok_or_else(|| SessionTransferError::conversion_failed("write"))?;
    let target_provider_id = ui_provider_id_for_slug(&converted.target_provider);
    let target_workspace = converted
        .canonical_session
        .workspace
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    let lossy = source_alias != target_alias || !converted.warnings.is_empty();

    Ok(SessionTransferResult {
        source_provider_id: ui_provider_id_for_slug(&converted.source_provider),
        source_session_id,
        source_path: source_path.to_string_lossy().into_owned(),
        target_provider_id,
        target_session_id: written.session_id,
        workspace: target_workspace,
        written_paths: written
            .paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        backup_path: written
            .backup_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned()),
        resume_command: written.resume_command,
        warnings: converted.warnings,
        lossy,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
    pub workspace: Option<PathBuf>,
}

/// Build an argv-only launch specification.  No shell syntax is produced.
pub fn build_launch_spec(
    target_provider: &str,
    session_id: &str,
    workspace: Option<&str>,
) -> Result<LaunchSpec, SessionTransferError> {
    let session_id = validate_launch_session_id(session_id)?;
    let workspace = normalize_workspace_path(workspace)?;
    let alias = casr_alias_for_provider(target_provider)
        .ok_or_else(SessionTransferError::unknown_provider)?;
    let registry = ProviderRegistry::default_registry();
    let provider = registry
        .find_by_alias(&alias)
        .ok_or_else(SessionTransferError::unknown_provider)?;
    build_launch_spec_for_slug(provider.slug(), session_id, workspace)
}

fn build_launch_spec_for_slug(
    slug: &str,
    session_id: &str,
    workspace: Option<PathBuf>,
) -> Result<LaunchSpec, SessionTransferError> {
    let (program, args) = match slug {
        "claude-code" => (
            "claude",
            vec!["--resume".to_string(), session_id.to_string()],
        ),
        "codex" => ("codex", vec!["resume".to_string(), session_id.to_string()]),
        "gemini" => (
            "gemini",
            vec!["--resume".to_string(), session_id.to_string()],
        ),
        "cursor" | "cline" | "aider" => return Err(SessionTransferError::launch_unsupported()),
        "amp" => (
            "amp",
            vec![
                "threads".to_string(),
                "continue".to_string(),
                "--execute".to_string(),
                format!("Continue from @{session_id}"),
            ],
        ),
        "opencode" => (
            "opencode",
            vec!["--session".to_string(), session_id.to_string()],
        ),
        "clawdbot" => (
            "clawdbot",
            vec!["--resume".to_string(), session_id.to_string()],
        ),
        "vibe" => ("vibe", vec!["--resume".to_string(), session_id.to_string()]),
        "factory" => (
            "factory",
            vec!["--resume".to_string(), session_id.to_string()],
        ),
        "openclaw" => (
            "openclaw",
            vec!["--resume".to_string(), session_id.to_string()],
        ),
        "pi-agent" => (
            "pi",
            vec![
                "--session".to_string(),
                pi_agent_session_path(session_id)
                    .to_string_lossy()
                    .into_owned(),
            ],
        ),
        "kiro" => (
            "kiro-cli",
            vec!["--resume-id".to_string(), session_id.to_string()],
        ),
        "grok" => ("grok", vec!["--resume".to_string(), session_id.to_string()]),
        "chatgpt" | "antigravity" => return Err(SessionTransferError::launch_unsupported()),
        _ => return Err(SessionTransferError::launch_unsupported()),
    };

    Ok(LaunchSpec {
        program: program.to_string(),
        args,
        workspace,
    })
}

fn validate_launch_session_id(session_id: &str) -> Result<&str, SessionTransferError> {
    let session_id = session_id.trim();
    if session_id.is_empty()
        || session_id.chars().count() > MAX_SESSION_ID_CHARS
        || session_id.contains('\0')
        || session_id
            .chars()
            .any(|character| matches!(character, '/' | '\\'))
        || session_id.contains(':')
        || Path::new(session_id).is_absolute()
        || Path::new(session_id).components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::CurDir
                    | Component::ParentDir
            )
        })
        || launch_session_id_contains_shell_metacharacters(session_id)
    {
        return Err(SessionTransferError::invalid_request("session id"));
    }
    Ok(session_id)
}

#[cfg(windows)]
fn launch_session_id_contains_shell_metacharacters(session_id: &str) -> bool {
    session_id
        .chars()
        .any(|character| matches!(character, '&' | '|' | '<' | '>' | '^' | '%' | '!' | '"'))
}

#[cfg(not(windows))]
fn launch_session_id_contains_shell_metacharacters(_session_id: &str) -> bool {
    false
}

fn launch_program_candidates(program: &str) -> Vec<&str> {
    if program == "kiro-cli" {
        vec!["kiro-cli", "kiro"]
    } else {
        vec![program]
    }
}

fn platform_supports_visible_launch() -> bool {
    cfg!(windows) || cfg!(target_os = "macos")
}

fn is_launchable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

/// Resolve a provider executable from a set of search directories.
///
/// Windows GUI processes frequently inherit a stale PATH.  Keeping this
/// helper independent from the process environment makes the extension and
/// precedence rules testable, while the caller supplies a merged effective
/// PATH plus known per-user CLI directories.
pub fn find_program_in_paths(program: &str, paths: &[PathBuf]) -> Option<PathBuf> {
    let program_path = Path::new(program);
    if program_path.is_absolute() && is_launchable_file(program_path) {
        return Some(program_path.to_path_buf());
    }

    #[cfg(windows)]
    let names = [
        format!("{program}.exe"),
        format!("{program}.cmd"),
        format!("{program}.bat"),
        program.to_string(),
    ];
    #[cfg(not(windows))]
    let names = [program.to_string()];

    // Prefer a native binary even when an earlier PATH entry only contains a
    // package-manager shim. This matters for GUI launches where npm and the
    // vendor installer can expose the same command through different roots.
    for name in &names {
        for directory in paths {
            let candidate = directory.join(name);
            if is_launchable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn push_unique_launch_path(paths: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidate.is_dir() {
        return;
    }
    let duplicate = paths.iter().any(|existing| {
        if cfg!(windows) {
            existing
                .to_string_lossy()
                .eq_ignore_ascii_case(&candidate.to_string_lossy())
        } else {
            existing == &candidate
        }
    });
    if !duplicate {
        paths.push(candidate);
    }
}

#[cfg(windows)]
fn expand_windows_environment(raw: &str) -> String {
    let mut expanded = String::with_capacity(raw.len());
    let mut remainder = raw;
    while let Some(start) = remainder.find('%') {
        expanded.push_str(&remainder[..start]);
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('%') else {
            expanded.push_str(&remainder[start..]);
            break;
        };
        let variable = &after_start[..end];
        if let Ok(value) = std::env::var(variable) {
            expanded.push_str(&value);
        } else {
            expanded.push('%');
            expanded.push_str(variable);
            expanded.push('%');
        }
        remainder = &after_start[end + 1..];
    }
    if !remainder.is_empty() && !remainder.contains('%') {
        expanded.push_str(remainder);
    }
    expanded
}

fn launch_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&path) {
            push_unique_launch_path(&mut paths, entry);
        }
    }

    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        for (root, key_path) in [
            (HKEY_CURRENT_USER, "Environment"),
            (
                HKEY_LOCAL_MACHINE,
                "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
            ),
        ] {
            if let Ok(key) = RegKey::predef(root).open_subkey(key_path) {
                if let Ok(raw) = key.get_value::<String, &str>("Path") {
                    let expanded = expand_windows_environment(&raw);
                    for entry in std::env::split_paths(std::ffi::OsStr::new(&expanded)) {
                        push_unique_launch_path(&mut paths, entry);
                    }
                }
            }
        }

        if let Some(home) = dirs::home_dir() {
            push_unique_launch_path(&mut paths, home.join("AppData").join("Roaming").join("npm"));
            push_unique_launch_path(
                &mut paths,
                home.join("AppData")
                    .join("Local")
                    .join("OpenAI")
                    .join("Codex")
                    .join("bin"),
            );
            push_unique_launch_path(&mut paths, home.join(".grok").join("bin"));
        }
    }

    paths
}

fn resolve_launch_program(program: &str) -> Option<PathBuf> {
    find_program_in_paths(program, &launch_search_paths())
}

#[cfg(windows)]
fn windows_cmd_executable() -> PathBuf {
    let system_root = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let cmd = system_root.join("System32").join("cmd.exe");
    if cmd.is_file() {
        cmd
    } else {
        PathBuf::from("cmd.exe")
    }
}

fn resolve_launch_invocation(program: &str, args: &[String]) -> Option<(PathBuf, Vec<String>)> {
    let resolved = resolve_launch_program(program)?;

    #[cfg(windows)]
    {
        let extension = resolved
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "cmd" | "bat") {
            // `call` is required for batch shims. The session ID is validated
            // above and all other arguments are generated by this module.
            let mut command_args = vec![
                "/d".to_string(),
                "/s".to_string(),
                "/c".to_string(),
                "call".to_string(),
                resolved.to_string_lossy().into_owned(),
            ];
            command_args.extend(args.iter().cloned());
            return Some((windows_cmd_executable(), command_args));
        }
    }

    Some((resolved, args.to_vec()))
}

#[cfg(any(test, target_os = "macos"))]
fn posix_shell_escape_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

#[cfg(any(test, target_os = "macos"))]
fn build_posix_visible_command(
    program: &Path,
    args: &[String],
    workspace: Option<&Path>,
) -> String {
    let invocation = std::iter::once(program.to_string_lossy().into_owned())
        .chain(args.iter().cloned())
        .map(|value| posix_shell_escape_argument(&value))
        .collect::<Vec<_>>()
        .join(" ");
    match workspace {
        Some(workspace) => format!(
            "cd {} && exec {invocation}",
            posix_shell_escape_argument(&workspace.to_string_lossy())
        ),
        None => format!("exec {invocation}"),
    }
}

#[cfg(windows)]
fn windows_visible_console_flags() -> u32 {
    0x0000_0010
}

#[cfg(windows)]
fn launch_visible_invocation(
    program: &Path,
    args: &[String],
    workspace: Option<&Path>,
) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;

    let mut command = Command::new(program);
    command.args(args);
    if let Some(workspace) = workspace {
        command.current_dir(workspace);
    }
    command.creation_flags(windows_visible_console_flags());
    command.spawn().map(|_| ())
}

#[cfg(target_os = "macos")]
fn launch_visible_invocation(
    program: &Path,
    args: &[String],
    workspace: Option<&Path>,
) -> std::io::Result<()> {
    let command = build_posix_visible_command(program, args, workspace);
    let escaped = command.replace('\\', "\\\\").replace('"', "\\\"");
    let script =
        format!("tell application \"Terminal\"\nactivate\ndo script \"{escaped}\"\nend tell");
    let status = Command::new("osascript").arg("-e").arg(script).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            "Terminal.app rejected the resume command",
        ))
    }
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn launch_visible_invocation(
    _program: &Path,
    _args: &[String],
    _workspace: Option<&Path>,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no supported visible terminal launcher",
    ))
}

fn pi_agent_session_path(session_id: &str) -> PathBuf {
    // `PI_AGENT_HOME` is the Pi-Agent root, not its `sessions` directory.
    // Every transferred session is written/read at `<root>/sessions/<id>.jsonl`.
    let home = match std::env::var_os("PI_AGENT_HOME") {
        Some(home) => PathBuf::from(home),
        None => dirs::home_dir()
            .unwrap_or_default()
            .join(".pi")
            .join("agent"),
    };
    home.join("sessions").join(format!("{session_id}.jsonl"))
}

pub fn launch_transferred_session_sync(
    request: LaunchTransferredSessionRequest,
) -> Result<LaunchTransferredSessionResult, SessionTransferError> {
    let provider_id = request.target_provider.trim().to_string();
    let provider_id_for_result = || {
        ui_provider_id_for_slug(
            ProviderRegistry::default_registry()
                .find_by_alias(&casr_alias_for_provider(&provider_id).expect("validated alias"))
                .expect("validated provider")
                .slug(),
        )
    };
    let spec = match build_launch_spec(
        &provider_id,
        &request.session_id,
        request.workspace.as_deref(),
    ) {
        Ok(spec) => spec,
        Err(error) if error.code == "launch_unsupported" => {
            return Ok(LaunchTransferredSessionResult {
                launched: false,
                provider_id: provider_id_for_result(),
                session_id: request.session_id.trim().to_string(),
                workspace: normalize_workspace_path(request.workspace.as_deref())?
                    .map(|path| path.to_string_lossy().into_owned()),
                warning: Some(error.message),
            });
        }
        Err(error) => return Err(error),
    };

    if !platform_supports_visible_launch() {
        return Ok(LaunchTransferredSessionResult {
            launched: false,
            provider_id: provider_id_for_result(),
            session_id: request.session_id.trim().to_string(),
            workspace: spec
                .workspace
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            warning: Some(SessionTransferError::visible_terminal_unavailable().message),
        });
    }

    let mut resolved_invocation = None;
    for program in launch_program_candidates(&spec.program) {
        if let Some(invocation) = resolve_launch_invocation(program, &spec.args) {
            resolved_invocation = Some(invocation);
            break;
        }
    }

    let Some((program, args)) = resolved_invocation else {
        log::warn!(
            "target provider executable not found program={}",
            spec.program
        );
        return Ok(LaunchTransferredSessionResult {
            launched: false,
            provider_id: provider_id_for_result(),
            session_id: request.session_id.trim().to_string(),
            workspace: spec
                .workspace
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            warning: Some(SessionTransferError::target_executable_unavailable().message),
        });
    };

    match launch_visible_invocation(&program, &args, spec.workspace.as_deref()) {
        Ok(_) => Ok(LaunchTransferredSessionResult {
            launched: true,
            provider_id: provider_id_for_result(),
            session_id: request.session_id.trim().to_string(),
            workspace: spec
                .workspace
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            warning: None,
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(LaunchTransferredSessionResult {
                launched: false,
                provider_id: provider_id_for_result(),
                session_id: request.session_id.trim().to_string(),
                workspace: spec
                    .workspace
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                warning: Some(SessionTransferError::target_executable_unavailable().message),
            })
        }
        Err(error) => {
            log::warn!(
                "target provider launch failed program={} error_type={}",
                program.display(),
                error.kind()
            );
            Err(SessionTransferError::launch_failed())
        }
    }
}

fn casr_error_type(error: &(dyn std::error::Error + 'static)) -> &'static str {
    let rendered = error.to_string().to_ascii_lowercase();
    if rendered.contains("rollback failed") {
        "rollback"
    } else if rendered.contains("schema") {
        "schema"
    } else if rendered.contains("validation") {
        "validation"
    } else if rendered.contains("verify") || rendered.contains("read back") {
        "verification"
    } else if rendered.contains("conflict") || rendered.contains("already exists") {
        "conflict"
    } else if rendered.contains("read") {
        "read"
    } else if rendered.contains("write") {
        "write"
    } else {
        "conversion"
    }
}

fn classify_conversion_error(error: &(dyn std::error::Error + 'static)) -> SessionTransferError {
    let rendered = error.to_string().to_ascii_lowercase();
    if rendered.contains("rollback failed") {
        SessionTransferError::rollback_failed()
    } else if rendered.contains("already exists") {
        SessionTransferError::target_conflict()
    } else if rendered.contains("read-back")
        || rendered.contains("verification")
        || rendered.contains("could not be read back")
    {
        SessionTransferError::verify_failed()
    } else if rendered.contains("schema") {
        SessionTransferError::target_schema_mismatch()
    } else if rendered.contains("validation failed") {
        SessionTransferError::conversion_failed("validation")
    } else {
        SessionTransferError::conversion_failed("write")
    }
}

pub async fn list_session_transfer_targets(
) -> Result<Vec<SessionTransferTarget>, SessionTransferError> {
    tauri::async_runtime::spawn_blocking(list_session_transfer_targets_sync)
        .await
        .map_err(|_| SessionTransferError::conversion_failed("capabilities"))
}

pub async fn transfer_session(
    request: SessionTransferRequest,
) -> Result<SessionTransferResult, SessionTransferError> {
    tauri::async_runtime::spawn_blocking(move || transfer_session_sync(request))
        .await
        .map_err(|_| SessionTransferError::conversion_failed("worker"))?
}

pub async fn launch_transferred_session(
    request: LaunchTransferredSessionRequest,
) -> Result<LaunchTransferredSessionResult, SessionTransferError> {
    tauri::async_runtime::spawn_blocking(move || launch_transferred_session_sync(request))
        .await
        .map_err(|_| SessionTransferError::conversion_failed("launcher"))?
}

fn normalize_provider_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['_', ' '], "-")
}

fn normalize_identity_path(path: &Path) -> String {
    let rendered = path.to_string_lossy().into_owned();
    if cfg!(windows) {
        rendered.to_ascii_lowercase()
    } else {
        rendered
    }
}

/// Return a stable identity for a native/virtual path when merging discovery.
pub(crate) fn session_path_identity(provider_id: &str, source_path: &str) -> String {
    // Normalize provider-specific virtual spellings first. This keeps legacy
    // `sqlite:<db>:<session-id>` paths equivalent to CASR's encoded
    // `<db>/<session-id>` paths even when the ID contains slashes, spaces, or
    // colons. Keep the older `:ses_` fallback for providers that are not
    // represented in CASR's registry yet.
    let normalized_source = normalize_source_path_for_provider(provider_id, source_path)
        .ok()
        .or_else(|| {
            let rest = source_path.strip_prefix("sqlite:")?;
            let separator = rest.rfind(":ses_")?;
            let db = Path::new(&rest[..separator]);
            Some(db.join(&rest[separator + 1..]))
        });
    let path = normalized_source
        .as_deref()
        .unwrap_or_else(|| Path::new(source_path));
    if let Some(parent) = path.parent().filter(|parent| parent.is_file()) {
        let parent = parent
            .canonicalize()
            .unwrap_or_else(|_| parent.to_path_buf());
        let leaf = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        return format!(
            "{}#{}#{}",
            normalize_provider_token(provider_id),
            normalize_identity_path(&parent),
            leaf
        );
    }

    let normalized = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    format!(
        "{}#{}",
        normalize_provider_token(provider_id),
        normalize_identity_path(&normalized)
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{build_posix_visible_command, launch_program_candidates, session_path_identity};

    #[cfg(unix)]
    use super::find_program_in_paths;

    #[cfg(windows)]
    use super::windows_visible_console_flags;

    #[test]
    fn posix_visible_command_quotes_every_external_value() {
        let command = build_posix_visible_command(
            Path::new("/tmp/tool path/cli"),
            &["--resume".to_string(), "session;echo injected".to_string()],
            Some(Path::new("/tmp/$project")),
        );

        assert_eq!(
            command,
            "cd '/tmp/$project' && exec '/tmp/tool path/cli' '--resume' 'session;echo injected'"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_launch_requests_a_visible_console() {
        assert_eq!(windows_visible_console_flags(), 0x0000_0010);
    }

    #[cfg(unix)]
    #[test]
    fn unix_program_lookup_rejects_files_without_execute_permission() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("tempdir");
        let program = directory.path().join("codex");
        std::fs::write(&program, "#!/bin/sh\nexit 0\n").expect("program fixture");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o644))
            .expect("non-executable permissions");
        assert_eq!(
            find_program_in_paths("codex", &[directory.path().to_path_buf()]),
            None
        );

        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("executable permissions");
        assert_eq!(
            find_program_in_paths("codex", &[directory.path().to_path_buf()]),
            Some(program)
        );
    }

    #[test]
    fn identity_matches_legacy_and_encoded_database_paths_for_arbitrary_ids() {
        let directory = tempfile::tempdir().expect("tempdir");
        let database = directory.path().join("opencode.db");
        std::fs::write(&database, "fixture").expect("database fixture");

        let legacy = format!("sqlite:{}:session/with spaces", database.display());
        let virtual_path = database.join("session%2Fwith%20spaces");

        assert_eq!(
            session_path_identity("opencode", &legacy),
            session_path_identity("opencode", &virtual_path.to_string_lossy()),
        );
    }

    #[test]
    fn identity_matches_colon_bearing_database_session_ids() {
        let directory = tempfile::tempdir().expect("tempdir");
        let database = directory.path().join("opencode.db");
        std::fs::write(&database, "fixture").expect("database fixture");

        let legacy = format!("sqlite:{}:session:part", database.display());
        let virtual_path = database.join("session%3Apart");

        assert_eq!(
            session_path_identity("opc", &legacy),
            session_path_identity("opc", &virtual_path.to_string_lossy()),
        );
    }

    #[test]
    fn identity_keeps_same_database_leaf_distinct_across_providers() {
        let directory = tempfile::tempdir().expect("tempdir");
        let database = directory.path().join("shared.db");
        std::fs::write(&database, "fixture").expect("write database");
        let path = database.join("session-1");

        assert_ne!(
            session_path_identity("opencode", &path.to_string_lossy()),
            session_path_identity("cursor", &path.to_string_lossy()),
        );
    }

    #[test]
    fn identity_keeps_virtual_session_id_case_distinct() {
        let directory = tempfile::tempdir().expect("tempdir");
        let database = directory.path().join("opencode.db");
        std::fs::write(&database, "fixture").expect("write database");

        let upper = database.join("Session-A");
        let lower = database.join("session-a");

        assert_ne!(
            session_path_identity("opencode", &upper.to_string_lossy()),
            session_path_identity("opencode", &lower.to_string_lossy()),
            "virtual session IDs are case-sensitive and must not be merged",
        );
    }

    #[test]
    fn rollback_failure_is_exposed_as_a_distinct_error_code() {
        let source_error = anyhow::anyhow!(
            "Written file(s) could not be read back; rollback failed: access denied"
        );
        let error = super::classify_conversion_error(source_error.as_ref());

        assert_eq!(error.code, "rollback_failed");
        assert!(!error.message.contains("access denied"));
    }

    #[test]
    fn kiro_launcher_accepts_both_cli_binary_names() {
        assert_eq!(
            launch_program_candidates("kiro-cli"),
            vec!["kiro-cli", "kiro"]
        );
        assert_eq!(launch_program_candidates("codex"), vec!["codex"]);
    }
}
