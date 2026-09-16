use cc_launch_lib::cursor::{
    e2e::{
        configured_entry_mode, isolated_profile_paths, require_ca, require_exclusive_entry,
        require_isolated_profile, safe_metadata_line, CursorE2eEntryMode,
    },
    profile::{launch_args, CursorProfile},
};
use std::path::PathBuf;

#[test]
fn e2e_profile_must_not_be_the_default_cursor_user_data_dir() {
    let default = cc_launch_lib::cursor::e2e::default_cursor_user_data();
    let error = require_isolated_profile(&CursorProfile::new(default.clone())).unwrap_err();
    assert!(error.to_string().contains("隔离"));
    require_isolated_profile(&CursorProfile::new(PathBuf::from(
        r"C:\Users\血饮\.cc-launch\cursor-e2e-profile",
    )))
    .unwrap();
}

#[test]
fn e2e_refuses_to_run_without_trusted_ca_unless_explicitly_skipped() {
    let error = require_ca(false, false).unwrap_err();
    assert!(error.to_string().contains("CA"));
    require_ca(true, false).unwrap();
    require_ca(false, true).unwrap();
}

#[test]
fn e2e_metadata_omits_credentials() {
    let line = safe_metadata_line("cursor_backend_port", "12345").unwrap();
    assert_eq!(line, "cursor_backend_port=12345");
    let error = safe_metadata_line("authorization", "Bearer secret").unwrap_err();
    assert!(error.to_string().contains("凭证"));
}

#[test]
fn e2e_rejects_hosts_and_proxy_together() {
    let error = require_exclusive_entry(true, Some("http://127.0.0.1:14259")).unwrap_err();
    assert!(
        error.to_string().contains("一种入口"),
        "mixed entry must fail closed, got {error}"
    );
}

#[test]
fn e2e_accepts_transparent_hosts_without_proxy() {
    assert_eq!(
        require_exclusive_entry(true, None).unwrap(),
        CursorE2eEntryMode::Transparent
    );
}

#[test]
fn e2e_accepts_proxy_without_hosts() {
    assert_eq!(
        require_exclusive_entry(false, Some("http://127.0.0.1:14259")).unwrap(),
        CursorE2eEntryMode::Proxy
    );
}

#[test]
fn e2e_transparent_launch_args_do_not_include_proxy_server() {
    let profile = CursorProfile::new(PathBuf::from(
        r"C:\Users\血饮\.cc-launch\cursor-e2e-profile",
    ));
    let args = launch_args(&profile);
    assert!(
        args.iter().all(|arg| !arg.contains("--proxy-server")),
        "mode T must launch without --proxy-server, got {args:?}"
    );
}

#[test]
fn e2e_entry_mode_can_be_selected_without_changing_platform_default() {
    assert_eq!(
        configured_entry_mode("windows", None).unwrap(),
        CursorE2eEntryMode::Transparent
    );
    assert_eq!(
        configured_entry_mode("windows", Some("proxy")).unwrap(),
        CursorE2eEntryMode::Proxy
    );
    assert_eq!(
        configured_entry_mode("macos", Some("transparent")).unwrap(),
        CursorE2eEntryMode::Transparent
    );
}

#[test]
fn e2e_entry_mode_rejects_unknown_override() {
    let error = configured_entry_mode("windows", Some("invalid")).unwrap_err();
    assert!(error.to_string().contains("proxy") && error.to_string().contains("transparent"));
}

#[test]
fn e2e_profile_paths_are_run_scoped_and_overrideable() {
    let app_dir = PathBuf::from(r"C:\cc2cx\app");
    let (profile, backup) = isolated_profile_paths(&app_dir, None, 4242);
    assert_eq!(
        profile,
        PathBuf::from(r"C:\cc2cx\app\cursor-e2e-profile-4242")
    );
    assert_eq!(
        backup,
        PathBuf::from(r"C:\cc2cx\app\cursor-e2e-settings-backup-4242.json")
    );

    let (profile, backup) = isolated_profile_paths(
        &app_dir,
        Some(PathBuf::from(r"D:\cc2cx\isolated-profile")),
        4242,
    );
    assert_eq!(profile, PathBuf::from(r"D:\cc2cx\isolated-profile"));
    assert_eq!(
        backup,
        PathBuf::from(r"D:\cc2cx\isolated-profile\cc2cx-e2e-settings-backup.json")
    );
}
