//! Provider trait and concrete provider implementations.
//!
//! Each supported provider (Claude Code, Codex, Gemini CLI, Antigravity CLI,
//! Cursor, Cline, Aider, Amp, OpenCode, ChatGPT, ClawdBot, Vibe, Factory,
//! OpenClaw, Pi-Agent, Kiro, Grok Build) implements the [`Provider`] trait to
//! read/write sessions in its native format.

pub mod aider;
pub mod amp;
pub mod antigravity;
pub mod chatgpt;
pub mod claude_code;
pub mod clawdbot;
pub mod cline;
pub mod codex;
pub mod cursor;
pub mod factory;
pub mod gemini;
pub mod grok;
pub mod kiro;
pub mod openclaw;
pub mod opencode;
pub mod pi_agent;
pub mod vibe;

use std::path::{Path, PathBuf};

use crate::discovery::DetectionResult;
use crate::model::CanonicalSession;

/// Validate a session ID before using it as a native path component.
///
/// Provider session IDs are untrusted data: they can come from imported
/// session files or a renderer request. Rejecting path syntax here keeps
/// file-based writers inside their configured session root on every platform.
pub(crate) fn safe_session_id_component(raw: &str) -> anyhow::Result<String> {
    if raw.is_empty() || raw.len() > 4096 || raw != raw.trim() {
        anyhow::bail!("invalid session id: empty, padded, or too long");
    }
    if raw == "." || raw == ".." {
        anyhow::bail!("invalid session id: path traversal component");
    }
    if raw.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            )
    }) {
        anyhow::bail!("invalid session id: path syntax");
    }
    if raw.ends_with('.') || raw.ends_with(' ') {
        anyhow::bail!("invalid session id: trailing dot or space");
    }

    // Windows reserves these device names even when an extension is added.
    let base = raw.split('.').next().unwrap_or(raw);
    let upper = base.to_ascii_uppercase();
    let reserved_numbered_device = if upper.len() == 4 && upper.is_ascii() {
        let bytes = upper.as_bytes();
        (&bytes[..3] == b"COM" || &bytes[..3] == b"LPT") && (b'1'..=b'9').contains(&bytes[3])
    } else {
        false
    };
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") || reserved_numbered_device {
        anyhow::bail!("invalid session id: reserved device name");
    }

    Ok(raw.to_owned())
}

/// Compatibility alias used by callers that describe the same boundary as a
/// provider-root path check.
pub(crate) fn safe_session_id_for_path(raw: &str) -> anyhow::Result<String> {
    safe_session_id_component(raw)
}

#[cfg(test)]
mod session_id_tests {
    use super::safe_session_id_for_path;

    #[test]
    fn rejects_session_ids_that_can_escape_a_provider_root() {
        for value in [
            "..\\escaped",
            "../escaped",
            "C:\\Windows\\system32",
            "/tmp/escaped",
            "..",
            ".",
            "session\0id",
            "session:id",
        ] {
            assert!(
                safe_session_id_for_path(value).is_err(),
                "unsafe session id must be rejected: {value:?}"
            );
        }
    }

    #[test]
    fn preserves_safe_session_ids() {
        for value in ["session-123", "2026-09-16T01-15-00", "中文会话"] {
            assert_eq!(safe_session_id_for_path(value).unwrap(), value);
        }
    }
}

/// Describes whether a provider can be used as a conversion write target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteSupport {
    /// The provider exposes a native writer suitable for normal conversion.
    Supported,
    /// The provider can be read/resumed but must never be written by CASR.
    ReadOnly { reason: String },
    /// The provider may be writable after a runtime/schema check.
    Conditional { reason: String },
}

/// Provider metadata used by UI clients before they start a conversion.
#[derive(Debug, Clone)]
pub struct ProviderCapability {
    pub slug: String,
    pub alias: String,
    pub name: String,
    pub detection: DetectionResult,
    pub write_support: WriteSupport,
}

/// Options controlling how a session is written to disk.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Overwrite existing session file (creates `.bak` backup).
    pub force: bool,
}

/// Describes the files produced by a successful write operation.
#[derive(Debug, Clone)]
pub struct WrittenSession {
    /// Provider-native paths used to read the written session back.
    ///
    /// Some providers expose a virtual path here (for example
    /// `database/session-id`) even though the physical write happened to a
    /// database or a shared history file. Use `rollback_paths` for cleanup.
    pub paths: Vec<PathBuf>,
    /// Session ID in the target provider's format.
    pub session_id: String,
    /// Ready-to-paste command to resume the session.
    pub resume_command: String,
    /// Path to the backup retained for rollback, if the writer touched an
    /// existing file/database. SQLite providers expose a temporary online
    /// snapshot here until read-back verification completes; the conversion
    /// pipeline removes that snapshot after successful verification.
    pub backup_path: Option<PathBuf>,
    /// Physical files changed by the write and their exact backups.
    ///
    /// This is intentionally separate from `paths`: a provider may write
    /// several files, may expose a virtual read path, and may back up a shared
    /// index file that is not itself a session path.
    pub rollback_paths: Vec<RollbackPath>,
    /// Non-fatal warnings produced while writing (e.g. the target session was
    /// written but could not be registered in the provider's resume index).
    /// Surfaced to the user and merged into the conversion's warning list.
    pub warnings: Vec<String>,
}

/// One physical file touched by a provider writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackPath {
    pub target_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

impl RollbackPath {
    pub fn new(target_path: PathBuf, backup_path: Option<PathBuf>) -> Self {
        Self {
            target_path,
            backup_path,
        }
    }
}

/// The core abstraction each provider implements.
///
/// Object-safe so we can store `Box<dyn Provider>` in the registry.
pub trait Provider: Send + Sync {
    /// Human-readable name (e.g. `"Claude Code"`).
    fn name(&self) -> &str;

    /// Short slug used in session metadata (e.g. `"claude-code"`).
    fn slug(&self) -> &str;

    /// CLI alias used in `casr resume <alias> …` (e.g. `"cc"`).
    fn cli_alias(&self) -> &str;

    /// Probe whether this provider is installed on the machine.
    fn detect(&self) -> DetectionResult;

    /// Root directories where this provider stores sessions.
    fn session_roots(&self) -> Vec<PathBuf>;

    /// Check if `session_id` belongs to this provider; return the file path if so.
    fn owns_session(&self, session_id: &str) -> Option<PathBuf>;

    /// Read a session from its native format into canonical IR.
    fn read_session(&self, path: &Path) -> anyhow::Result<CanonicalSession>;

    /// Write a canonical session into this provider's native format.
    fn write_session(
        &self,
        session: &CanonicalSession,
        opts: &WriteOptions,
    ) -> anyhow::Result<WrittenSession>;

    /// Describe whether this provider is a safe conversion write target.
    ///
    /// Most providers have a native writer and are supported by default. A
    /// provider with a static read-only boundary or a runtime schema gate can
    /// override this method without changing the object-safe trait contract.
    fn write_support(&self) -> WriteSupport {
        WriteSupport::Supported
    }

    /// Build the shell command to resume a session with this provider.
    fn resume_command(&self, session_id: &str) -> String;

    /// Enumerate all discoverable sessions for this provider.
    ///
    /// Returns `Some(vec)` of `(session_id, path)` pairs when the provider
    /// stores multiple sessions in a single file or database and directory
    /// walking alone would undercount.  The default returns `None`, which
    /// tells the caller to fall back to directory walking + `read_session`.
    fn list_sessions(&self) -> Option<Vec<(String, PathBuf)>> {
        None
    }
}
