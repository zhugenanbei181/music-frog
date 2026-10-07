//! Cancellation crosses the real manager/port boundary without network or installation effects.
//! test-intent: behavior
use super::*;
use tempfile::TempDir;

struct CanceledProgress;
impl VersionProgressSink for CanceledProgress {
    fn progress(&self, _: VersionDownloadProgress) {
        panic!("canceled installation must not report transfer progress");
    }
    fn is_cancelled(&self) -> bool {
        true
    }
}

#[tokio::test]
async fn cancelled_download_keeps_the_original_category_policy_and_verification_receipt() {
    let home = TempDir::new().unwrap();
    let port = MihomoVersionPort::with_home(home.path().into()).unwrap();
    let failure = Failure::from(
        port.install("v-test".into(), Arc::new(CanceledProgress))
            .await
            .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::Canceled);
    assert_eq!(failure.reason, Some(FailureReason::DownloadCanceled));
    assert!(!failure.retryable);
    assert_eq!(
        port.verification(),
        CoreArtifactVerification::Rejected {
            version: "v-test".into(),
            failure,
        }
    );
    assert!(!home.path().join("versions").exists());
}

#[tokio::test]
async fn separate_instances_do_not_share_verification_receipt() {
    let home1 = TempDir::new().unwrap();
    let home2 = TempDir::new().unwrap();
    let port1 = MihomoVersionPort::with_home(home1.path().into()).unwrap();
    let port2 = MihomoVersionPort::with_home(home2.path().into()).unwrap();

    let _ = port1
        .install("v-test".into(), Arc::new(CanceledProgress))
        .await;

    assert!(matches!(
        port1.verification(),
        CoreArtifactVerification::Rejected { .. }
    ));
    assert_eq!(port2.verification(), CoreArtifactVerification::Unknown);
}
