//! Real isolated host files; no invented receipt paths or overwritten documents.
use super::DesktopLogExportPort;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::session::SessionToken;
use infiltrator_domain::log_export::prepare_log_export;
use std::fs;
use tempfile::tempdir;

#[test]
fn complete_redacted_bytes_are_published_once_and_the_real_path_survives_retry() {
    let directory = tempdir().unwrap();
    let writer = DesktopLogExportPort::new(directory.path());
    assert!(!directory.path().join("exports").exists());
    let artifact = prepare_log_export(
        LogSession {
            generation: 1,
            token: SessionToken::new(37),
        },
        1,
        &[(4, "INFO[1] password=credential café".into())],
        &[],
    )
    .unwrap();
    let receipt = writer.publish(artifact.clone()).unwrap();
    assert_eq!(
        fs::read_to_string(&receipt.path).unwrap(),
        "INFO[1] password=*** café\n"
    );
    assert_eq!(
        fs::metadata(&receipt.path).unwrap().len() as usize,
        receipt.bytes_written
    );
    assert_eq!(receipt.summary, artifact.summary);
    assert_eq!(writer.publish(artifact).unwrap(), receipt);
    let entries: Vec<_> = fs::read_dir(directory.path().join("exports"))
        .unwrap()
        .collect();
    assert_eq!(
        entries.len(),
        1,
        "publication must remove only its own temporary file"
    );
}
#[test]
fn invalid_bytes_and_destination_collision_never_replace_the_original_artifact() {
    let directory = tempdir().unwrap();
    let writer = DesktopLogExportPort::new(directory.path());
    let artifact = prepare_log_export(
        LogSession {
            generation: 1,
            token: SessionToken::new(37),
        },
        1,
        &[(4, "INFO[1] healthy".into())],
        &[],
    )
    .unwrap();
    let mut corrupted = artifact.clone();
    corrupted.content.push_str("extra");
    assert_eq!(
        writer.publish(corrupted).unwrap_err().error_code(),
        ErrorCode::InvalidInput
    );
    assert!(!directory.path().join("exports").exists());
    let receipt = writer.publish(artifact.clone()).unwrap();
    fs::write(&receipt.path, "owned by user").unwrap();
    assert_eq!(
        writer.publish(artifact).unwrap_err().error_code(),
        ErrorCode::Storage
    );
    assert_eq!(fs::read_to_string(&receipt.path).unwrap(), "owned by user");
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        1
    );
}
#[test]
fn unusable_destination_reports_storage_failure_without_a_success_receipt() {
    let directory = tempdir().unwrap();
    fs::write(directory.path().join("exports"), "keep existing file").unwrap();
    let writer = DesktopLogExportPort::new(directory.path());
    let artifact = prepare_log_export(
        LogSession {
            generation: 1,
            token: SessionToken::new(37),
        },
        1,
        &[],
        &[],
    )
    .unwrap();
    assert_eq!(
        writer.publish(artifact).unwrap_err().error_code(),
        ErrorCode::Storage
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("exports")).unwrap(),
        "keep existing file"
    );
}
