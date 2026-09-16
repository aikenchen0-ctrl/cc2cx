//! Isolated Cursor profile paths used by local integration tests and launchers.

use std::{
    io,
    path::{Path, PathBuf},
    process::{Child, Command},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorProfile {
    root: PathBuf,
}

pub fn launch_args(profile: &CursorProfile) -> Vec<String> {
    vec![
        "--user-data-dir".to_string(),
        profile.root.to_string_lossy().into_owned(),
    ]
}

pub fn launch_args_transparent(profile: &CursorProfile) -> Vec<String> {
    let mut args = launch_args(profile);
    args.push("--disable-quic".to_string());
    args.push(
        "--host-resolver-rules=MAP api2.cursor.sh 127.0.0.2, MAP api3.cursor.sh 127.0.0.3, MAP agentn.global.api5.cursor.sh 127.0.0.4, MAP agentn.api5.cursor.sh 127.0.0.4, MAP api2geo.cursor.sh 127.0.0.5, MAP api2direct.cursor.sh 127.0.0.6, MAP agentn.global.api5geo.cursor.sh 127.0.0.7, MAP agentn.global.api5lat.cursor.sh 127.0.0.8"
            .to_string(),
    );
    args
}

pub fn launch_args_with_proxy(profile: &CursorProfile, proxy_url: &str) -> Vec<String> {
    let mut args = launch_args(profile);
    args.push(format!("--proxy-server={proxy_url}"));
    args.push("--disable-quic".to_string());
    args
}

/// Locate a normal per-machine or per-user Cursor installation.
///
/// The caller may still pass an explicit executable path; this helper only covers common
/// installation locations and never edits the installation directory.
pub fn discover_executable() -> Option<PathBuf> {
    executable_candidates()
        .into_iter()
        .find(|candidate| candidate.is_file())
}

#[cfg(target_os = "windows")]
fn executable_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(program_files) = std::env::var_os("PROGRAMFILES") {
        candidates.push(PathBuf::from(program_files).join("cursor/Cursor.exe"));
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(PathBuf::from(local_app_data).join("Programs/cursor/Cursor.exe"));
    }
    candidates
}

#[cfg(target_os = "macos")]
fn executable_candidates() -> Vec<PathBuf> {
    let home = crate::config::get_home_dir();
    vec![
        home.join("Applications/Cursor.app/Contents/MacOS/Cursor"),
        PathBuf::from("/Applications/Cursor.app/Contents/MacOS/Cursor"),
        home.join(".local/bin/cursor"),
        PathBuf::from("/opt/homebrew/bin/cursor"),
        PathBuf::from("/usr/local/bin/cursor"),
        PathBuf::from("/usr/bin/cursor"),
    ]
}

