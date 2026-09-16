//! Reversible management of Cursor's user-level settings file.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::error::{CursorError, Result};

const PROXY_KEYS: [&str; 5] = [
    "http.proxy",
    "http.proxyKerberosServicePrincipal",
    "http.proxySupport",
    "http.experimental.systemCertificatesV2",
    "cursor.general.disableHttp2",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsBackup {
    path: PathBuf,
    had_file: bool,
    original: Vec<u8>,
    original_sha256: String,
    applied_sha256: String,
}

pub struct CursorSettingsStore {
    path: PathBuf,
    backup_path: PathBuf,
}

impl CursorSettingsStore {
    pub fn new(path: PathBuf, backup_path: PathBuf) -> Self {
        Self { path, backup_path }
    }

    pub fn default_path() -> Result<PathBuf> {
        let home = crate::config::get_home_dir();
        match std::env::consts::OS {
            "macos" => Ok(home.join("Library/Application Support/Cursor/User/settings.json")),
            "windows" => Ok(std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Roaming"))
                .join("Cursor/User/settings.json")),
            "linux" => Ok(std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".config"))
                .join("Cursor/User/settings.json")),
            platform => Err(CursorError::Config(format!(
                "当前平台不支持 Cursor settings: {platform}"
            ))),
        }
    }

    pub fn apply_proxy(&self, proxy_url: &str) -> Result<SettingsBackup> {
        let proxy_url = proxy_url.trim();
        validate_proxy_url(proxy_url)?;
        if self.backup_path.exists() {
            return Err(CursorError::Config(format!(
                "已有未完成的 Cursor 设置备份: {}",
                self.backup_path.display()
            )));
        }

        let (original, had_file) = match fs::read(&self.path) {
            Ok(bytes) => (bytes, true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Vec::new(), false),
            Err(error) => return Err(error.into()),
        };
        let mut settings = if original.is_empty() {
            Value::Object(Map::new())
        } else {
            let source = String::from_utf8(original.clone())
                .map_err(|error| CursorError::Parse(error.to_string()))?;
            json5::from_str(&source).map_err(|error| CursorError::Parse(error.to_string()))?
        };
        if !settings.is_object() {
            return Err(CursorError::Parse(
                "Cursor settings 根值必须是对象".to_string(),
            ));
        }

        let root = settings.as_object_mut().expect("object checked above");
        root.insert(
            "http.proxy".to_string(),
            Value::String(proxy_url.to_string()),
        );
        root.insert(
            "http.proxyKerberosServicePrincipal".to_string(),
            Value::String(proxy_url.to_string()),
        );
        root.insert(
            "http.proxySupport".to_string(),
            Value::String("on".to_string()),
        );
        root.insert(
            "http.experimental.systemCertificatesV2".to_string(),
            Value::Bool(true),
        );
        root.insert("cursor.general.disableHttp2".to_string(), Value::Bool(true));

        let mut changed = serde_json::to_vec_pretty(&settings)
            .map_err(|error| CursorError::Parse(error.to_string()))?;
        changed.push(b'\n');
        let backup = SettingsBackup {
            path: self.path.clone(),
            had_file,
            original_sha256: sha256(&original),
            applied_sha256: sha256(&changed),
            original,
        };

        // Re-read immediately before publishing the backup and replacement. This closes the
        // common Cursor-save race; restore still performs a second hash check for later edits.
        let current = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        if sha256(&current) != backup.original_sha256 {
            return Err(CursorError::ExternalModification(
                self.path.display().to_string(),
            ));
        }
        self.persist_backup(&backup)?;
        if let Err(error) = crate::config::atomic_write(&self.path, &changed) {
            let _ = fs::remove_file(&self.backup_path);
            return Err(CursorError::Config(error.to_string()));
        }
        Ok(backup)
    }

    pub fn restore(&self) -> Result<()> {
        let backup = self.load_backup()?;
        if backup.path != self.path {
            return Err(CursorError::Config(
                "备份文件路径与当前 settings 不一致".to_string(),
            ));
        }
        let current = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        // A crash after persisting the backup but before replacing settings leaves the
        // original bytes in place. It is safe to discard that uncommitted transaction.
        if sha256(&current) == backup.original_sha256 {
            fs::remove_file(&self.backup_path)?;
            return Ok(());
        }
        if sha256(&current) != backup.applied_sha256 {
            return Err(CursorError::ExternalModification(
                self.path.display().to_string(),
            ));
        }

        self.restore_raw(&backup)?;
        fs::remove_file(&self.backup_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CursorError::Config("恢复后备份文件已不存在".to_string())
            } else {
                CursorError::Io(error)
            }
        })?;
        Ok(())
    }

    pub fn has_backup(&self) -> bool {
        self.backup_path.exists()
    }

    /// Remove Mode P proxy keys so Mode T cannot inherit a stale `http.proxy`.
    pub fn clear_proxy_keys(&self) -> Result<()> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        if bytes.is_empty() {
            return Ok(());
        }
        let source =
            String::from_utf8(bytes).map_err(|error| CursorError::Parse(error.to_string()))?;
        let mut settings: Value =
            json5::from_str(&source).map_err(|error| CursorError::Parse(error.to_string()))?;
        let Some(root) = settings.as_object_mut() else {
            return Err(CursorError::Parse(
                "Cursor settings 根值必须是对象".to_string(),
            ));
        };
        let mut changed = false;
        for key in PROXY_KEYS {
            if root.remove(key).is_some() {
                changed = true;
            }
        }
        if !changed {
            return Ok(());
        }
        let mut serialized = serde_json::to_vec_pretty(&settings)
            .map_err(|error| CursorError::Parse(error.to_string()))?;
        serialized.push(b'\n');
        crate::config::atomic_write(&self.path, &serialized)
            .map_err(|error| CursorError::Config(error.to_string()))
    }

    pub fn matches_proxy(&self, proxy_url: &str) -> Result<bool> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let source =
            String::from_utf8(bytes).map_err(|error| CursorError::Parse(error.to_string()))?;
        let settings: Value =
            json5::from_str(&source).map_err(|error| CursorError::Parse(error.to_string()))?;
        Ok(
            settings.get("http.proxy") == Some(&Value::String(proxy_url.to_string()))
                && settings.get("http.proxySupport") == Some(&Value::String("on".to_string()))
                && settings.get("http.experimental.systemCertificatesV2")
                    == Some(&Value::Bool(true))
                && settings.get("cursor.general.disableHttp2") == Some(&Value::Bool(true)),
        )
    }

    fn persist_backup(&self, backup: &SettingsBackup) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(backup)
            .map_err(|error| CursorError::Config(error.to_string()))?;
        crate::config::atomic_write_private(&self.backup_path, &bytes)
            .map_err(|error| CursorError::Config(error.to_string()))
    }

    fn load_backup(&self) -> Result<SettingsBackup> {
        let bytes = fs::read(&self.backup_path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CursorError::Config("没有可恢复的 Cursor 设置备份".to_string())
            } else {
                CursorError::Io(error)
            }
        })?;
        serde_json::from_slice(&bytes).map_err(|error| CursorError::Config(error.to_string()))
    }

    fn restore_raw(&self, backup: &SettingsBackup) -> Result<()> {
        if backup.had_file {
            crate::config::atomic_write(&self.path, &backup.original)
                .map_err(|error| CursorError::Config(error.to_string()))?;
        } else if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

fn validate_proxy_url(proxy_url: &str) -> Result<()> {
    let parsed = url::Url::parse(proxy_url.trim())
        .map_err(|error| CursorError::InvalidProxyUrl(error.to_string()))?;
    if parsed.scheme() != "http" || parsed.host_str().is_none() || parsed.port().is_none() {
        return Err(CursorError::InvalidProxyUrl(
            "必须是带主机和端口的 http URL".to_string(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[allow(dead_code)]
pub fn managed_keys() -> &'static [&'static str] {
    &PROXY_KEYS
}
