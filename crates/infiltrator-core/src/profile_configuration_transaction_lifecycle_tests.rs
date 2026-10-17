//! Lifecycle matrix for native profile candidate admission.
//! test-intent: behavior

use super::*;
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::lifecycle::CoreLifecycle;
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
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("initial workspace");

        let mut foreign = original.identity();
        foreign.document.engine_id = EngineId::new("sing-box").unwrap();
        assert!(matches!(
            fixture
                .transaction
                .transact(request(foreign, b"port: 7891\n"))
                .await,
            Err(PortError::Unsupported { .. })
        ));
        assert_eq!(fixture.profile_store.writes(), 0);

        super::replace_profile_file_for_test(
            &fixture.config,
            "main",
            "# newer external edit\nport: 7900\n",
        )
        .await;
        let newer = fixture
            .config
            .engine_profile_store()
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
                .engine_profile_store()
                .load_active_workspace()
                .await
                .expect("foreign and stale candidates do not replace the workspace"),
            newer
        );
    }
}

#[tokio::test]
async fn inactive_native_invalid_candidates_have_zero_writes() {
    for lifecycle in [CandidateLifecycle::Stopped, CandidateLifecycle::Failed] {
        let fixture = transaction_fixture(NativeValidationResult::Reject).await;
        prepare_lifecycle(&fixture, lifecycle).await;
        let starts_before = fixture.lifecycle.start_calls();
        let stops_before = fixture.lifecycle.stop_calls();
        let previous = fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("initial workspace");
        let result = fixture
            .transaction
            .transact(request(previous.identity(), b"port: 7891\n"))
            .await;

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
                .engine_profile_store()
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
                CandidateLifecycle::Running => unreachable!("inactive test has no running Core"),
            }
        );
        assert_eq!(fixture.validator.calls(), 1);
        assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    }
}
