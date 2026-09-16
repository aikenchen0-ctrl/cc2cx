//! Cursor-local certificate authority material.
//!
//! The manager owns only the application CA files. Trust-store installation is explicit at the
//! command boundary and is never performed as a side effect of proxy startup.

use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, RsaKeySize, PKCS_RSA_SHA256,
};
use serde::Serialize;
use sha1::{Digest as Sha1Digest, Sha1};
use time::{Duration, OffsetDateTime};
use x509_parser::prelude::FromDer;

use super::error::{CursorError, Result};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(test, not(target_os = "macos")))]
#[allow(dead_code)]
#[path = "ca/macos.rs"]
mod macos_host_compile;
#[cfg(target_os = "windows")]
mod windows;

const CA_COMMON_NAME: &str = "CC2CX Cursor Local CA";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaState {
    Missing,
    Invalid,
    Untrusted,
    Ready,
}

pub struct LoadedCa {
    pub issuer: Issuer<'static, KeyPair>,
}

pub struct CaManager {
    dir: PathBuf,
}

impl CaManager {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn certificate_path(&self) -> PathBuf {
        self.dir.join("ca.crt")
    }

    pub fn private_key_path(&self) -> PathBuf {
        self.dir.join("ca.key")
    }

    pub fn state(&self) -> Result<CaState> {
        let cert = fs::read_to_string(self.certificate_path());
        let key = fs::read_to_string(self.private_key_path());
        match (cert, key) {
            (Err(cert_error), Err(key_error))
                if cert_error.kind() == std::io::ErrorKind::NotFound
                    && key_error.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(CaState::Missing)
            }
            (Ok(cert), Ok(key)) => {
                if validate_pair(&cert, &key).is_err() {
                    return Ok(CaState::Invalid);
                }

                // Trust installation is intentionally separate from material generation. On
                // platforms with a native probe, Ready means this exact certificate is trusted.
                Ok(if is_installed(&cert, &self.certificate_path())? {
                    CaState::Ready
                } else {
                    CaState::Untrusted
                })
            }
            _ => Ok(CaState::Invalid),
        }
    }

    pub fn initialize(&self) -> Result<()> {
        match self.state()? {
            CaState::Missing => self.generate(),
            CaState::Invalid => Err(CursorError::Config(
                "CA 文件不完整或证书与私钥不匹配".to_string(),
            )),
            CaState::Untrusted | CaState::Ready => Ok(()),
        }
    }

    /// Install this exact, already initialized certificate into the platform trust store when a
    /// native installer is available. Installation is idempotent.
    pub fn install(&self) -> Result<()> {
        match self.state()? {
            CaState::Missing => {
                return Err(CursorError::Config(
                    "请先初始化 Cursor CA 材料，再执行安装".to_string(),
                ))
            }
            CaState::Invalid => {
                return Err(CursorError::Config(
                    "CA 文件不完整或证书与私钥不匹配".to_string(),
                ))
            }
            CaState::Untrusted | CaState::Ready => {}
        }
        let cert = self.certificate_pem()?;
        if is_installed(&cert, &self.certificate_path())? {
            return Ok(());
        }

        #[cfg(target_os = "windows")]
        {
            windows::install(&cert)?;
            if !is_installed(&cert, &self.certificate_path())? {
                return Err(CursorError::Config(
                    "Windows Root 证书库未确认已安装 CC2CX CA；请在证书导入向导中选择“当前用户”，将证书放入“受信任的根证书颁发机构”，完成安全警告后重试".to_string(),
                ));
            }
        }

        #[cfg(target_os = "macos")]
        {
            macos::install(&self.certificate_path())?;
            if !is_installed(&cert, &self.certificate_path())? {
                return Err(CursorError::Config(
                    "macOS 用户钥匙串未确认已信任 CC2CX CA；请解锁登录钥匙串并重试".to_string(),
                ));
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            // macOS/Linux need privileged, platform-specific trust-store workflows. Do not run
            // shell commands implicitly; leave the explicit command available to the UI.
            return Err(CursorError::Config(
                "当前平台不支持自动安装 Cursor CA，请执行显示的安装命令".to_string(),
            ));
        }
        Ok(())
    }

    /// Remove only the exact managed certificate from the current user's trust store.
    pub fn uninstall(&self) -> Result<()> {
        if self.state()? == CaState::Missing {
            return Ok(());
        }
        if self.state()? == CaState::Invalid {
            return Err(CursorError::Config(
                "CA 文件不完整或证书与私钥不匹配".to_string(),
            ));
        }
        let cert = self.certificate_pem()?;
        #[cfg(target_os = "windows")]
        {
            windows::uninstall(&cert)?;
            if is_installed(&cert, &self.certificate_path())? {
                return Err(CursorError::Config(
                    "Windows Root 证书库仍包含 CC2CX CA".to_string(),
                ));
            }
        }

        #[cfg(target_os = "macos")]
        {
            macos::uninstall(&cert, &self.certificate_path())?;
            if is_installed(&cert, &self.certificate_path())? {
                return Err(CursorError::Config(
                    "macOS 用户钥匙串仍包含 CC2CX CA".to_string(),
                ));
            }
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            return Err(CursorError::Config(
                "当前平台不支持自动卸载 Cursor CA，请执行显示的卸载命令".to_string(),
            ));
        }
        Ok(())
    }

    pub fn load(&self) -> Result<LoadedCa> {
        let cert = self.certificate_pem()?;
        let key = self.private_key_pem()?;
        Ok(LoadedCa {
            issuer: validate_pair(&cert, &key)?,
        })
    }

    pub fn certificate_pem(&self) -> Result<String> {
        fs::read_to_string(self.certificate_path()).map_err(CursorError::Io)
    }

    pub fn private_key_pem(&self) -> Result<String> {
        fs::read_to_string(self.private_key_path()).map_err(CursorError::Io)
    }

    /// Returns the exact command a UI may show for explicit trust installation.
    /// The command is not executed by this module.
    pub fn install_command(&self) -> Option<String> {
        self.install_command_for(std::env::consts::OS)
    }

    fn install_command_for(&self, os: &str) -> Option<String> {
        let cert = self.certificate_path();
        match os {
            "windows" => Some(format!(
                "certutil -user -addstore -f Root \"{}\"",
                cert.display()
            )),
            "macos" => Some(format!(
                "/usr/bin/security add-trusted-cert -r trustRoot -p ssl -k \"$HOME/Library/Keychains/login.keychain-db\" {}",
                shell_quote(&cert)
            )),
            "linux" => Some(format!(
                "sudo cp '{}' /usr/local/share/ca-certificates/cc2cx-cursor-local-ca.crt && sudo update-ca-certificates",
                cert.display().to_string().replace('\'', "'\\''")
            )),
            _ => None,
        }
    }

    /// Returns a platform command for removing only this managed certificate.
    pub fn uninstall_command(&self) -> Option<String> {
        self.uninstall_command_for(std::env::consts::OS)
    }

    fn uninstall_command_for(&self, os: &str) -> Option<String> {
        match os {
            "windows" => self.windows_uninstall_command(),
            "macos" => self.macos_uninstall_command(),
            "linux" => Some(
                "sudo rm -f /usr/local/share/ca-certificates/cc2cx-cursor-local-ca.crt && sudo update-ca-certificates"
                    .to_string(),
            ),
            _ => None,
        }
    }

    #[cfg(target_os = "windows")]
    fn windows_uninstall_command(&self) -> Option<String> {
        let cert = fs::read(self.certificate_path()).ok()?;
        let der = pem::parse(cert).ok()?.into_contents();
        let digest = Sha1::digest(der);
        let thumbprint = digest
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        Some(format!("certutil -user -delstore Root \"{thumbprint}\""))
    }

    #[cfg(not(target_os = "windows"))]
    fn windows_uninstall_command(&self) -> Option<String> {
        None
    }

    #[cfg(target_os = "macos")]
    fn macos_uninstall_command(&self) -> Option<String> {
        macos_uninstall_command_for_path(&self.certificate_path())
    }

    #[cfg(not(target_os = "macos"))]
    fn macos_uninstall_command(&self) -> Option<String> {
        macos_uninstall_command_for_path(&self.certificate_path())
    }

    fn generate(&self) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        #[cfg(unix)]
        fs::set_permissions(&self.dir, fs::Permissions::from_mode(0o700))?;

        let key = KeyPair::generate_rsa_for(&PKCS_RSA_SHA256, RsaKeySize::_3072)
            .map_err(|error| CursorError::Config(format!("生成 CA 私钥失败: {error}")))?;
        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|error| CursorError::Config(format!("创建 CA 参数失败: {error}")))?;
        let now = OffsetDateTime::now_utc();
        params.not_before = now - Duration::days(1);
        params.not_after = now + Duration::days(3_650);
        let mut name = DistinguishedName::new();
        name.push(DnType::CommonName, CA_COMMON_NAME);
        name.push(DnType::OrganizationName, "CC2CX");
        params.distinguished_name = name;
        params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];
        let cert = params
            .self_signed(&key)
            .map_err(|error| CursorError::Config(format!("生成 CA 证书失败: {error}")))?;

        let key_path = self.private_key_path();
        write_private(&key_path, key.serialize_pem().as_bytes())?;
        if let Err(error) =
            crate::config::atomic_write(&self.certificate_path(), cert.pem().as_bytes())
        {
            // Never leave a private key without its matching certificate. A later initialization
            // can safely regenerate the complete pair after this failure.
            let _ = fs::remove_file(key_path);
            return Err(CursorError::Config(error.to_string()));
        }
        Ok(())
    }
}

