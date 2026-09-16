//! Coordinates the reversible Cursor integration transaction.

use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::Serialize;
use tokio::sync::Mutex;

use super::{
    ca::{CaManager, CaState},
    error::Result,
    fake_ip::FakeIpMap,
    hosts::{self, HostsTransaction},
    mitm::{CursorProxyConfig, CursorProxyRuntime},
    profile::CursorProfile,
    protocol_backend::{CursorBackendHealth, CursorProtocolBackend, CursorProtocolBackendRuntime},
    settings::CursorSettingsStore,
    transparent::{self, TransparentRuntime},
};
use crate::{app_config::AppType, database::Database};

const TRANSPARENT_HOSTS: &[&str] = &[
    "api2.cursor.sh",
    "api3.cursor.sh",
    "agentn.global.api5.cursor.sh",
    "api2geo.cursor.sh",
    "api2direct.cursor.sh",
    "agentn.global.api5geo.cursor.sh",
    "agentn.global.api5lat.cursor.sh",
    "agentn.api5.cursor.sh",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorEntryMode {
    Proxy,
    Transparent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorIntegrationState {
    Disabled,
    Starting,
    Running,
    Degraded,
    Stopping,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FakeIpEntry {
    pub ip: String,
    pub hostname: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparentEntryStatus {
    pub running: bool,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorHarnessStatus {
    pub state: CursorIntegrationState,
    pub ca: CaState,
    pub ca_install_command: Option<String>,
    pub ca_uninstall_command: Option<String>,
    pub settings_applied: bool,
    pub proxy_url: Option<String>,
    pub proxy_port: Option<u16>,
    pub backend_port: Option<u16>,
    pub backend_health: Option<CursorBackendHealth>,
    pub backend_error_code: Option<String>,
    pub settings_backup_present: bool,
    pub transparent_entry: Option<TransparentEntryStatus>,
    pub managed_hosts: bool,
    pub fake_ip_entries: Vec<FakeIpEntry>,
}

struct Inner {
    ca: CaManager,
    ca_lock: Mutex<()>,
    profile: Option<CursorProfile>,
    settings: CursorSettingsStore,
    proxy: Mutex<CursorProxyRuntime>,
    backend: Mutex<Option<CursorProtocolBackendRuntime>>,
    backend_override: Mutex<Option<Option<SocketAddr>>>,
    database: Option<Arc<Database>>,
    provider_app_type: Option<AppType>,
    proxy_config: CursorProxyConfig,
    hosts_path: std::sync::Mutex<Option<PathBuf>>,
    entry_mode: std::sync::Mutex<CursorEntryMode>,
    transparent_listen_port: std::sync::Mutex<u16>,
    transparent: Mutex<Option<TransparentRuntime>>,
    hosts_tx: Mutex<Option<HostsTransaction>>,
    fake_ips: Mutex<Option<FakeIpMap>>,
}

#[derive(Clone)]
pub struct CursorHarness {
    inner: Arc<Inner>,
}

impl CursorHarness {
    fn from_parts(
        ca_dir: PathBuf,
        profile: Option<CursorProfile>,
        settings_path: PathBuf,
        backup_path: PathBuf,
        proxy_config: CursorProxyConfig,
        database: Option<Arc<Database>>,
        provider_app_type: Option<AppType>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                ca: CaManager::new(ca_dir),
                ca_lock: Mutex::new(()),
                profile,
                settings: CursorSettingsStore::new(settings_path, backup_path),
                proxy: Mutex::new(CursorProxyRuntime::default()),
                backend: Mutex::new(None),
                backend_override: Mutex::new(None),
                database,
                provider_app_type,
                proxy_config,
                hosts_path: std::sync::Mutex::new(None),
                entry_mode: std::sync::Mutex::new(CursorEntryMode::Proxy),
                transparent_listen_port: std::sync::Mutex::new(0),
                transparent: Mutex::new(None),
                hosts_tx: Mutex::new(None),
                fake_ips: Mutex::new(None),
            }),
        }
    }

    pub fn new(
        ca_dir: PathBuf,
        settings_path: PathBuf,
        backup_path: PathBuf,
        proxy_config: CursorProxyConfig,
    ) -> Self {
        Self::from_parts(
            ca_dir,
            None,
            settings_path,
            backup_path,
            proxy_config,
            None,
            None,
        )
    }

    /// Construct a Harness bound to an isolated Cursor user-data directory.
    ///
    /// The profile owns the settings path used by the Harness. This keeps a test or explicit
    /// validation launch from modifying the user's default Cursor profile.
    pub fn new_for_profile(
        ca_dir: PathBuf,
        profile: CursorProfile,
        backup_path: PathBuf,
        proxy_config: CursorProxyConfig,
    ) -> Self {
        Self::from_parts(
            ca_dir,
            Some(profile.clone()),
            profile.settings_path(),
            backup_path,
            proxy_config,
            None,
            None,
        )
    }

    /// Construct a harness whose backend is bound to the selected cc2cx provider.
    ///
    /// The provider is resolved once when the integration starts. Changing the selected
    /// provider while a session is running therefore requires a stop/start, which keeps
    /// credentials and connection state pinned to one session.
    pub fn new_with_database(
        ca_dir: PathBuf,
        settings_path: PathBuf,
        backup_path: PathBuf,
        proxy_config: CursorProxyConfig,
        database: Arc<Database>,
    ) -> Self {
        Self::new_with_database_for_app(
            ca_dir,
            settings_path,
            backup_path,
            proxy_config,
            database,
            AppType::Claude,
        )
    }

    pub fn new_with_database_for_app(
        ca_dir: PathBuf,
        settings_path: PathBuf,
        backup_path: PathBuf,
        proxy_config: CursorProxyConfig,
        database: Arc<Database>,
        provider_app_type: AppType,
    ) -> Self {
        Self::from_parts(
            ca_dir,
            None,
            settings_path,
            backup_path,
            proxy_config,
            Some(database),
            Some(provider_app_type),
        )
    }

    /// Construct an isolated-profile Harness backed by the selected cc2cx provider.
    pub fn new_for_profile_with_database(
        ca_dir: PathBuf,
        profile: CursorProfile,
        backup_path: PathBuf,
        proxy_config: CursorProxyConfig,
        database: Arc<Database>,
        provider_app_type: AppType,
    ) -> Self {
        Self::from_parts(
            ca_dir,
            Some(profile.clone()),
            profile.settings_path(),
            backup_path,
            proxy_config,
            Some(database),
            Some(provider_app_type),
        )
    }

    pub async fn status(&self) -> Result<CursorHarnessStatus> {
        let ca = match self.inner.ca.state() {
            Ok(state) => state,
            Err(super::error::CursorError::Io(error))
                if error.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                CaState::Untrusted
            }
            Err(error) => return Err(error),
        };
        let proxy = self.inner.proxy.lock().await;
        if !proxy.running() && self.inner.settings.has_backup() {
            // A proxy task can terminate after start() has returned. Reconcile the settings
            // transaction here so status polling cannot leave Cursor pointed at a dead port.
            if let Err(error) = self.inner.settings.restore() {
                match error {
                    super::error::CursorError::Io(io_error)
                        if io_error.kind() == std::io::ErrorKind::PermissionDenied => {}
                    other => return Err(other),
                }
            }
        }
        let proxy_url = proxy.url();
        let settings_applied = proxy_url
            .as_deref()
            .map(|url| self.inner.settings.matches_proxy(url))
            .transpose()?
            .unwrap_or(false);
        let backend = self.inner.backend.lock().await;
        let backend_port = backend
            .as_ref()
            .filter(|runtime| runtime.running())
            .map(|runtime| runtime.address().port());
        let backend_health = backend.as_ref().map(|runtime| runtime.health());
        let backend_running = backend.as_ref().is_some_and(|runtime| runtime.running());
        drop(backend);
        let transparent_running = self.inner.transparent.lock().await.is_some();
        let entry_up = proxy.running() || transparent_running;
        let configured = settings_applied || transparent_running;
        let base_state = match (entry_up, configured, ca) {
            (false, _, _) => CursorIntegrationState::Disabled,
            (true, true, CaState::Untrusted) => CursorIntegrationState::Degraded,
            (true, true, CaState::Ready) => CursorIntegrationState::Running,
            _ => CursorIntegrationState::Degraded,
        };
        let state = if backend_health.is_some() && !backend_running
            || backend_health
                .as_ref()
                .is_some_and(|(health, _)| health.is_failure())
        {
            CursorIntegrationState::Degraded
        } else {
            base_state
        };
        let backend_error_code = backend_health.as_ref().and_then(|(_, error)| error.clone());
        let proxy_port = proxy.port();
        drop(proxy);
        let transparent = self.inner.transparent.lock().await;
        let fake_ips = self.inner.fake_ips.lock().await;
        let hosts_tx = self.inner.hosts_tx.lock().await;
        let fake_ip_entries: Vec<FakeIpEntry> = fake_ips
            .as_ref()
            .map(|map| {
                map.entries()
                    .into_iter()
                    .map(|(ip, hostname)| FakeIpEntry {
                        ip: ip.to_string(),
                        hostname,
                    })
                    .collect()
            })
            .unwrap_or_default();
        let transparent_entry = transparent.as_ref().map(|runtime| TransparentEntryStatus {
            running: true,
            addresses: fake_ip_entries
                .iter()
                .map(|entry| {
                    format!(
                        "{}:{}",
                        entry.ip,
                        runtime.address_for(&entry.hostname).port()
                    )
                })
                .collect(),
        });
        Ok(CursorHarnessStatus {
            state,
            ca,
            ca_install_command: self.inner.ca.install_command(),
            ca_uninstall_command: self.inner.ca.uninstall_command(),
            settings_applied,
            proxy_port,
            backend_port,
            backend_health: backend_health.map(|(health, _)| health),
            backend_error_code,
            proxy_url,
            settings_backup_present: self.inner.settings.has_backup(),
            transparent_entry,
            managed_hosts: hosts_tx.is_some(),
            fake_ip_entries,
        })
    }

    pub fn enable_transparent_entry(&self, hosts_path: PathBuf, listen_port: u16) {
        *self.inner.hosts_path.lock().expect("hosts path lock") = Some(hosts_path);
        *self.inner.entry_mode.lock().expect("entry mode lock") = CursorEntryMode::Transparent;
        *self
            .inner
            .transparent_listen_port
            .lock()
            .expect("transparent port lock") = listen_port;
    }

    pub fn entry_mode(&self) -> CursorEntryMode {
        *self.inner.entry_mode.lock().expect("entry mode lock")
    }

    pub fn transparent_hosts_path(&self) -> Option<PathBuf> {
        self.inner
            .hosts_path
            .lock()
            .expect("hosts path lock")
            .clone()
    }

    pub fn transparent_listen_port(&self) -> u16 {
        *self
            .inner
            .transparent_listen_port
            .lock()
            .expect("transparent port lock")
    }

    async fn rollback_transparent_entry(&self) {
        if let Some(mut runtime) = self.inner.transparent.lock().await.take() {
            runtime.stop().await;
        }
        if let Some(transaction) = self.inner.hosts_tx.lock().await.take() {
            let _ = hosts::restore(&transaction);
        }
        *self.inner.fake_ips.lock().await = None;
    }

    async fn start_transparent_entry(&self) -> Result<()> {
        if self.inner.transparent.lock().await.is_some() {
            return Ok(());
        }
        let hosts_path = self
            .inner
            .hosts_path
            .lock()
            .expect("hosts path lock")
            .clone();
        let Some(hosts_path) = hosts_path else {
            return Ok(());
        };
        let listen_port = *self
            .inner
            .transparent_listen_port
            .lock()
            .expect("transparent port lock");
        validate_transparent_entry(&hosts_path, listen_port)?;
        let hosts: Vec<String> = TRANSPARENT_HOSTS
            .iter()
            .map(|host| (*host).to_string())
            .collect();
        let map = FakeIpMap::allocate(&hosts)?;
        let ca = self.inner.ca.load()?;
        let entries = map.entries();
        let enable_entries: Vec<(IpAddr, &str)> = entries
            .iter()
            .map(|(ip, hostname)| (IpAddr::V4(*ip), hostname.as_str()))
            .collect();
        let runtime = transparent::start_on_port(
            ca,
            Arc::new(map.clone()),
            SocketAddr::from(([127, 0, 0, 1], 1)),
            listen_port,
        )
        .await?;
        let transaction = match hosts::enable(&hosts_path, &enable_entries) {
            Ok(transaction) => transaction,
            Err(error) => {
                let mut runtime = runtime;
                runtime.stop().await;
                return Err(error);
            }
        };
        *self.inner.fake_ips.lock().await = Some(map);
        *self.inner.hosts_tx.lock().await = Some(transaction);
        *self.inner.transparent.lock().await = Some(runtime);
        Ok(())
    }

    pub async fn start(&self) -> Result<CursorHarnessStatus> {
        let entry_mode = self.entry_mode();
        if entry_mode == CursorEntryMode::Transparent {
            // Validate the platform boundary before touching Cursor settings. In particular,
            // macOS cannot modify /etc/hosts or bind a privileged port without a helper; a
            // failed validation must leave the user's settings byte-for-byte unchanged.
            let hosts_path = self
                .inner
                .hosts_path
                .lock()
                .expect("hosts path lock")
                .clone();
            let listen_port = *self
                .inner
                .transparent_listen_port
                .lock()
                .expect("transparent port lock");
            if let Some(hosts_path) = hosts_path {
                validate_transparent_entry(&hosts_path, listen_port)?;
            }
        }

        let ca_guard = self.inner.ca_lock.lock().await;
        self.inner.ca.initialize()?;
        drop(ca_guard);

        if entry_mode == CursorEntryMode::Transparent {
            if self.inner.settings.has_backup() {
                if let Err(error) = self.inner.settings.restore() {
                    match error {
                        super::error::CursorError::Io(io_error)
                            if io_error.kind() == std::io::ErrorKind::PermissionDenied => {}
                        other => return Err(other),
                    }
                }
            }
            self.inner.settings.clear_proxy_keys()?;
            if let Err(error) = self.start_transparent_entry().await {
                self.rollback_transparent_entry().await;
                return Err(error);
            }
        }

        let mut proxy = self.inner.proxy.lock().await;
        if !proxy.running() {
            if entry_mode == CursorEntryMode::Proxy && self.inner.settings.has_backup() {
                self.inner.settings.restore()?;
            }
            let ca = self.inner.ca.load()?;
            let mut backend = self.inner.backend.lock().await;
            let configured_backend = self.inner.backend_override.lock().await;
            let requested_backend = configured_backend.as_ref().copied().unwrap_or_else(|| {
                self.inner
                    .database
                    .as_ref()
                    .map(|_| {
                        self.inner
                            .proxy_config
                            .backend
                            .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0)))
                    })
                    .or(self.inner.proxy_config.backend)
            });
            // A proxy task may have exited without going through `stop()`. Never reuse the
            // previous backend in that case: its provider/session belongs to the dead run.
            if backend.is_some() {
                if let Some(mut stale_backend) = backend.take() {
                    let _ = stale_backend.stop().await;
                }
            }
            let backend_address = if let Some(requested) = requested_backend {
                let backend_builder = match (&self.inner.database, &self.inner.provider_app_type) {
                    (Some(database), Some(app_type)) => {
                        CursorProtocolBackend::with_current_provider(database, app_type)?
                    }
                    _ => CursorProtocolBackend::new(),
                };
                if let Err(error) = backend_builder.preflight().await {
                    drop(backend);
                    drop(configured_backend);
                    drop(proxy);
                    self.rollback_transparent_entry().await;
                    return Err(super::error::CursorError::Config(format!(
                        "Cursor 启动前探测失败 [{}]: {}",
                        error.code, error.message
                    )));
                }
                *backend = Some(
                    backend_builder
                        .start(requested)
                        .await
                        .map_err(super::error::CursorError::Config)?,
                );
                let address = backend.as_ref().map(CursorProtocolBackendRuntime::address);
                if let Some(address) = address {
                    if let Some(runtime) = self.inner.transparent.lock().await.as_ref() {
                        runtime.set_backend(address);
                    }
                }
                address
            } else {
                None
            };
            if entry_mode == CursorEntryMode::Proxy {
                let mut proxy_config = self.inner.proxy_config;
                proxy_config.backend = backend_address;
                let (url, _) = match proxy.start(ca, proxy_config).await {
                    Ok(result) => result,
                    Err(error) => {
                        if let Some(mut runtime) = backend.take() {
                            let _ = runtime.stop().await;
                        }
                        drop(backend);
                        drop(configured_backend);
                        drop(proxy);
                        return Err(super::error::CursorError::Config(error));
                    }
                };
                if let Err(error) = self.inner.settings.apply_proxy(&url) {
                    proxy.stop().await;
                    if let Some(mut runtime) = backend.take() {
                        let _ = runtime.stop().await;
                    }
                    drop(backend);
                    drop(configured_backend);
                    drop(proxy);
                    return Err(error);
                }
            }
            drop(backend);
            drop(configured_backend);
        }
        drop(proxy);
        self.status().await
    }

    pub async fn initialize_ca(&self) -> Result<CursorHarnessStatus> {
        let ca_guard = self.inner.ca_lock.lock().await;
        self.inner.ca.initialize()?;
        drop(ca_guard);
        self.status().await
    }

    pub async fn install_ca(&self) -> Result<CursorHarnessStatus> {
        let ca_guard = self.inner.ca_lock.lock().await;
        self.inner.ca.initialize()?;
        self.inner.ca.install()?;
        drop(ca_guard);
        self.status().await
    }

    pub async fn uninstall_ca(&self) -> Result<CursorHarnessStatus> {
        let proxy = self.inner.proxy.lock().await;
        if proxy.running() {
            return Err(super::error::CursorError::Config(
                "请先停止 Cursor 接入，再卸载 CA".to_string(),
            ));
        }
        drop(proxy);
        let ca_guard = self.inner.ca_lock.lock().await;
        self.inner.ca.uninstall()?;
        drop(ca_guard);
        self.status().await
    }

    /// Recover a settings transaction left by a crashed or externally terminated process.
    /// Running integrations are never touched; the proxy lock serializes this with `start`.
    pub async fn recover_stale_settings(&self) -> Result<()> {
        let proxy = self.inner.proxy.lock().await;
        if proxy.running() {
            return Ok(());
        }
        if self.inner.settings.has_backup() {
            self.inner.settings.restore()?;
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<CursorHarnessStatus> {
        self.rollback_transparent_entry().await;
        let mut proxy = self.inner.proxy.lock().await;
        proxy.stop().await;
        let mut backend = self.inner.backend.lock().await;
        if let Some(mut runtime) = backend.take() {
            let _ = runtime.stop().await;
        }
        if self.inner.settings.has_backup() {
            if let Err(error) = self.inner.settings.restore() {
                match error {
                    super::error::CursorError::Io(io_error)
                        if io_error.kind() == std::io::ErrorKind::PermissionDenied => {}
                    other => return Err(other),
                }
            }
        }
        drop(backend);
        drop(proxy);
        self.status().await
    }

    pub fn ca_manager(&self) -> &CaManager {
        &self.inner.ca
    }

    /// Return the explicitly bound Cursor profile, if this Harness was constructed for one.
    ///
    /// `None` identifies the normal user-settings constructor and prevents a caller from
    /// accidentally launching an isolated Cursor process against the default profile.
    pub fn profile(&self) -> Option<CursorProfile> {
        self.inner.profile.clone()
    }

    pub async fn configure_backend(
        &self,
        address: Option<SocketAddr>,
    ) -> Result<CursorHarnessStatus> {
        if self.inner.proxy.lock().await.running() {
            return Err(super::error::CursorError::Config(
                "请先停止 Cursor 接入，再修改 backend 配置".to_string(),
            ));
        }
        *self.inner.backend_override.lock().await = Some(address);
        self.status().await
    }
}

fn validate_transparent_entry_for_os(os: &str, hosts_path: &Path, listen_port: u16) -> Result<()> {
    if os != "macos" {
        return Ok(());
    }

    // `/etc` is a symlink to `/private/etc` on macOS. Check both lexical spellings and the
    // canonical target so a symlink alias cannot bypass the no-helper boundary.
    let system_hosts = [
        Some(hosts_path.to_path_buf()),
        hosts_path.canonicalize().ok(),
        Some(hosts::system_path()),
    ]
    .into_iter()
    .flatten()
    .any(|path| path == Path::new("/etc/hosts") || path == Path::new("/private/etc/hosts"));
    let privileged_port = listen_port != 0 && listen_port < 1024;
    if system_hosts || privileged_port {
        return Err(super::error::CursorError::Config(
            "macOS 透明入口需要特权网络 helper；当前版本请使用本机 HTTP 代理入口".to_string(),
        ));
    }
    Ok(())
}

fn validate_transparent_entry(hosts_path: &Path, listen_port: u16) -> Result<()> {
    validate_transparent_entry_for_os(std::env::consts::OS, hosts_path, listen_port)
}

impl CursorHarness {
    pub fn default_for_current_user() -> Self {
        let app_dir = crate::config::get_app_config_dir();
        let settings_path = CursorSettingsStore::default_path().unwrap_or_else(|_| {
            crate::config::get_home_dir().join(".config/Cursor/User/settings.json")
        });
        Self::new(
            app_dir.join("cursor-ca"),
            settings_path,
            app_dir.join("cursor-settings-backup.json"),
            CursorProxyConfig::default(),
        )
    }

    pub fn default_for_current_user_with_database(database: Arc<Database>) -> Self {
        let app_dir = crate::config::get_app_config_dir();
        let settings_path = CursorSettingsStore::default_path().unwrap_or_else(|_| {
            crate::config::get_home_dir().join(".config/Cursor/User/settings.json")
        });
        Self::new_with_database(
            app_dir.join("cursor-ca"),
            settings_path,
            app_dir.join("cursor-settings-backup.json"),
            CursorProxyConfig::default(),
            database,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::profile::CursorProfile;
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn macos_transparent_entry_rejects_system_hosts_and_all_privileged_ports() {
        assert!(validate_transparent_entry_for_os("macos", Path::new("/etc/hosts"), 443).is_err());
        assert!(
            validate_transparent_entry_for_os("macos", Path::new("/private/etc/hosts"), 1023,)
                .is_err()
        );
        assert!(validate_transparent_entry_for_os("macos", Path::new("/tmp/hosts"), 1024).is_ok());
        assert!(validate_transparent_entry_for_os("macos", Path::new("/tmp/hosts"), 0).is_ok());
        assert!(validate_transparent_entry_for_os("windows", Path::new("/etc/hosts"), 443).is_ok());
    }

    #[test]
    fn profile_constructor_uses_the_profile_settings_file() {
        let dir = tempdir().expect("temporary harness directory");
        let profile = CursorProfile::new(dir.path().join("cursor-profile"));
        let harness = CursorHarness::new_for_profile(
            dir.path().join("cursor-ca"),
            profile.clone(),
            dir.path().join("cursor-settings-backup.json"),
            CursorProxyConfig::default(),
        );

        assert_eq!(harness.profile(), Some(profile));
    }

    #[tokio::test]
    async fn start_discards_a_stopped_backend_runtime_instead_of_reusing_it() {
        let dir = tempdir().expect("temporary harness directory");
        let harness = CursorHarness::new(
            dir.path().join("cursor-ca"),
            dir.path().join("Cursor/User/settings.json"),
            dir.path().join("cursor-settings-backup.json"),
            CursorProxyConfig {
                requested_port: 0,
                ..CursorProxyConfig::default()
            },
        );

        let backend = CursorProtocolBackend::new();
        let mut stopped = backend
            .start(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("backend should bind");
        stopped.stop().await.expect("backend should stop");
        *harness.inner.backend.lock().await = Some(stopped);

        let status = harness
            .start()
            .await
            .expect("harness should start after stale backend cleanup");
        assert!(status.settings_applied);
        assert_eq!(status.backend_port, None);

        harness.stop().await.expect("harness should stop");
    }

    #[tokio::test]
    async fn disabled_status_exposes_no_backend_health_snapshot() {
        let dir = tempdir().expect("temporary harness directory");
        let harness = CursorHarness::new(
            dir.path().join("cursor-ca"),
            dir.path().join("Cursor/User/settings.json"),
            dir.path().join("cursor-settings-backup.json"),
            CursorProxyConfig::default(),
        );

        let status = harness.status().await.expect("harness status");
        assert_eq!(status.backend_health, None);
        assert_eq!(status.backend_error_code, None);
    }
}
