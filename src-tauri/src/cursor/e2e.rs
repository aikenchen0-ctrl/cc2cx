//! Isolated Cursor E2E contract helpers. These never launch Cursor or write Hosts.

use std::path::PathBuf;

use super::{
    error::{CursorError, Result},
    profile::CursorProfile,
};

pub fn default_cursor_user_data() -> PathBuf {
    match std::env::consts::OS {
        "windows" => std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate::config::get_home_dir().join("AppData/Roaming"))
            .join("Cursor"),
        "macos" => crate::config::get_home_dir().join("Library/Application Support/Cursor"),
        _ => crate::config::get_home_dir().join(".config/Cursor"),
    }
}

/// Resolve an isolated profile and transaction-backup path for one E2E run.
///
/// The default profile is scoped by the runner process id so a stale transaction created by a
/// previous elevation level or a crashed Cursor process cannot block a fresh diagnostic run. An
/// explicit profile remains supported for repeatable tests; its backup is kept inside that
/// profile instead of the shared application directory.
pub fn isolated_profile_paths(
    app_dir: &std::path::Path,
    profile_override: Option<PathBuf>,
    run_id: u32,
) -> (PathBuf, PathBuf) {
    match profile_override {
        Some(profile) => (
            profile.clone(),
            profile.join("cc2cx-e2e-settings-backup.json"),
        ),
        None => (
            app_dir.join(format!("cursor-e2e-profile-{run_id}")),
            app_dir.join(format!("cursor-e2e-settings-backup-{run_id}.json")),
        ),
    }
}

pub fn require_isolated_profile(profile: &CursorProfile) -> Result<()> {
    let default = default_cursor_user_data();
    if profile.root() == default || profile.root().starts_with(&default) {
        return Err(CursorError::Config(
            "E2E 必须使用隔离 profile，禁止默认 Cursor 用户目录".to_string(),
        ));
    }
    Ok(())
}

pub fn require_ca(ready: bool, skip: bool) -> Result<()> {
    if ready || skip {
        Ok(())
    } else {
        Err(CursorError::Config(
            "Cursor E2E CA is not trusted; run with an explicit CA install action first"
                .to_string(),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorE2eEntryMode {
    Proxy,
    Transparent,
}

/// Resolve the E2E entry mode from an optional explicit override.
///
/// Windows/Linux retain the transparent default, while macOS retains the proxy default because
/// the current runner has no privileged helper for `/etc/hosts` and port 443. An explicit mode is
/// useful for local diagnostics and does not alter the normal application path.
pub fn configured_entry_mode(os: &str, requested: Option<&str>) -> Result<CursorE2eEntryMode> {
    match requested.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) if value.eq_ignore_ascii_case("proxy") => Ok(CursorE2eEntryMode::Proxy),
        Some(value) if value.eq_ignore_ascii_case("transparent") => {
            Ok(CursorE2eEntryMode::Transparent)
        }
        Some(_) => Err(CursorError::Config(
            "CC2CX_E2E_ENTRY_MODE 必须是 proxy 或 transparent".to_string(),
        )),
        None if os.eq_ignore_ascii_case("macos") => Ok(CursorE2eEntryMode::Proxy),
        None => Ok(CursorE2eEntryMode::Transparent),
    }
}

/// One E2E run may enable Hosts/fake-IP or http.proxy, never both.
pub fn require_exclusive_entry(
    managed_hosts: bool,
    proxy_url: Option<&str>,
) -> Result<CursorE2eEntryMode> {
    let proxy_url = proxy_url.filter(|url| !url.is_empty());
    match (managed_hosts, proxy_url) {
        (true, None) => Ok(CursorE2eEntryMode::Transparent),
        (false, Some(_)) => Ok(CursorE2eEntryMode::Proxy),
        (true, Some(_)) => Err(CursorError::Config(
            "E2E 一次只能启用一种入口：Hosts 与 http.proxy 不能同时打开".to_string(),
        )),
        (false, None) => Err(CursorError::Config("E2E 未启用任何入口".to_string())),
    }
}

pub fn safe_metadata_line(key: &str, value: &str) -> Result<String> {
    let rendered = format!("{key}={value}");
    let lower = rendered.to_ascii_lowercase();
    const FORBIDDEN: [&str; 6] = [
        "authorization",
        "cookie",
        "token",
        "api_key",
        "apikey",
        "bearer",
    ];
    if FORBIDDEN.iter().any(|item| lower.contains(item)) {
        return Err(CursorError::Config("E2E 元数据禁止包含凭证".to_string()));
    }
    Ok(rendered)
}
