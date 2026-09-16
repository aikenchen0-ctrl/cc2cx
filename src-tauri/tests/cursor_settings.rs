use cc_launch_lib::cursor::settings::CursorSettingsStore;
use serde_json::json;
use sha2::Digest;
use std::fs;
use tempfile::tempdir;

#[test]
fn apply_and_restore_preserves_original_jsonc_bytes() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("User").join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    let original = br#"{
  // user comment must survive a full rollback
  "editor.fontSize": 14,
  "http.proxy": "http://user-proxy:8080"
}
"#;
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, original).unwrap();

    let store = CursorSettingsStore::new(path.clone(), backup.clone());
    store
        .apply_proxy("http://127.0.0.1:43121")
        .expect("apply proxy");
    let changed = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = json5::from_str(&changed).expect("valid JSONC after apply");
    assert_eq!(parsed["http.proxy"], json!("http://127.0.0.1:43121"));
    assert_eq!(parsed["http.proxySupport"], json!("on"));
    assert_eq!(parsed["cursor.general.disableHttp2"], json!(true));

    store.restore().expect("restore settings");
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!backup.exists());
}

#[test]
fn restore_refuses_to_overwrite_external_changes() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    fs::write(&path, br#"{"editor.fontSize":14}"#).unwrap();
    let store = CursorSettingsStore::new(path.clone(), backup.clone());
    store
        .apply_proxy("http://127.0.0.1:43121")
        .expect("apply proxy");
    fs::write(&path, br#"{"editor.fontSize":18}"#).unwrap();

    let error = store.restore().expect_err("external edit must conflict");
    assert!(error.to_string().contains("外部修改"));
    assert_eq!(fs::read(&path).unwrap(), br#"{"editor.fontSize":18}"#);
    assert!(backup.exists());
}

#[test]
fn apply_rejects_invalid_proxy_urls_without_touching_settings() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    let original = br#"{"editor.fontSize":14}"#;
    fs::write(&path, original).unwrap();
    let store = CursorSettingsStore::new(path.clone(), backup);

    assert!(store.apply_proxy("not-a-url").is_err());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn apply_trims_proxy_url_before_persisting_settings() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    fs::write(&path, br#"{"editor.fontSize":14}"#).unwrap();
    let store = CursorSettingsStore::new(path.clone(), backup);

    store
        .apply_proxy("  http://127.0.0.1:43121  ")
        .expect("trimmed proxy URL should be accepted");

    let settings: serde_json::Value =
        json5::from_str(&fs::read_to_string(&path).unwrap()).expect("valid JSON");
    assert_eq!(settings["http.proxy"], json!("http://127.0.0.1:43121"));
    assert_eq!(
        settings["http.proxyKerberosServicePrincipal"],
        json!("http://127.0.0.1:43121")
    );
}

#[test]
fn apply_sets_disable_http2_and_restore_preserves_prior_value() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    let original = br#"{"editor.fontSize":14,"cursor.general.disableHttp2":false}"#;
    fs::write(&path, original).unwrap();
    let store = CursorSettingsStore::new(path.clone(), backup);

    store
        .apply_proxy("http://127.0.0.1:43121")
        .expect("proxy should apply");

    let settings: serde_json::Value =
        json5::from_str(&fs::read_to_string(&path).unwrap()).expect("valid JSON");
    assert_eq!(settings["cursor.general.disableHttp2"], json!(true));

    store.restore().expect("restore settings");
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn matches_proxy_requires_disable_http2() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    fs::write(
        &path,
        br#"{
          "http.proxy": "http://127.0.0.1:43121",
          "http.proxySupport": "on",
          "http.experimental.systemCertificatesV2": true,
          "cursor.general.disableHttp2": true
        }"#,
    )
    .unwrap();
    let store = CursorSettingsStore::new(path, backup);

    assert!(store
        .matches_proxy("http://127.0.0.1:43121")
        .expect("settings should parse"));
}

#[test]
fn restore_discards_backup_when_apply_was_interrupted_before_file_replace() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("settings.json");
    let backup = dir.path().join("cc2cx-backup.json");
    let original = br#"{"editor.fontSize":14}"#;
    fs::write(&path, original).unwrap();
    let store = CursorSettingsStore::new(path.clone(), backup.clone());

    // Simulate the crash window by writing a valid backup whose applied hash differs,
    // while leaving the original settings file untouched.
    let original_sha256 = format!("{:x}", sha2::Sha256::digest(original));
    let backup_doc = serde_json::json!({
        "path": path,
        "had_file": true,
        "original": original,
        "original_sha256": original_sha256,
        "applied_sha256": "not-the-original",
    });
    fs::write(&backup, serde_json::to_vec(&backup_doc).unwrap()).unwrap();

    store
        .restore()
        .expect("stale transaction should be discarded");
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!backup.exists());
}
