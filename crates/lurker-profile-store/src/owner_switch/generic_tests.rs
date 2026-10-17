//! The moved machinery must not be hard-coded to the product selection.
//!
//! This test instantiates the exact same file, CAS, coordinator and lease code
//! with a test-only selection payload and a test-only record that carries its
//! own product-style metadata field.

use crate::layout::ProfileStoreLayout;
use crate::owner_switch::cas::{OwnerSwitchCasReady, OwnerSwitchFence};
use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::coordinator::OwnerSwitchCoordinator;
use crate::owner_switch::file::OwnerSwitchFile;
use crate::owner_switch::owner::{
    ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION, ActiveCoreOwnerLaunchProof, ActiveCoreOwnerProof,
    ActiveCoreOwnerProofRecord, ActiveCoreOwnerState, ActiveOwnerToken,
};
use crate::owner_switch::recovery::{recover_old_owner_after_terminal, recover_owner_switch};
use crate::owner_switch::registry::{lock_local_owner_registry, remove_local_owner_registration};
use crate::owner_switch::stage::switch_snapshot;
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRecovery, EngineSwitchRevision, EngineSwitchSnapshot,
    EngineSwitchStage,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_ports::error::PortError;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tempfile::TempDir;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "selection", rename_all = "snake_case")]
enum TestSelection {
    Idle { revision: u64 },
    Engine { engine: EngineId, revision: u64 },
}

impl TestSelection {
    fn revision(&self) -> u64 {
        match self {
            Self::Idle { revision } | Self::Engine { revision, .. } => *revision,
        }
    }

