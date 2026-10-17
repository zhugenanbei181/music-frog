use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::active_reference_owner::{
    ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION, ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof,
    ActiveCoreOwnerProofRecord, ActiveCoreOwnerState, ActiveOwnerToken,
    ActiveProfileReferenceRecord,
};
use crate::active_reference_owner_coordinator::ActiveProfileOwnerCoordinator;
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use tempfile::TempDir;

#[tokio::test]
async fn owner_normalization_clears_bound_record_only_after_exact_terminal_proof() {
    let directory = TempDir::new().unwrap();
    let file = ActiveProfileReferenceFile::new(directory.path());
    let identity = CoreIdentity::new(
        EngineId::new("mihomo").unwrap(),
        CoreInstanceId::new("test-owner").unwrap(),
        None,
    );
    let generation = CoreGeneration::new(4);
    let token = ActiveOwnerToken("owner-token".into());
    let mut record = ActiveProfileReferenceRecord {
        owner: ActiveCoreOwnerState::Bound {
            identity: identity.clone(),
            generation,
            token: token.clone(),
        },
        ..ActiveProfileReferenceRecord::pending_legacy_migration()
    };
    file.write_active_profile_record_sync(&record).unwrap();
    file.write_owner_proof_sync(&ActiveCoreOwnerProofRecord {
        schema_version: ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
        state: ActiveCoreOwnerProof::Terminal {
            identity,
            bound_generation: generation,
            terminal_generation: CoreGeneration::new(5),
            token,
        },
        launch: ActiveCoreOwnerLaunchProof::LegacyUnverified,
    })
    .unwrap();
    let (canonical_root, root_guard) = file.acquire_root_lock().await.unwrap();

    let (normalized, owner_state_guard) = ActiveProfileOwnerCoordinator::new(canonical_root)
        .normalize_owner_state_unlocked(&mut record)
        .await
        .unwrap();

    assert!(matches!(normalized, ActiveCoreOwnerState::Unowned));
    assert!(matches!(record.owner, ActiveCoreOwnerState::Unowned));
    drop(owner_state_guard);
    drop(root_guard);
}
