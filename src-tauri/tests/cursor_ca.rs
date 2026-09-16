use cc_launch_lib::cursor::ca::{CaManager, CaState};
use tempfile::tempdir;
use x509_parser::prelude::FromDer;

#[test]
fn new_manager_reports_missing_then_generates_a_valid_ca() {
    let dir = tempdir().expect("temp dir");
    let manager = CaManager::new(dir.path().to_path_buf());
    assert_eq!(manager.state().expect("read state"), CaState::Missing);

    manager.initialize().expect("generate CA");
    assert!(matches!(
        manager.state().expect("read generated state"),
        CaState::Untrusted | CaState::Ready
    ));
    assert!(manager
        .certificate_pem()
        .expect("read certificate")
        .contains("BEGIN CERTIFICATE"));
    assert!(manager
        .private_key_pem()
        .expect("read key")
        .contains("PRIVATE KEY"));

    let cert = pem::parse(manager.certificate_pem().unwrap()).unwrap();
    let (_, parsed) = x509_parser::certificate::X509Certificate::from_der(cert.contents())
        .expect("parse generated certificate");
    let validity_days = (parsed.validity().not_after.timestamp()
        - parsed.validity().not_before.timestamp())
        / 86_400;
    assert!((3_640..=3_660).contains(&validity_days));
}

#[test]
fn incomplete_or_mismatched_ca_is_invalid_and_never_loaded() {
    let dir = tempdir().expect("temp dir");
    let manager = CaManager::new(dir.path().to_path_buf());
    std::fs::create_dir_all(dir.path()).unwrap();
    std::fs::write(dir.path().join("ca.crt"), "not a certificate").unwrap();
    std::fs::write(dir.path().join("ca.key"), "not a key").unwrap();

    assert_eq!(manager.state().expect("read state"), CaState::Invalid);
    assert!(manager.load().is_err());
}

#[test]
fn install_requires_initialized_ca_material() {
    let dir = tempdir().expect("temp dir");
    let manager = CaManager::new(dir.path().to_path_buf());

    let error = manager
        .install()
        .expect_err("missing CA must not be installed");
    assert!(error.to_string().contains("CA"));
    assert!(!dir.path().join("ca.crt").exists());
    assert!(!dir.path().join("ca.key").exists());
}

#[test]
fn uninstall_is_idempotent_when_ca_material_is_missing() {
    let dir = tempdir().expect("temp dir");
    let manager = CaManager::new(dir.path().to_path_buf());

    manager
        .uninstall()
        .expect("uninstalling a missing CA should be a no-op");
}

#[cfg(target_os = "windows")]
#[test]
fn windows_uninstall_command_uses_certificate_thumbprint() {
    let dir = tempdir().expect("temp dir");
    let manager = CaManager::new(dir.path().to_path_buf());
    manager.initialize().expect("generate CA");
    let command = manager.uninstall_command().expect("windows command");

    assert!(command.contains("certutil -user -delstore Root"));
    let thumbprint = command.split('"').nth(1).expect("thumbprint argument");
    assert_eq!(thumbprint.len(), 40);
    assert!(thumbprint
        .chars()
        .all(|character| character.is_ascii_hexdigit()));
    assert!(!command.contains("ca.crt"));
}

#[cfg(target_os = "windows")]
#[test]
fn windows_install_persists_exact_ca_in_current_user_root_store() {
    let dir = tempdir().expect("temp dir");
    let manager = CaManager::new(dir.path().to_path_buf());
    manager.initialize().expect("generate CA");

    manager
        .install()
        .expect("install CA into current-user Root");
    assert_eq!(
        manager.state().expect("read installed CA state"),
        CaState::Ready
    );

    manager.uninstall().expect("remove test CA");
}
