use super::{
    ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION, ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof,
    ActiveProfileReferenceRecord, LegacySelectionMigration, parse_active_core_owner_proof_record,
};
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;

#[test]
fn legacy_bound_owner_proof_migrates_to_unverified_launch_state() {
    let legacy = serde_json::json!({
        "schema_version": 1,
        "state": {
            "state": "bound",
            "owner": {
                "identity": {
                    "engine": "mihomo",
                    "instance": "legacy-owner",
                    "version": null
                },
                "generation": 1,
                "token": "legacy-token"
            }
        }
    });

    let migrated = parse_active_core_owner_proof_record(&legacy.to_string()).unwrap();

    assert_eq!(
        migrated.schema_version,
        ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION
    );
    assert!(matches!(migrated.state, ActiveCoreOwnerProof::Bound { .. }));
    assert_eq!(
        migrated.launch,
        ActiveCoreOwnerLaunchProof::LegacyUnverified
    );
}

#[test]
fn new_reference_record_starts_unselected_and_explicitly_pending_migration() {
    let record = ActiveProfileReferenceRecord::pending_legacy_migration();

    assert!(matches!(
        record.selection,
        ActiveProfileReferenceState::Unselected { revision }
            if revision.value() == 0
    ));
    assert_eq!(
        record.legacy_migration,
        LegacySelectionMigration::PendingDesktopMihomoV0
    );
    assert!(matches!(record.owner, super::ActiveCoreOwnerState::Unowned));
}