fn validate_pair(cert: &str, key: &str) -> Result<Issuer<'static, KeyPair>> {
    let key_pair = KeyPair::from_pem(key)
        .map_err(|error| CursorError::Config(format!("解析 CA 私钥失败: {error}")))?;
    let pem = pem::parse(cert)
        .map_err(|error| CursorError::Config(format!("解析 CA PEM 失败: {error}")))?;
    let (_, parsed) = x509_parser::certificate::X509Certificate::from_der(pem.contents())
        .map_err(|error| CursorError::Config(format!("解析 CA X.509 失败: {error}")))?;
    if parsed.public_key().subject_public_key.data.as_ref() != key_pair.public_key_raw() {
        return Err(CursorError::Config("CA 证书与私钥不匹配".to_string()));
    }
    if !parsed.validity().is_valid() {
        return Err(CursorError::Config("CA 证书已过期或尚未生效".to_string()));
    }
    if !parsed
        .basic_constraints()
        .map_err(|error| CursorError::Config(format!("读取 CA 约束失败: {error}")))?
        .is_some_and(|constraints| constraints.value.ca)
    {
        return Err(CursorError::Config("证书不是 CA".to_string()));
    }
    Issuer::from_ca_cert_pem(cert, key_pair)
        .map_err(|error| CursorError::Config(format!("加载 CA 失败: {error}")))
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    crate::config::atomic_write_private(path, bytes)
        .map_err(|error| CursorError::Config(error.to_string()))
}

