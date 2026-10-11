//! Lifecycle matrix for native profile candidate admission.
//! test-intent: behavior

use super::*;
use crate::apply::ConfigReloader;
use async_trait::async_trait;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::profile_configuration_transaction::ProfileConfigurationCandidateValidatorPort;
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_store::ProfileStore;
use mihomo_config::profile_document_codec::MihomoProfileDocumentCodec;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
enum CandidateLifecycle {
    Stopped,
    Failed,
    Running,
}

async fn prepare_lifecycle(fixture: &TransactionFixture, lifecycle: CandidateLifecycle) {
    match lifecycle {
        CandidateLifecycle::Stopped => {}
        CandidateLifecycle::Failed => fixture.lifecycle.set_status(CoreLifecycle::Failed),
        CandidateLifecycle::Running => {
            let generation = fixture
                .lifecycle
                .start()
                .await
                .expect("fixture Core starts");
            fixture
                .lifecycle
                .wait_for_ready(generation, Duration::from_secs(1))
                .await
                .expect("fixture Core reaches ready");
            fixture.lifecycle.set_status(CoreLifecycle::Running);
        }
    }
}

#[tokio::test]
async fn foreign_and_stale_candidates_have_zero_writes_across_lifecycles() {
    for lifecycle in [
        CandidateLifecycle::Stopped,
        CandidateLifecycle::Failed,
        CandidateLifecycle::Running,
    ] {
        let fixture = transaction_fixture(NativeValidationResult::Accept).await;
        prepare_lifecycle(&fixture, lifecycle).await;
        let starts_before = fixture.lifecycle.start_calls();
        let stops_before = fixture.lifecycle.stop_calls();
        let original = fixture
            .config
            .load_active_workspace()
            .await
            .expect("initial workspace");

        let mut foreign = original.identity();
        foreign.document.engine_id =
            infiltrator_contract::core_identity::EngineId::new("sing-box").unwrap();
        assert!(matches!(
            fixture
                .transaction
                .transact(request(foreign, b"port: 7891\n"))
                .await,
            Err(PortError::Unsupported { .. })
        ));
        assert_eq!(fixture.profile_store.writes(), 0);

        fixture
            .config
            .save("main", "# newer external edit\nport: 7900\n")
            .await
            .expect("simulate a newer workspace revision");
        let newer = fixture
            .config
            .load_active_workspace()
            .await
            .expect("newer workspace");
        assert!(matches!(
            fixture
                .transaction
                .transact(request(original.identity(), b"port: 7891\n"))
                .await,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::NotReady,
                ..
            }))
        ));

        assert_eq!(fixture.profile_store.writes(), 0);
        assert_eq!(fixture.validator.calls(), 0);
        assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.lifecycle.start_calls(), starts_before);
        assert_eq!(fixture.lifecycle.stop_calls(), stops_before);
        assert_eq!(
            fixture.lifecycle.lifecycle(),
            match lifecycle {
                CandidateLifecycle::Stopped => CoreLifecycle::Stopped,
                CandidateLifecycle::Failed => CoreLifecycle::Failed,
                CandidateLifecycle::Running => CoreLifecycle::Running,
            }
        );
        assert_eq!(
            fixture
                .config
                .load_active_workspace()
                .await
                .expect("foreign and stale candidates do not replace the workspace"),
            newer
        );
    }
}

struct RejectingNativePreflight {
    calls: AtomicUsize,
    reload_calls: AtomicUsize,
    candidate_path: Mutex<Option<PathBuf>>,
    candidate_bytes: Mutex<Option<Vec<u8>>>,
}

impl RejectingNativePreflight {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            reload_calls: AtomicUsize::new(0),
            candidate_path: Mutex::new(None),
            candidate_bytes: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ConfigReloader for RejectingNativePreflight {
    async fn preflight_check(&self, candidate: &Path) -> Result<(), String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.candidate_path.lock().expect("candidate path lock") = Some(candidate.to_path_buf());
        let bytes = tokio::fs::read(candidate)
            .await
            .map_err(|error| error.to_string())?;
        *self.candidate_bytes.lock().expect("candidate bytes lock") = Some(bytes);
        Err("fake Mihomo rejected the candidate".to_owned())
    }

