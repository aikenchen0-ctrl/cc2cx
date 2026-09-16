//! Isolated Cursor E2E runner.
//!
//! Windows/Linux exercise mode T (Hosts + fake-IP :443). macOS uses mode P (the local HTTP
//! proxy), because the current build has no privileged helper for /etc/hosts or port 443. The
//! runner never touches the default Cursor profile.

use std::{error::Error, path::PathBuf, sync::Arc};

use cc_launch_lib::{
    cursor::{
        e2e::{
            configured_entry_mode, isolated_profile_paths, require_ca, require_exclusive_entry,
            require_isolated_profile, safe_metadata_line, CursorE2eEntryMode,
        },
        harness::CursorHarness,
        hosts,
        mitm::CursorProxyConfig,
        profile::{discover_executable, launch_transparent, launch_with_proxy, CursorProfile},
    },
    get_app_config_dir, AppType, Database,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app_dir = get_app_config_dir();
    let profile_override = std::env::var_os("CC2CX_E2E_PROFILE_DIR")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty());
    let (profile_root, backup_path) =
        isolated_profile_paths(&app_dir, profile_override, std::process::id());
    let profile = CursorProfile::new(profile_root.clone());
    let database = Arc::new(Database::init()?);
    let ca_dir = std::env::var_os("CC2CX_E2E_CA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| app_dir.join("cursor-e2e-ca"));
    require_isolated_profile(&profile)?;
    let harness = CursorHarness::new_for_profile_with_database(
        ca_dir,
        profile.clone(),
        backup_path,
        CursorProxyConfig {
            requested_port: 0,
            ..CursorProxyConfig::default()
        },
        database,
        AppType::Claude,
    );
    let requested_entry_mode = std::env::var("CC2CX_E2E_ENTRY_MODE").ok();
    let expected_entry =
        configured_entry_mode(std::env::consts::OS, requested_entry_mode.as_deref())?;
    let use_transparent_entry = expected_entry == CursorE2eEntryMode::Transparent;
    if use_transparent_entry {
        harness.enable_transparent_entry(hosts::system_path(), 443);
    }

    harness.initialize_ca().await?;
    if std::env::var_os("CC2CX_E2E_INSTALL_CA").is_some_and(|value| value == "1") {
        harness.install_ca().await?;
    }
    let status = harness.status().await?;
    let skip_ca_check =
        std::env::var_os("CC2CX_E2E_SKIP_CA_CHECK").is_some_and(|value| value == "1");
    require_ca(
        status.ca == cc_launch_lib::cursor::ca::CaState::Ready,
        skip_ca_check,
    )?;

    let status = harness.start().await?;
    let entry = require_exclusive_entry(status.managed_hosts, status.proxy_url.as_deref())?;
    if entry != expected_entry {
        return Err(format!("cursor_e2e_lab expected {expected_entry:?}, got {entry:?}").into());
    }
    let executable = discover_executable().ok_or("Cursor executable was not found")?;
    let ca_path = harness.ca_manager().certificate_path();
    let process = if let Some(proxy_url) = status.proxy_url.as_deref() {
        launch_with_proxy(&executable, &profile, proxy_url)?
    } else {
        launch_transparent(&executable, &profile, &ca_path)?
    };
    println!(
        "{}",
        safe_metadata_line("cursor_pid", &process.id().to_string())?
    );
    println!(
        "{}",
        safe_metadata_line("cursor_executable", &executable.display().to_string())?
    );
    println!(
        "{}",
        safe_metadata_line("cursor_profile", &profile_root.display().to_string())?
    );
    println!(
        "{}",
        safe_metadata_line(
            "cursor_entry_mode",
            &format!("{entry:?}").to_ascii_lowercase()
        )?
    );
    println!(
        "{}",
        safe_metadata_line(
            "cursor_backend_port",
            &status.backend_port.unwrap_or_default().to_string()
        )?
    );
    println!(
        "{}",
        safe_metadata_line("cursor_managed_hosts", &status.managed_hosts.to_string())?
    );
    for entry in &status.fake_ip_entries {
        println!(
            "{}",
            safe_metadata_line(
                "cursor_fake_ip",
                &format!("{}={}", entry.hostname, entry.ip)
            )?
        );
    }
    if let Some(entry) = &status.transparent_entry {
        println!(
            "{}",
            safe_metadata_line("cursor_transparent_running", &entry.running.to_string())?
        );
        for address in &entry.addresses {
            println!(
                "{}",
                safe_metadata_line("cursor_transparent_address", address)?
            );
        }
    }
    println!("send the fixed message CC2CX_E2E_OK in this isolated Cursor window");
    println!("press Ctrl+C here after the response and verification");

    tokio::signal::ctrl_c().await?;
    process.stop()?;
    harness.stop().await?;
    Ok(())
}