#[cfg(target_os = "windows")]
fn is_installed(cert: &str, _cert_path: &Path) -> Result<bool> {
    windows::is_installed(cert)
}

#[cfg(target_os = "macos")]
fn is_installed(cert: &str, cert_path: &Path) -> Result<bool> {
    macos::is_installed(cert, cert_path)
}

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
fn is_installed(_cert: &str, _cert_path: &Path) -> Result<bool> {
    // Linux needs its own trust-store probe. Keep it explicitly untrusted rather than guessing
    // from files in a distribution-specific location.
    Ok(false)
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

fn macos_uninstall_command_for_path(path: &Path) -> Option<String> {
    let cert = fs::read(path).ok()?;
    let der = pem::parse(cert).ok()?.into_contents();
    // `security delete-certificate -Z` accepts SHA-1 on macOS 12 and older releases. The
    // destructive operation is still guarded by an exact DER probe in the native adapter.
    let digest = Sha1::digest(der);
    let fingerprint = digest
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<String>();
    Some(format!(
        "/usr/bin/security delete-certificate -Z {fingerprint} -t \"$HOME/Library/Keychains/login.keychain-db\""
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn macos_install_command_uses_current_user_login_keychain() {
        let manager = CaManager::new(PathBuf::from("/tmp/cc2cx-cursor-ca"));
        let command = manager
            .install_command_for("macos")
            .expect("macOS install command");
        assert!(command.starts_with("/usr/bin/security add-trusted-cert"));
        assert!(command.contains("-r trustRoot -p ssl"));
        assert!(command.contains("$HOME/Library/Keychains/login.keychain-db"));
        assert!(!command.contains("sudo"));
        assert!(!command.contains(" -d "));
        assert!(!command.contains("System.keychain"));
    }

    #[test]
    fn macos_uninstall_command_targets_exact_sha1_in_login_keychain() {
        let dir = tempdir().expect("temporary CA directory");
        let manager = CaManager::new(dir.path().to_path_buf());
        manager.initialize().expect("generate CA");
        let cert = pem::parse(manager.certificate_pem().expect("certificate")).expect("PEM");
        let expected = Sha1::digest(cert.contents());
        let expected = expected
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        let command = manager
            .uninstall_command_for("macos")
            .expect("macOS uninstall command");
        assert!(command.contains(&format!("-Z {expected}")));
        assert!(command.contains(" -t "));
        assert!(command.contains("$HOME/Library/Keychains/login.keychain-db"));
        assert!(!command.contains("sudo"));
        assert!(!command.contains("remove-trusted-cert"));
    }
}