    async fn reload(&self, _candidate: &Path) -> Result<(), String> {
        self.reload_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

fn transaction_with_preflight(
    fixture: &TransactionFixture,
    reloader: Arc<dyn ConfigReloader>,
) -> MihomoProfileConfigurationTransactionAdapter<MockStore> {
    let codec: Arc<dyn ProfileDocumentCodecPort> = Arc::new(MihomoProfileDocumentCodec);
    let validator: Arc<dyn ProfileConfigurationCandidateValidatorPort> = fixture.validator.clone();
    let session: Arc<dyn CoreLifecyclePort> = fixture.lifecycle.clone();
    let profile_store: Arc<dyn ProfileStore> = fixture.profile_store.clone();
    MihomoProfileConfigurationTransactionAdapter::new_with_profile_store_for_test(
        codec,
        validator,
        session,
        Arc::clone(&fixture.config),
        profile_store,
        reloader,
        Arc::new(tokio::sync::Mutex::new(())),
    )
    .expect("fake native validator uses the production Mihomo scope")
}

#[tokio::test]
async fn native_invalid_candidates_have_zero_writes_across_lifecycles() {
    for lifecycle in [
        CandidateLifecycle::Stopped,
        CandidateLifecycle::Failed,
        CandidateLifecycle::Running,
    ] {
        let validation = match lifecycle {
            CandidateLifecycle::Stopped | CandidateLifecycle::Failed => {
                NativeValidationResult::Reject
            }
            CandidateLifecycle::Running => NativeValidationResult::Accept,
        };
        let fixture = transaction_fixture(validation).await;
        prepare_lifecycle(&fixture, lifecycle).await;
        let starts_before = fixture.lifecycle.start_calls();
        let stops_before = fixture.lifecycle.stop_calls();
        let previous = fixture
            .config
            .load_active_workspace()
            .await
            .expect("initial workspace");
        let running_preflight = Arc::new(RejectingNativePreflight::new());
        let result = match lifecycle {
            CandidateLifecycle::Running => {
                let reloader: Arc<dyn ConfigReloader> = running_preflight.clone();
                transaction_with_preflight(&fixture, reloader)
                    .transact(request(previous.identity(), b"port: 7891\n"))
                    .await
            }
            CandidateLifecycle::Stopped | CandidateLifecycle::Failed => {
                fixture
                    .transaction
                    .transact(request(previous.identity(), b"port: 7891\n"))
                    .await
            }
        };

        assert!(matches!(
            result,
            Err(PortError::Rejected(Failure {
                code: ErrorCode::Configuration,
                ..
            }))
        ));
        assert_eq!(fixture.profile_store.writes(), 0);
        assert_eq!(
            fixture
                .config
                .load_active_workspace()
                .await
                .expect("native-invalid candidate leaves active workspace unchanged"),
            previous
        );
        assert_eq!(fixture.lifecycle.start_calls(), starts_before);
        assert_eq!(fixture.lifecycle.stop_calls(), stops_before);
        assert_eq!(
            fixture.lifecycle.lifecycle(),
            match lifecycle {
                CandidateLifecycle::Stopped => CoreLifecycle::Stopped,
                CandidateLifecycle::Failed => CoreLifecycle::Failed,
                CandidateLifecycle::Running => CoreLifecycle::Running,
            }
        );

        match lifecycle {
            CandidateLifecycle::Stopped | CandidateLifecycle::Failed => {
                assert_eq!(fixture.validator.calls(), 1);
                assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
            }
            CandidateLifecycle::Running => {
                assert_eq!(fixture.validator.calls(), 0);
                assert_eq!(running_preflight.calls.load(Ordering::SeqCst), 1);
                assert_eq!(running_preflight.reload_calls.load(Ordering::SeqCst), 0);
                assert_eq!(
                    *running_preflight
                        .candidate_bytes
                        .lock()
                        .expect("candidate bytes lock"),
                    Some(b"port: 7891\n".to_vec())
                );
                let candidate_path = running_preflight
                    .candidate_path
                    .lock()
                    .expect("candidate path lock")
                    .clone()
                    .expect("preflight candidate path");
                assert!(!candidate_path.exists());
            }
        }
    }
}
