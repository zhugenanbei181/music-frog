//! Exact rollback and rollback-failure behavior for unpublished configuration candidates.

use super::*;

#[tokio::test]
async fn inactive_readback_failure_returns_rolled_back_or_rollback_failed_truthfully() {
    for store_behavior in [
        StoreBehavior::CorruptReadbackAndRestore,
        StoreBehavior::CorruptReadbackAndFailRestore,
    ] {
        let fixture =
            transaction_fixture_with_store_behavior(NativeValidationResult::Accept, store_behavior)
                .await;
        let sidecar_path = options_path(fixture.config.config_dir(), "main");
        create_dir_all(sidecar_path.parent().expect("sidecar parent"))
            .await
            .expect("create sidecar directory");
        let previous_sidecar = b"# prior opaque bytes\r\n\xff\x00";
        write(&sidecar_path, previous_sidecar)
            .await
            .expect("seed exact prior sidecar bytes");
        let before = fixture
            .config
            .engine_profile_store()
            .load_active_workspace()
            .await
            .expect("workspace with exact prior sidecar");
        let candidate = b"# candidate\r\nport: 7893\r\n";
        let mut proposal = request(before.identity(), candidate).clone();
        let candidate_sidecar = b"# candidate sidecar\r\n\xfe\x01";
        proposal.update.sidecar_bytes = Some(candidate_sidecar.to_vec());

        let result = fixture
            .transaction
            .transact(proposal)
            .await
            .expect("post-publication failure is a typed terminal outcome");

        match store_behavior {
            StoreBehavior::CorruptReadbackAndRestore => {
                let ProfileConfigurationTransactionOutcome::RolledBack { restored, .. } = result
                else {
                    panic!("successful exact recovery must report RolledBack");
                };
                assert_eq!(restored, before);
                assert_eq!(
                    fixture
                        .config
                        .engine_profile_store()
                        .load_active_workspace()
                        .await
                        .expect("exact previous workspace after recovery"),
                    before
                );
            }
            StoreBehavior::CorruptReadbackAndFailRestore => {
                let ProfileConfigurationTransactionOutcome::RollbackFailed { cause, rollback } =
                    result
                else {
                    panic!("failed exact recovery must report RollbackFailed");
                };
                assert_eq!(cause.code, ErrorCode::InvalidState);
                assert_eq!(rollback.code, ErrorCode::Storage);
                let uncertain = fixture
                    .config
                    .engine_profile_store()
                    .load_active_workspace()
                    .await
                    .expect("workspace state after injected restore failure");
                assert_eq!(uncertain.document.raw_bytes, candidate);
                assert_eq!(
                    uncertain.sidecar_bytes.as_deref(),
                    Some(candidate_sidecar.as_slice())
                );
                assert_ne!(uncertain, before);
            }
            StoreBehavior::Exact => unreachable!("fault test requires an injected readback error"),
        }
        assert_eq!(fixture.profile_store.writes(), 1);
        assert_eq!(fixture.profile_store.restores(), 1);
        assert_eq!(fixture.lifecycle.start_calls(), 0);
        assert_eq!(fixture.lifecycle.stop_calls(), 0);
        assert_eq!(fixture.reloader.calls.load(Ordering::SeqCst), 0);
    }
}