#[cfg(target_os = "linux")]
fn executable_candidates() -> Vec<PathBuf> {
    let home = crate::config::get_home_dir();
    vec![
        home.join(".local/bin/cursor"),
        PathBuf::from("/usr/local/bin/cursor"),
        PathBuf::from("/usr/bin/cursor"),
    ]
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn executable_candidates() -> Vec<PathBuf> {
    Vec::new()
}

pub struct CursorProcess {
    child: Child,
}

impl CursorProcess {
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn stop(mut self) -> io::Result<()> {
        self.child.kill()?;
        let _ = self.child.wait()?;
        Ok(())
    }
}

pub fn launch(executable: &Path, profile: &CursorProfile) -> io::Result<CursorProcess> {
    std::fs::create_dir_all(profile.root())?;
    let child = Command::new(executable)
        .args(launch_args(profile))
        .spawn()?;
    Ok(CursorProcess { child })
}

/// Environment for Mode T. NodeService does not always use the Windows trust store unless
/// asked; live logs requested `--use-system-ca`. This must not set HTTP(S)_PROXY.
pub fn launch_env_transparent(ca_cert: &Path) -> Vec<(String, String)> {
    vec![
        ("NODE_OPTIONS".to_string(), "--use-system-ca".to_string()),
        (
            "NODE_EXTRA_CA_CERTS".to_string(),
            ca_cert.to_string_lossy().into_owned(),
        ),
    ]
}

pub fn launch_transparent(
    executable: &Path,
    profile: &CursorProfile,
    ca_cert: &Path,
) -> io::Result<CursorProcess> {
    std::fs::create_dir_all(profile.root())?;
    let mut command = Command::new(executable);
    command.args(launch_args_transparent(profile));
    for (key, value) in launch_env_transparent(ca_cert) {
        command.env(key, value);
    }
    Ok(CursorProcess {
        child: command.spawn()?,
    })
}

/// Launch Cursor with the local proxy inherited by embedded NodeService workers. Cursor's
/// `http.proxy` setting does not cover every Node client, so the process environment is part of
/// the real end-to-end integration contract.
pub fn launch_with_proxy(
    executable: &Path,
    profile: &CursorProfile,
    proxy_url: &str,
) -> io::Result<CursorProcess> {
    std::fs::create_dir_all(profile.root())?;
    let child = Command::new(executable)
        .args(launch_args_with_proxy(profile, proxy_url))
        .env("HTTP_PROXY", proxy_url)
        .env("HTTPS_PROXY", proxy_url)
        .env("ALL_PROXY", proxy_url)
        .env("http_proxy", proxy_url)
        .env("https_proxy", proxy_url)
        .env("all_proxy", proxy_url)
        .env("NO_PROXY", "localhost,127.0.0.1,::1")
        .env("no_proxy", "localhost,127.0.0.1,::1")
        .spawn()?;
    Ok(CursorProcess { child })
}

impl CursorProfile {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Cursor stores user settings below `<user-data-dir>/User/settings.json`.
    pub fn settings_path(&self) -> PathBuf {
        self.root.join("User").join("settings.json")
    }

    pub fn user_data_dir_arg(&self) -> (&'static str, &Path) {
        ("--user-data-dir", &self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::CursorProfile;
    use std::path::PathBuf;

    #[test]
    fn isolated_profile_uses_a_private_user_settings_path() {
        let profile = CursorProfile::new(PathBuf::from("C:/cc2cx/cursor-profile"));

        assert_eq!(
            profile.settings_path(),
            PathBuf::from("C:/cc2cx/cursor-profile/User/settings.json")
        );
        assert_eq!(profile.user_data_dir_arg().0, "--user-data-dir");
        assert_eq!(
            profile.user_data_dir_arg().1,
            PathBuf::from("C:/cc2cx/cursor-profile").as_path()
        );
    }

    #[test]
    fn launch_args_point_cursor_at_the_isolated_profile() {
        let profile = CursorProfile::new(PathBuf::from("C:/cc2cx/cursor-profile"));
        assert_eq!(
            super::launch_args(&profile),
            vec![
                "--user-data-dir".to_string(),
                "C:/cc2cx/cursor-profile".to_string(),
            ]
        );
    }

    #[test]
    fn transparent_launch_args_disable_quic_without_proxy_server() {
        let profile = CursorProfile::new(PathBuf::from("C:/cc2cx/cursor-profile"));
        let args = super::launch_args_transparent(&profile);
        assert!(
            args.iter().any(|arg| arg == "--disable-quic"),
            "mode T must disable QUIC so Agent traffic stays on TCP :443, got {args:?}"
        );
        assert!(
            args.iter().all(|arg| !arg.contains("--proxy-server")),
            "mode T must not set --proxy-server, got {args:?}"
        );
    }

    #[test]
    fn transparent_launch_args_map_cname_aliases_without_hosts() {
        let profile = CursorProfile::new(PathBuf::from("C:/cc2cx/cursor-profile"));
        let args = super::launch_args_transparent(&profile);
        let rules = args
            .iter()
            .find(|arg| arg.starts_with("--host-resolver-rules="))
            .cloned()
            .expect("mode T must map CNAME aliases without writing Hosts");
        assert!(rules.contains("api2direct.cursor.sh"));
        assert!(rules.contains("agentn.global.api5lat.cursor.sh"));
        assert!(
            rules.contains("agentn.api5.cursor.sh"),
            "mode T must map Agent default host agentn.api5.cursor.sh, got {rules}"
        );
        assert!(rules.contains("127.0.0.2") || rules.contains("127.0.0."));
        assert!(
            args.iter().all(|arg| !arg.contains("--proxy-server")),
            "mode T must not set --proxy-server, got {args:?}"
        );
    }

    #[test]
    fn transparent_launch_env_uses_system_ca_without_proxy() {
        let env = super::launch_env_transparent(std::path::Path::new(r"C:\cc2cx\cursor-ca\ca.crt"));
        let options = env
            .iter()
            .find(|(key, _)| key == "NODE_OPTIONS")
            .map(|(_, value)| value.as_str());
        assert_eq!(options, Some("--use-system-ca"));
        let extra = env
            .iter()
            .find(|(key, _)| key == "NODE_EXTRA_CA_CERTS")
            .map(|(_, value)| value.as_str());
        assert_eq!(extra, Some(r"C:\cc2cx\cursor-ca\ca.crt"));
        assert!(
            env.iter().all(|(key, _)| key != "SSL_CERT_FILE"),
            "mode T must not replace the platform root store with the private CA, got {env:?}"
        );
        assert!(
            env.iter()
                .all(|(key, _)| !key.eq_ignore_ascii_case("HTTP_PROXY")
                    && !key.eq_ignore_ascii_case("HTTPS_PROXY")
                    && !key.eq_ignore_ascii_case("ALL_PROXY")),
            "mode T must not inject a proxy env, got {env:?}"
        );
    }

    #[test]
    fn launch_args_with_proxy_disable_quic_for_real_e2e() {
        let profile = CursorProfile::new(PathBuf::from("C:/cc2cx/cursor-profile"));
        assert_eq!(
            super::launch_args_with_proxy(&profile, "http://127.0.0.1:15722"),
            vec![
                "--user-data-dir".to_string(),
                "C:/cc2cx/cursor-profile".to_string(),
                "--proxy-server=http://127.0.0.1:15722".to_string(),
                "--disable-quic".to_string(),
            ]
        );
    }
}
