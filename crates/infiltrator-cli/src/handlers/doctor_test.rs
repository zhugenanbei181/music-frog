use infiltrator_core::bootstrap;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_contract::doctor::DoctorStatus;
use infiltrator_core::doctor_port::MihomoDoctor;
use mihomo_version::manager::VersionManager;

use super::{render_explanation, report_rows, report_summary, status_label};
use crate::test_support::EnvGuard;

/// Plant a runnable fake kernel (`<home>/versions/<version>/mihomo`) so the
/// `version.binary_available` check and `set_default`'s smoke check pass.
#[cfg(unix)]
fn plant_runnable_kernel(home: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let dir = home.join("versions").join("v0.0.0-test");
    std::fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("mihomo");
    std::fs::write(&binary, "#!/bin/sh\necho \"Mihomo Meta v0.0.0-test\"\n").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[tokio::test]
async fn doctor_report_passes_on_bootstrapped_home() {
    let _guard = EnvGuard::acquire().await;
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().to_path_buf();

    bootstrap::ensure_bootstrap_at(&home).await.unwrap();
    plant_runnable_kernel(&home);
    let version_manager = VersionManager::with_home(home.clone()).unwrap();
    version_manager.set_default("v0.0.0-test").await.unwrap();

    let application = DoctorApplication::new(std::sync::Arc::new(MihomoDoctor::with_home(home)));
    let report = application.run(None).await.unwrap();
    assert!(!report.checks.is_empty());
    let failures: Vec<String> = report
        .checks
        .iter()
        .filter(|check| check.status == DoctorStatus::Fail)
        .map(|check| format!("{}: {}", check.id, check.summary))
        .collect();
    assert!(failures.is_empty(), "unexpected failures: {failures:?}");
    assert_eq!(if report.has_failures() { 1 } else { 0 }, 0);
    assert!(report_summary(&report).contains("0 fail"));
    assert_eq!(report_rows(&report).len(), report.checks.len());
}

#[cfg(not(unix))]
#[tokio::test]
async fn doctor_report_flags_missing_kernel_on_non_unix() {
    let _guard = EnvGuard::acquire().await;
    let temp = tempfile::tempdir().unwrap();
    bootstrap::ensure_bootstrap_at(temp.path()).await.unwrap();
    let application = DoctorApplication::new(std::sync::Arc::new(MihomoDoctor::with_home(
        temp.path().to_path_buf(),
    )));
    let report = application.run(Some("version".to_string())).await.unwrap();
    assert_eq!(if report.has_failures() { 1 } else { 0 }, 1);
}

#[tokio::test]
async fn doctor_filter_limits_report_to_matching_checks() {
    let _guard = EnvGuard::acquire().await;
    let temp = tempfile::tempdir().unwrap();
    let application = DoctorApplication::new(std::sync::Arc::new(MihomoDoctor::with_home(
        temp.path().to_path_buf(),
    )));
    let report = application
        .run(Some("service".to_string()))
        .await
        .unwrap();
    assert!(!report.checks.is_empty());
    assert!(
        report
            .checks
            .iter()
            .all(|check| check.id.starts_with("service."))
    );
}

#[test]
fn status_labels_match_severity() {
    assert_eq!(status_label(&DoctorStatus::Pass), "pass");
    assert_eq!(status_label(&DoctorStatus::Warn), "warn");
    assert_eq!(status_label(&DoctorStatus::Fail), "FAIL");
    assert_eq!(status_label(&DoctorStatus::Skip), "skip");
}

#[test]
fn explanation_renders_known_check_and_rejects_unknown_id() {
    let text = render_explanation("config.current_yaml").unwrap();
    assert!(text.contains("config.current_yaml"));
    assert!(text.contains("hint:"));
    assert!(render_explanation("no.such_check").is_err());
}