    fn engine(&self) -> Option<&EngineId> {
        match self {
            Self::Idle { .. } => None,
            Self::Engine { engine, .. } => Some(engine),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestRecord {
    schema_version: u32,
    selection: TestSelection,
    /// Product-style metadata the neutral machinery must round-trip untouched.
    metadata: String,
    owner: ActiveCoreOwnerState<TestSelection>,
}

struct TestCodec;

impl OwnerSwitchCodec for TestCodec {
    type Selection = TestSelection;
    type Record = TestRecord;

    fn layout() -> &'static ProfileStoreLayout {
        static LAYOUT: OnceLock<ProfileStoreLayout> = OnceLock::new();
        LAYOUT.get_or_init(ProfileStoreLayout::default)
    }

    fn selection_revision(selection: &Self::Selection) -> EngineSwitchRevision {
        EngineSwitchRevision::new(selection.revision())
    }

    fn selection_engine(selection: &Self::Selection) -> Option<&EngineId> {
        selection.engine()
    }

    fn pending() -> Self::Record {
        TestRecord {
            schema_version: 1,
            selection: TestSelection::Idle { revision: 0 },
            metadata: String::new(),
            owner: ActiveCoreOwnerState::Unowned,
        }
    }

    fn selection(record: &Self::Record) -> &Self::Selection {
        &record.selection
    }

    fn owner(record: &Self::Record) -> &ActiveCoreOwnerState<Self::Selection> {
        &record.owner
    }

    fn set_selection(record: &mut Self::Record, selection: Self::Selection) {
        record.selection = selection;
        record.metadata = "explicit".to_owned();
    }

    fn set_owner(record: &mut Self::Record, owner: ActiveCoreOwnerState<Self::Selection>) {
        record.owner = owner;
    }

    fn validate(record: &Self::Record) -> Result<(), PortError> {
        if record.schema_version != 1 {
            return Err(PortError::Failed("unsupported test record".to_owned()));
        }
        Ok(())
    }

    fn encode(record: &Self::Record) -> Result<Vec<u8>, PortError> {
        serde_json::to_vec_pretty(record).map_err(|error| PortError::Io(error.to_string()))
    }

    fn decode(bytes: &[u8]) -> Result<Self::Record, PortError> {
        serde_json::from_slice(bytes).map_err(|error| PortError::Io(error.to_string()))
    }
}

fn engine(value: &str) -> EngineId {
    EngineId::new(value).expect("test engine")
}

fn identity(engine_id: EngineId, instance: &str) -> CoreIdentity {
    CoreIdentity::new(
        engine_id,
        CoreInstanceId::new(instance).expect("test instance"),
        None,
    )
}

fn terminal_proof(
    identity: &CoreIdentity,
    bound_generation: CoreGeneration,
    terminal_generation: CoreGeneration,
    token: &ActiveOwnerToken,
) -> ActiveCoreOwnerProofRecord {
    ActiveCoreOwnerProofRecord {
        schema_version: ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
        state: ActiveCoreOwnerProof::Terminal {
            identity: identity.clone(),
            bound_generation,
            terminal_generation,
            token: token.clone(),
        },
        launch: ActiveCoreOwnerLaunchProof::Unstarted,
    }
}

#[test]
fn cas_and_file_round_trip_a_second_selection_payload() {
    let directory = TempDir::new().unwrap();
    let file = OwnerSwitchFile::<TestCodec>::new(directory.path());
    assert!(file.read_active_profile_record_sync().unwrap().is_none());

    let record = TestRecord {
        schema_version: 1,
        selection: TestSelection::Engine {
            engine: engine("mihomo"),
            revision: 3,
        },
        metadata: "kept".to_owned(),
        owner: ActiveCoreOwnerState::Unowned,
    };
    file.publish_active_profile_record_sync(&record).unwrap();
    let observed = file.read_active_profile_record_sync().unwrap().unwrap();
    assert_eq!(observed, record, "test metadata did not round-trip");

    let ready =
        OwnerSwitchCasReady::<TestCodec>::compare(&record.selection, EngineSwitchRevision::new(3))
            .unwrap();
    let committed = ready
        .transition(
            TestSelection::Idle { revision: 4 },
            OwnerSwitchFence::Unowned,
        )
        .unwrap();
    assert_eq!(committed, TestSelection::Idle { revision: 4 });

    let snapshot = switch_snapshot::<TestCodec>(&record).unwrap();
    assert_eq!(snapshot.revision, EngineSwitchRevision::new(3));
    assert_eq!(snapshot.owner, EngineOwnerState::Unowned);
}

#[tokio::test]
async fn the_same_switch_leases_drive_a_second_selection_payload() {
    let directory = TempDir::new().unwrap();
    let config_root = directory.path().to_path_buf();
    let old_identity = identity(engine("mihomo"), "old-owner");
    let old_generation = CoreGeneration::new(0);
    let old_token = ActiveOwnerToken("old-owner-token".to_owned());
    let previous = TestSelection::Engine {
        engine: engine("mihomo"),
        revision: 1,
    };
    let record = TestRecord {
        schema_version: 1,
        selection: previous.clone(),
        metadata: String::new(),
        owner: ActiveCoreOwnerState::Bound {
            identity: old_identity.clone(),
            generation: old_generation,
            token: old_token.clone(),
        },
    };
    let file = OwnerSwitchFile::<TestCodec>::new(config_root.clone());
    file.write_active_profile_record_sync(&record).unwrap();
    file.write_owner_proof_sync(&ActiveCoreOwnerProofRecord {
        schema_version: ACTIVE_CORE_OWNER_PROOF_SCHEMA_VERSION,
        state: ActiveCoreOwnerProof::Bound {
            identity: old_identity.clone(),
            generation: old_generation,
            token: old_token.clone(),
        },
        launch: ActiveCoreOwnerLaunchProof::Unstarted,
    })
    .unwrap();

    let (canonical_root, root_guard) = file.acquire_root_lock().await.unwrap();
    let owner_lock = OwnerSwitchFile::<TestCodec>::new(canonical_root.clone())
        .try_acquire_owner_lock()
        .unwrap()
        .expect("owner admission is free");
    lock_local_owner_registry()
        .unwrap()
        .register_bound(
            canonical_root.clone(),
            old_token.clone(),
            old_identity.clone(),
            old_generation,
            owner_lock,
        )
        .unwrap();

    let coordinator = OwnerSwitchCoordinator::<TestCodec>::new(canonical_root.clone());
    let switch = coordinator
        .begin_owner_switch_while_root_locked(
            root_guard,
            EngineSwitchRevision::new(1),
            previous.clone(),
            old_identity.clone(),
            old_generation,
        )
        .await
        .unwrap();
    assert!(matches!(
        switch.stage(),
        EngineSwitchStage::ClosingOld { .. }
    ));

    OwnerSwitchFile::<TestCodec>::new(canonical_root.clone())
        .publish_owner_proof_sync_reconciled(&terminal_proof(
            &old_identity,
            old_generation,
            CoreGeneration::new(1),
            &old_token,
        ))
        .unwrap();
    remove_local_owner_registration(&canonical_root, &old_token).unwrap();

    let stopped = switch
        .confirm_old_owner_stopped(CoreGeneration::new(1))
        .await
        .unwrap();
    let target_engine = engine("sing-box");
    let candidate_identity = identity(target_engine.clone(), "candidate");
    let target = TestSelection::Engine {
        engine: target_engine.clone(),
        revision: 2,
    };
    let changed = stopped
        .compare_and_set_target(target.clone(), target_engine.clone())
        .await
        .unwrap();
    let binding = changed
        .reserve_new_owner(
            target_engine.clone(),
            candidate_identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();

    let discarded = binding
        .discard_unstarted_candidate(CoreGeneration::new(0))
        .await
        .unwrap();
    assert_eq!(discarded.selection, target);
    assert_eq!(
        discarded.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::SelectionChanged {
                previous: previous.clone(),
                selection: target.clone(),
                owner_identity: old_identity.clone(),
                terminal_generation: CoreGeneration::new(1),
            }
        }
    );
    assert_eq!(
        recover_owner_switch::<TestCodec>(canonical_root.clone(), EngineSwitchRevision::new(2))
            .await
            .unwrap(),
        EngineSwitchRecovery::ResumeSelectionChanged {
            previous: previous.clone(),
            selection: target.clone(),
            owner_identity: old_identity.clone(),
            terminal_generation: CoreGeneration::new(1),
        }
    );

    let recovered = recover_old_owner_after_terminal::<TestCodec>(
        canonical_root.clone(),
        EngineSwitchRevision::new(2),
        CoreGeneration::new(1),
    )
    .await
    .unwrap();
    let retried = recovered
        .compare_and_set_target(target.clone(), target_engine.clone())
        .await
        .unwrap();
    let retried_binding = retried
        .reserve_new_owner(
            target_engine.clone(),
            candidate_identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    retried_binding
        .finalize(candidate_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    assert_eq!(
        switch_snapshot::<TestCodec>(
            &OwnerSwitchFile::<TestCodec>::new(canonical_root)
                .read_active_profile_record_sync()
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        EngineSwitchSnapshot {
            revision: EngineSwitchRevision::new(2),
            selection: target,
            owner: EngineOwnerState::Bound {
                identity: candidate_identity,
                generation: CoreGeneration::new(0),
            },
        }
    );
}
