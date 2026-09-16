//! Reversible CC2CX Hosts transaction for Cursor transparent entry.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::ErrorKind,
    net::IpAddr,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::error::{CursorError, Result};

const BEGIN_MARKER: &str = "# BEGIN CC2CX CURSOR";
const END_MARKER: &str = "# END CC2CX CURSOR";

#[derive(Debug, Clone)]
pub struct HostsTransaction {
    path: PathBuf,
    original: Vec<u8>,
    applied_sha256: String,
}

impl HostsTransaction {
    pub fn applied_sha256(&self) -> &str {
        &self.applied_sha256
    }
}

pub fn system_path() -> PathBuf {
    #[cfg(windows)]
    {
        let root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        root.join("System32")
            .join("drivers")
            .join("etc")
            .join("hosts")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/hosts")
    }
}

pub fn require_writable(path: &Path) -> Result<()> {
    match OpenOptions::new().write(true).open(path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => {
            Err(CursorError::Config(format!(
                "Hosts 事务需要提升权限，未修改任何 Hosts 文件: {}",
                path.display()
            )))
        }
        Err(error) => Err(error.into()),
    }
}

pub fn enable(path: &Path, entries: &[(IpAddr, &str)]) -> Result<HostsTransaction> {
    validate_entries(entries)?;
    let current = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    let original = user_bytes_without_managed_block(&current)?;
    if managed_entries_match(&current, entries) {
        return Ok(HostsTransaction {
            path: path.to_path_buf(),
            original,
            applied_sha256: sha256_hex(&current),
        });
    }
    if path.exists() {
        if let Err(error) = require_writable(path) {
            if managed_entries_are_subset(&current, entries) {
                return Ok(HostsTransaction {
                    path: path.to_path_buf(),
                    original,
                    applied_sha256: sha256_hex(&current),
                });
            }
            return Err(error);
        }
    }
    let applied = apply_managed_block(&original, entries);
    let applied_sha256 = sha256_hex(&applied);
    if current != applied {
        crate::config::atomic_write(path, &applied)
            .map_err(|error| CursorError::Config(error.to_string()))?;
    }
    Ok(HostsTransaction {
        path: path.to_path_buf(),
        original,
        applied_sha256,
    })
}

pub fn restore(transaction: &HostsTransaction) -> Result<()> {
    let current = match fs::read(&transaction.path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    if current == transaction.original {
        return Ok(());
    }
    if sha256_hex(&current) != transaction.applied_sha256 {
        return Err(CursorError::ExternalModification(
            transaction.path.display().to_string(),
        ));
    }
    crate::config::atomic_write(&transaction.path, &transaction.original)
        .map_err(|error| CursorError::Config(error.to_string()))?;
    Ok(())
}

pub fn recover(path: &Path, backup: &Path) -> Result<()> {
    let original = fs::read(backup)?;
    let current = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    if current == original {
        return Ok(());
    }
    let user_bytes = user_bytes_without_managed_block(&current)?;
    if user_bytes != original {
        return Err(CursorError::ExternalModification(
            path.display().to_string(),
        ));
    }
    crate::config::atomic_write(path, &original)
        .map_err(|error| CursorError::Config(error.to_string()))?;
    Ok(())
}

fn managed_entries_from(bytes: &[u8]) -> Option<Vec<(IpAddr, String)>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let begin = find_marker_line(text, BEGIN_MARKER)?;
    let rest = &text[begin.end..];
    let end_rel = find_marker_line(rest, END_MARKER)?;
    let block = &rest[..end_rel.start];
    let mut entries = Vec::new();
    for line in block.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let ip = parts.next()?.parse().ok()?;
        let host = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        entries.push((ip, host.to_ascii_lowercase()));
    }
    Some(entries)
}

fn managed_entries_are_subset(current: &[u8], wanted: &[(IpAddr, &str)]) -> bool {
    let Some(existing) = managed_entries_from(current) else {
        return false;
    };
    !existing.is_empty()
        && existing.iter().all(|(ip, host)| {
            wanted.iter().any(|(want_ip, want_host)| {
                ip == want_ip && host == &want_host.to_ascii_lowercase()
            })
        })
}

fn managed_entries_match(current: &[u8], wanted: &[(IpAddr, &str)]) -> bool {
    let Some(existing) = managed_entries_from(current) else {
        return false;
    };
    existing.len() == wanted.len()
        && existing
            .iter()
            .zip(wanted)
            .all(|(have, want)| have.0 == want.0 && have.1 == want.1.to_ascii_lowercase())
}

fn validate_entries(entries: &[(IpAddr, &str)]) -> Result<()> {
    let mut seen = HashSet::new();
    for (ip, host) in entries {
        if !ip.is_loopback() {
            return Err(CursorError::Config(format!(
                "Hosts 事务只允许 loopback 地址: {ip}"
            )));
        }
        let host = host.trim();
        if host.is_empty() || host.split_whitespace().nth(1).is_some() {
            return Err(CursorError::Config(format!("Hosts 主机名无效: {host}")));
        }
        let key = host.to_ascii_lowercase();
        if !seen.insert(key) {
            return Err(CursorError::Config(format!(
                "Hosts 事务包含重复主机名: {host}"
            )));
        }
    }
    Ok(())
}

fn user_bytes_without_managed_block(bytes: &[u8]) -> Result<Vec<u8>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let text = std::str::from_utf8(bytes).map_err(|error| CursorError::Parse(error.to_string()))?;
    let Some(begin) = find_marker_line(text, BEGIN_MARKER) else {
        return Ok(bytes.to_vec());
    };
    let rest = &text[begin.end..];
    let Some(end_rel) = find_marker_line(rest, END_MARKER) else {
        return Err(CursorError::Parse(
            "Hosts 文件含有未闭合的 CC2CX 标记块".to_string(),
        ));
    };
    let end = begin.end + end_rel.end;
    let mut original = text[..begin.start].to_string();
    original.push_str(&text[end..]);
    Ok(original.into_bytes())
}

fn apply_managed_block(original: &[u8], entries: &[(IpAddr, &str)]) -> Vec<u8> {
    let mut out = original.to_vec();
    if !out.is_empty() && !out.ends_with(b"\n") {
        out.push(b'\n');
    }
    out.extend_from_slice(BEGIN_MARKER.as_bytes());
    out.push(b'\n');
    for (ip, host) in entries {
        out.extend_from_slice(format!("{ip} {}\n", host.trim()).as_bytes());
    }
    out.extend_from_slice(END_MARKER.as_bytes());
    out.push(b'\n');
    out
}

struct MarkerSpan {
    start: usize,
    end: usize,
}

fn find_marker_line(text: &str, marker: &str) -> Option<MarkerSpan> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if content == marker {
            return Some(MarkerSpan {
                start: offset,
                end: offset + line.len(),
            });
        }
        offset += line.len();
    }
    None
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
