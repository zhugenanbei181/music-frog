//! test-intent: behavior
use super::*;
use infiltrator_domain::error::DomainFailureCategory;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::fs::{read, write};
use tempfile::tempdir;

#[test]
fn nested_io_permission_and_storage_errors_keep_category_context_and_recovery_policy() {
    for (kind, code, retryable) in [
        (ErrorKind::PermissionDenied, ErrorCode::Permission, false),
        (ErrorKind::NotFound, ErrorCode::Storage, false),
        (ErrorKind::Interrupted, ErrorCode::Storage, true),
    ] {
        let error = CoreError::io_with_kind(kind, "repair configuration: concrete host failure");
        let failure = Failure::from(adapter_error(error));
        assert_eq!(failure.code, code);
        assert_eq!(failure.retryable, retryable);
        assert!(failure.message.contains("repair configuration"));
        assert!(failure.message.contains("concrete host failure"));
    }
    let expected = Failure::new(ErrorCode::InvalidInput, "invalid configuration", false);
    let converted = CoreError::from(PortError::Rejected(expected.clone()));
    assert_eq!(converted.category(), DomainFailureCategory::Other);
    let failure = Failure::from(adapter_error(CoreError::new(
        converted.category(),
        format!("host adapter: {}", converted.message()),
    )));
    assert_eq!(failure.code, ErrorCode::Internal);
    assert!(!failure.retryable);
    assert!(failure.message.contains("host adapter"));
    assert!(failure.message.contains("invalid configuration"));
    for (kind, code) in [
        (ErrorKind::PermissionDenied, ErrorCode::Permission),
        (ErrorKind::NotADirectory, ErrorCode::Storage),
    ] {
        let error =
            CoreError::io_with_kind(kind, "construct profile store: profile root unavailable");
        let failure = Failure::from(adapter_error(error));
        assert_eq!(failure.code, code);
        assert!(failure.message.contains("construct profile store"));
        assert!(failure.message.contains("profile root unavailable"));
    }
}
#[tokio::test]
async fn real_bootstrap_filesystem_conflict_is_storage_failure_and_never_rewrites_the_file() {
    let dir = tempdir().unwrap();
    let home = dir.path().join("home");
    write(&home, b"operator data").unwrap();
    let port = MihomoDoctor::with_home(home.clone());
    let failure = Failure::from(port.bootstrap().await.unwrap_err());
    assert_eq!(failure.code, ErrorCode::Storage);
    assert_eq!(read(&home).unwrap(), b"operator data");
}
