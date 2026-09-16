use cc_launch_lib::cursor::hosts::{enable, require_writable, restore, system_path};
use std::fs;
use tempfile::tempdir;

#[test]
fn hosts_transaction_is_idempotent_and_restores_exact_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hosts");
    let original = b"127.0.0.1 localhost\n# user entry\n";
    fs::write(&path, original).unwrap();
    let tx = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    let applied = fs::read_to_string(&path).unwrap();
    assert!(applied.contains("127.0.0.1 localhost"));
    assert!(applied.contains("# user entry"));
    assert!(applied.contains("# BEGIN CC2CX CURSOR"));
    assert!(applied.contains("127.0.0.2 api2.cursor.sh"));
    assert!(applied.contains("# END CC2CX CURSOR"));
    let tx2 = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), applied);
    assert_eq!(tx.applied_sha256(), tx2.applied_sha256());
    restore(&tx).unwrap();
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn hosts_transaction_refuses_to_overwrite_external_changes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hosts");
    fs::write(&path, "127.0.0.1 localhost\n").unwrap();
    let tx = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    fs::write(&path, "127.0.0.1 localhost\n# external edit\n").unwrap();
    let error = restore(&tx).unwrap_err();
    assert!(error.to_string().contains("外部修改"));
}

#[test]
fn system_hosts_path_is_the_os_hosts_file() {
    let path = system_path();
    assert_eq!(path.file_name().unwrap(), "hosts");
    #[cfg(windows)]
    {
        let rendered = path.to_string_lossy().to_ascii_lowercase();
        assert!(rendered.contains("drivers"));
        assert!(rendered.contains("etc"));
    }
}

#[test]
fn enable_skips_write_when_existing_block_is_a_subset_and_not_writable() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hosts");
    let existing = b"127.0.0.1 localhost\n# BEGIN CC2CX CURSOR\n127.0.0.2 api2.cursor.sh\n127.0.0.3 api3.cursor.sh\n# END CC2CX CURSOR\n";
    fs::write(&path, existing).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();

    enable(
        &path,
        &[
            ("127.0.0.2".parse().unwrap(), "api2.cursor.sh"),
            ("127.0.0.3".parse().unwrap(), "api3.cursor.sh"),
            ("127.0.0.4".parse().unwrap(), "api2direct.cursor.sh"),
        ],
    )
    .expect("readonly subset Hosts must not fail closed");
    assert_eq!(fs::read(&path).unwrap(), existing);
}

#[test]
fn enable_skips_write_when_managed_entries_already_match() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hosts");
    let existing = b"\xEF\xBB\xBF127.0.0.1 localhost\r\n# BEGIN CC2CX CURSOR\r\n127.0.0.2 api2.cursor.sh\r\n# END CC2CX CURSOR\r\n";
    fs::write(&path, existing).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();

    let tx = enable(&path, &[("127.0.0.2".parse().unwrap(), "api2.cursor.sh")]).unwrap();
    assert_eq!(fs::read(&path).unwrap(), existing);
    assert_eq!(tx.applied_sha256().len(), 64);
}

#[test]
fn require_writable_fails_closed_without_changing_bytes() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("hosts");
    let original = b"127.0.0.1 localhost\n";
    fs::write(&path, original).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    let error = require_writable(&path).unwrap_err();
    assert!(error.to_string().contains("提升权限"));
    assert_eq!(fs::read(&path).unwrap(), original);
}
