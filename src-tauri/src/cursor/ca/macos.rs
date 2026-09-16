//! macOS trust-store integration for the Cursor-local CA.
//!
//! macOS has two separate concepts here: the certificate object in a keychain and the user's
//! trust settings. Both operations must stay in the logged-in user's security domain. In
//! particular, `security -d` and `/Library/Keychains/System.keychain` are intentionally not used:
//! they target the administrator/system domain and require privileges that the desktop app does
//! not own.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use sha1::{Digest, Sha1};

use super::super::error::{CursorError, Result};

const SECURITY_TOOL: &str = "/usr/bin/security";
const LOGIN_KEYCHAIN_DB: &str = "Library/Keychains/login.keychain-db";
const LOGIN_KEYCHAIN_LEGACY: &str = "Library/Keychains/login.keychain";

pub(super) fn install(cert_path: &Path) -> Result<()> {
    let keychain = login_keychain_for_operation()?;
    let output = Command::new(SECURITY_TOOL)
        .args(["add-trusted-cert", "-r", "trustRoot", "-p", "ssl", "-k"])
        .arg(&keychain)
        .arg(cert_path)
        .output()
        .map_err(CursorError::Io)?;
    ensure_success("安装 macOS 用户 CA", output)
}

pub(super) fn uninstall(cert: &str, _cert_path: &Path) -> Result<()> {
    let Some(keychain) = existing_login_keychain()? else {
        return Ok(());
    };
    let der = parse_der(cert)?;
    if !contains_exact_certificate(&keychain, &der)? {
        return Ok(());
    }
    let fingerprint = sha1_hex(&der);
    let output = Command::new(SECURITY_TOOL)
        .args(["delete-certificate", "-Z", &fingerprint, "-t"])
        .arg(&keychain)
        .output()
        .map_err(CursorError::Io)?;
    ensure_success("卸载 macOS 用户 CA", output)
}

pub(super) fn is_installed(cert: &str, cert_path: &Path) -> Result<bool> {
    let der = parse_der(cert)?;
    let Some(keychain) = existing_login_keychain()? else {
        return Ok(false);
    };
    if !contains_exact_certificate(&keychain, &der)? {
        return Ok(false);
    }

    // Presence in a keychain is not equivalent to trust. Verify the exact certificate through
    // the user trust domain without supplying `-r`, which would bypass that domain entirely.
    let output = Command::new(SECURITY_TOOL)
        .args(["verify-cert", "-c"])
        .arg(cert_path)
        .args(["-k"])
        .arg(&keychain)
        .args(["-p", "ssl", "-l", "-L", "-q"])
        .output()
        .map_err(CursorError::Io)?;
    Ok(output.status.success())
}

fn parse_der(cert: &str) -> Result<Vec<u8>> {
    pem::parse(cert)
        .map(|pem| pem.into_contents())
        .map_err(|error| CursorError::Config(format!("解析 CA PEM 失败: {error}")))
}

fn existing_login_keychain() -> Result<Option<PathBuf>> {
    // `security login-keychain -d user` reflects the keychain selected for the current
    // user's security domain. Probe it first so custom login keychains are handled
    // consistently by install, verification, and uninstall.
    let output = Command::new(SECURITY_TOOL)
        .args(["login-keychain", "-d", "user"])
        .output()
        .ok();
    if let Some(output) = output {
        if output.status.success() {
            if let Some(path) = parse_login_keychain_output(&output.stdout) {
                if path.is_file() {
                    return Ok(Some(path));
                }
            }
        }
    }

    // Older macOS releases and test environments may not expose the probe. Keep the
    // conventional paths as a compatibility fallback, but never let them override a
    // valid custom keychain returned above.
    let home = crate::config::get_home_dir();
    for relative in [LOGIN_KEYCHAIN_DB, LOGIN_KEYCHAIN_LEGACY] {
        let path = home.join(relative);
        if path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn parse_login_keychain_output(output: &[u8]) -> Option<PathBuf> {
    let rendered = std::str::from_utf8(output).ok()?;
    let line = rendered
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let path = match line.as_bytes() {
        [first, rest @ .., last]
            if (first == &b'"' && last == &b'"') || (first == &b'\'' && last == &b'\'') =>
        {
            std::str::from_utf8(rest).ok()?.trim()
        }
        _ => line,
    };
    if path.is_empty()
        || path.starts_with("security:")
        || path.contains(['\0', '\r', '\n'])
        || !(path.starts_with('/') || Path::new(path).is_absolute())
    {
        return None;
    }
    Some(PathBuf::from(path))
}

fn login_keychain_for_operation() -> Result<PathBuf> {
    if let Some(path) = existing_login_keychain()? {
        return Ok(path);
    }
    let home = crate::config::get_home_dir();
    let fallback = home.join(LOGIN_KEYCHAIN_DB);
    Err(CursorError::Config(format!(
        "无法定位当前用户登录钥匙串: {}",
        fallback.display()
    )))
}

fn contains_exact_certificate(keychain: &Path, wanted_der: &[u8]) -> Result<bool> {
    let output = Command::new(SECURITY_TOOL)
        .args(["find-certificate", "-a", "-p", "-c"])
        .arg(super::CA_COMMON_NAME)
        .arg(keychain)
        .output()
        .map_err(CursorError::Io)?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let not_found = output.status.code() == Some(44)
            || detail.to_ascii_lowercase().contains("could not be found");
        if not_found {
            return Ok(false);
        }
        return Err(CursorError::Config(format!(
            "读取 macOS 用户钥匙串失败: {}",
            detail.trim()
        )));
    }
    let rendered = String::from_utf8_lossy(&output.stdout);
    if rendered.trim().is_empty() {
        return Ok(false);
    }
    let certificates = pem::parse_many(rendered.as_ref())
        .map_err(|error| CursorError::Config(format!("解析 macOS 钥匙串证书失败: {error}")))?;
    Ok(certificates
        .into_iter()
        .any(|certificate| certificate.into_contents() == wanted_der))
}

fn ensure_success(action: &str, output: Output) -> Result<()> {
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    let detail = detail.trim();
    if detail.is_empty() {
        Err(CursorError::Config(format!(
            "{action}失败，security 返回 {}",
            output.status
        )))
    } else {
        Err(CursorError::Config(format!("{action}失败: {detail}")))
    }
}

fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_keychain_probe_parses_quoted_output() {
        assert_eq!(
            parse_login_keychain_output(b"\"/Users/alice/Library/Keychains/custom.keychain-db\"\n"),
            Some(PathBuf::from(
                "/Users/alice/Library/Keychains/custom.keychain-db"
            ))
        );
    }

    #[test]
    fn login_keychain_probe_rejects_empty_or_non_path_output() {
        assert_eq!(parse_login_keychain_output(b"\n"), None);
        assert_eq!(parse_login_keychain_output(b"security: error\n"), None);
    }
}
