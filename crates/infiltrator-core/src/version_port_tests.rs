//! Cancellation crosses the real manager/port boundary without network or installation effects.
//! test-intent: behavior
use super::*;
use infiltrator_ports::version::{VersionInstallRequest, VersionPort, VersionProgressSink};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use mihomo_version::manager::VersionManager;
use std::sync::{Arc, Mutex};
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
    let port =
        MihomoVersionPort::with_home(home.path().into(), MihomoVersionPort::engine_identifier())
            .unwrap();
    let engine_id = MihomoVersionPort::engine_identifier();
    let failure = Failure::from(
        port.install(VersionInstallRequest {
            engine_id: engine_id.clone(),
            version: "v-test".into(),
            progress: Arc::new(CanceledProgress),
        })
        .await
        .unwrap_err(),
    );
    assert_eq!(failure.code, ErrorCode::Canceled);
    assert_eq!(failure.reason, Some(FailureReason::DownloadCanceled));
    assert!(!failure.retryable);
    assert_eq!(
        port.verification(&MihomoVersionPort::engine_identifier()),
        CoreArtifactVerification::Rejected {
            engine_id,
            version: "v-test".into(),
            failure,
        }
    );
    assert!(!home.path().join("engines/mihomo/versions").exists());
}

#[tokio::test]
async fn separate_instances_do_not_share_verification_receipt() {
    let home1 = TempDir::new().unwrap();
    let home2 = TempDir::new().unwrap();
    let engine_id = MihomoVersionPort::engine_identifier();
    let port1 = MihomoVersionPort::with_home(home1.path().into(), engine_id.clone()).unwrap();
    let port2 = MihomoVersionPort::with_home(home2.path().into(), engine_id.clone()).unwrap();

    let _ = port1
        .install(VersionInstallRequest {
            engine_id: engine_id.clone(),
            version: "v-test".into(),
            progress: Arc::new(CanceledProgress),
        })
        .await;

    assert!(matches!(
        port1.verification(&engine_id),
        CoreArtifactVerification::Rejected { .. }
    ));
    assert_eq!(
        port2.verification(&engine_id),
        CoreArtifactVerification::Unknown
    );
}

#[tokio::test]
async fn mihomo_port_rejects_another_engine_before_touching_its_store() {
    let home = TempDir::new().unwrap();
    let port =
        MihomoVersionPort::with_home(home.path().into(), MihomoVersionPort::engine_identifier())
            .unwrap();
    let wrong_engine = EngineId::new("sing-box").unwrap();
    let error = port
        .install(VersionInstallRequest {
            engine_id: wrong_engine,
            version: "v-test".into(),
            progress: Arc::new(CanceledProgress),
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        PortError::Unsupported {
            capability: Capability::CoreVersionInstall,
            ..
        }
    ));
    assert!(!home.path().join("engines/mihomo/versions").exists());
}

#[test]
fn verification_receipt_from_another_engine_is_reported_as_unknown() {
    let home = TempDir::new().unwrap();
    let manager =
        VersionManager::for_engine(home.path().into(), MihomoVersionPort::engine_identifier())
            .unwrap();
    let foreign_verification = Arc::new(Mutex::new(CoreArtifactVerification::Verified {
        engine_id: EngineId::new("sing-box").unwrap(),
        version: "v1.2.3".to_owned(),
    }));
    let port = MihomoVersionPort::with_shared_verification(
        MihomoVersionPort::engine_identifier(),
        manager,
        foreign_verification,
    )
    .unwrap();

    assert_eq!(
        port.verification(&MihomoVersionPort::engine_identifier()),
        CoreArtifactVerification::Unknown
    );
}
