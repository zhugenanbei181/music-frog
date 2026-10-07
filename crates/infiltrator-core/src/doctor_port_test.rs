//! test-intent: behavior
use super::*;
use infiltrator_contract::error::{ErrorCode, Failure};
use mihomo_api::error::MihomoError;
use std::fs::{read, write};
use tempfile::tempdir;

#[test]
fn nested_io_permission_and_storage_errors_keep_category_context_and_recovery_policy() {
    for (kind, code, retryable) in [
        (ErrorKind::PermissionDenied, ErrorCode::Permission, false),
        (ErrorKind::NotFound, ErrorCode::Storage, false),
        (ErrorKind::Interrupted, ErrorCode::Storage, true),
    ] {
        let error = anyhow::Error::new(Error::new(kind, "concrete host failure"))
            .context("repair configuration");
        let failure = Failure::from(adapter_error(error));
        assert_eq!(failure.code, code);
        assert_eq!(failure.retryable, retryable);
        assert!(failure.message.contains("repair configuration"));
        assert!(failure.message.contains("concrete host failure"));
    }
    let expected = Failure::new(ErrorCode::InvalidInput, "invalid configuration", false);
    assert_eq!(
        Failure::from(adapter_error(
            anyhow::Error::new(PortError::Rejected(expected.clone())).context("host adapter")
        )),
        expected
    );
    for (kind, code) in [
        (ErrorKind::PermissionDenied, ErrorCode::Permission),
        (ErrorKind::NotADirectory, ErrorCode::Storage),
    ] {
        let error = anyhow::Error::new(MihomoError::Io(Error::new(
            kind,
            "profile root unavailable",
        )))
        .context("construct profile store");
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
