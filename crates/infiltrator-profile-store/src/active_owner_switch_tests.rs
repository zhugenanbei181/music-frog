//! Real-store coverage for the neutral owner-switch journal.

use crate::active_owner_switch::stage_mapping::{
    active_reference_revision, engine_switch_revision,
};
use crate::active_reference_file::ActiveProfileReferenceFile;
use crate::store::EngineProfileStore;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use lurker_core_contract::engine_switch::{
    EngineOwnerState, EngineSwitchRecovery, EngineSwitchStage,
};
use lurker_core_contract::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::{CoreLifecycle, CoreLifecycleSnapshot};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_document_identity::{
    ProfileFormatId, ProfileId, ProfileSchemaVersion,
};
use lurker_core_ports::engine_switch::EngineSwitchJournal;
use lurker_core_ports::error::PortError;
use lurker_core_ports::profile_document_codec::{
    ProfileDocumentCodecPort, ProfileDocumentCodecScope,
};
use lurker_profile_store::owner_switch::owner::ActiveCoreOwnerProof;
use lurker_profile_store::store::ProfileDocumentExtension;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

struct TestCodec(ProfileDocumentCodecScope);

impl ProfileDocumentCodecPort for TestCodec {
    fn scope(&self) -> ProfileDocumentCodecScope {
        self.0.clone()
    }

    fn inspect(&self, _document: &[u8]) -> Result<DocumentValidation, PortError> {
        Ok(DocumentValidation::Valid)
    }
}

#[derive(Clone, Default)]
struct MemorySecureStore {
    values: Arc<Mutex<HashMap<(String, String), String>>>,
}

#[async_trait::async_trait]
impl SecureStore for MemorySecureStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, PortError> {
        Ok(self
            .values
            .lock()
            .expect("secure store lock")
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn set(&self, namespace: &str, key: &str, value: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .expect("secure store lock")
            .insert((namespace.to_owned(), key.to_owned()), value.to_owned());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<(), PortError> {
        self.values
            .lock()
            .expect("secure store lock")
            .remove(&(namespace.to_owned(), key.to_owned()));
        Ok(())
    }
}

fn store(directory: &TempDir) -> EngineProfileStore<MemorySecureStore> {
    let engine = EngineId::mihomo();
    let codec = Arc::new(TestCodec(ProfileDocumentCodecScope {
        engine_id: engine.clone(),
        format_id: ProfileFormatId::new("mihomo-yaml").unwrap(),
        schema_version: ProfileSchemaVersion::new(1).unwrap(),
    }));
    EngineProfileStore::new(
        engine,
        directory.path().to_path_buf(),
        codec,
        ProfileDocumentExtension::Yaml,
        MemorySecureStore::default(),
    )
    .unwrap()
}

fn identity(instance: &str) -> CoreIdentity {
    engine_identity(EngineId::mihomo(), instance)
}

fn engine_identity(engine: EngineId, instance: &str) -> CoreIdentity {
    CoreIdentity::new(engine, CoreInstanceId::new(instance).unwrap(), None)
}

fn terminal(identity: CoreIdentity, generation: CoreGeneration) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: Some(identity),
        lifecycle: CoreLifecycle::Stopped,
        generation: generation.value(),
        session_token: None,
        revision: 1,
        failure: None,
    }
}

fn selected_reference(store: &Path) -> ActiveProfileReferenceState {
    let record: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(store.join("active-profile-reference.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(record["selection"].clone()).unwrap()
}

async fn selected_store(directory: &TempDir) -> (EngineProfileStore<MemorySecureStore>, ProfileId) {
    let store = store(directory);
    let profile = ProfileId::new("primary").unwrap();
    store
        .write_profile_document_locked(&profile, b"mode: rule\n")
        .await
        .unwrap();
    store
        .compare_and_set(
            ActiveProfileReferenceRevision::new(0),
            ActiveProfileReferenceTarget::selected(EngineId::mihomo(), profile.clone()),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();
    (store, profile)
}

#[tokio::test]
async fn journal_walks_every_persisted_stage_and_keeps_the_wire_format() {
    let directory = TempDir::new().unwrap();
    let (store, _profile) = selected_store(&directory).await;
    let owner_identity = identity("journal-owner");
    let owner = store
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(owner_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    let initial = store.snapshot().await.unwrap();
    assert_eq!(
        initial.revision,
        engine_switch_revision(initial.selection.revision())
    );
    assert!(matches!(
        initial.owner,
        EngineOwnerState::Bound {
            generation,
            ..
        } if generation == CoreGeneration::new(0)
    ));

    let switch = store
        .begin_owner_switch(
            initial.revision,
            initial.selection.clone(),
            owner_identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    assert!(matches!(
        switch.stage(),
        EngineSwitchStage::ClosingOld { .. }
    ));
    assert_wire_stage(directory.path(), "closing_old");

    owner
        .release_after_terminal(&terminal(owner_identity.clone(), CoreGeneration::new(3)))
        .await
        .unwrap();
    let recovery = store.recover_owner_switch(initial.revision).await.unwrap();
    assert!(matches!(
        recovery,
        EngineSwitchRecovery::ResumeClosingOld { .. }
    ));

    let stopped = store
        .recover_old_owner_after_terminal(initial.revision, CoreGeneration::new(3))
        .await
        .unwrap();
    assert!(matches!(
        store.recover_owner_switch(initial.revision).await.unwrap(),
        EngineSwitchRecovery::ResumeOldStopped { .. }
    ));
    assert!(matches!(
        stopped.stage(),
        EngineSwitchStage::OldStopped { .. }
    ));
    assert_wire_stage(directory.path(), "old_stopped");

    let target_engine = EngineId::sing_box();
    let changed = stopped
        .compare_and_set_target(
            ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                target_engine.clone(),
                ProfileId::new("replacement").unwrap(),
                ActiveProfileReferenceRevision::new(2),
            )),
            target_engine.clone(),
        )
        .await
        .unwrap();
    assert!(matches!(
        changed.stage(),
        EngineSwitchStage::SelectionChanged { .. }
    ));
    assert_wire_stage(directory.path(), "reference_changed");
    assert_eq!(
        selected_reference(directory.path()).revision(),
        ActiveProfileReferenceRevision::new(2)
    );

    // A recovered selection-changed stage repeats the same CAS idempotently:
    // the revision must not advance a second time.
    let published = store.snapshot().await.unwrap();
    assert_eq!(
        published.revision,
        engine_switch_revision(ActiveProfileReferenceRevision::new(2))
    );
    let recovered = store
        .recover_old_owner_after_terminal(published.revision, CoreGeneration::new(3))
        .await
        .unwrap();
    assert!(matches!(
        recovered.stage(),
        EngineSwitchStage::SelectionChanged { .. }
    ));
    let repeated = recovered
        .compare_and_set_target(
            ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                target_engine.clone(),
                ProfileId::new("replacement").unwrap(),
                ActiveProfileReferenceRevision::new(2),
            )),
            target_engine.clone(),
        )
        .await
        .unwrap();
    assert_eq!(
        selected_reference(directory.path()).revision(),
        ActiveProfileReferenceRevision::new(2)
    );
    drop(repeated);

    let binding = changed
        .reserve_new_owner(
            target_engine.clone(),
            engine_identity(EngineId::sing_box(), "journal-candidate"),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    assert!(matches!(
        binding.stage(),
        EngineSwitchStage::BindingNew { .. }
    ));
    assert_wire_stage(directory.path(), "binding_new");
    let wire = fs::read_to_string(directory.path().join("active-profile-reference.json")).unwrap();
    assert!(wire.contains("\"token\""), "binding stage lost its token");
    assert!(
        wire.contains("\"terminal_generation\""),
        "binding stage lost its terminal generation"
    );

    let recovery = store
        .recover_owner_switch(published.revision)
        .await
        .unwrap();
    assert!(matches!(
        recovery,
        EngineSwitchRecovery::ResumeBindingNew { .. }
    ));
    let cleared = store
        .recover_candidate_after_terminal(published.revision, CoreGeneration::new(0))
        .await
        .unwrap();
    assert_eq!(cleared.owner, EngineOwnerState::Unowned);
    assert_eq!(cleared.selection, selected_reference(directory.path()));
    assert!(matches!(
        store
            .recover_owner_switch(published.revision)
            .await
            .unwrap(),
        EngineSwitchRecovery::NoPendingSwitch
    ));
}

#[tokio::test]
async fn stale_revisions_and_leases_are_rejected_before_any_write() {
    let directory = TempDir::new().unwrap();
    let (store, _profile) = selected_store(&directory).await;
    let owner_identity = identity("stale-owner");
    let owner = store
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(owner_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    let snapshot = store.snapshot().await.unwrap();
    let before =
        fs::read_to_string(directory.path().join("active-profile-reference.json")).unwrap();
    let stale = engine_switch_revision(ActiveProfileReferenceRevision::new(0));

    assert!(matches!(
        store
            .begin_owner_switch(
                stale,
                snapshot.selection.clone(),
                owner_identity.clone(),
                CoreGeneration::new(0)
            )
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        store.recover_owner_switch(stale).await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        store
            .recover_old_owner_after_terminal(stale, CoreGeneration::new(0))
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        store
            .recover_candidate_after_terminal(stale, CoreGeneration::new(0))
            .await,
        Err(PortError::Rejected(_))
    ));
    assert_eq!(
        fs::read_to_string(directory.path().join("active-profile-reference.json")).unwrap(),
        before,
        "a stale revision wrote to the persisted record"
    );

    // A lease issued for the closing stage is stale once the journal advanced.
    let switch = store
        .begin_owner_switch(
            snapshot.revision,
            snapshot.selection.clone(),
            owner_identity,
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    owner
        .release_after_terminal(&terminal(identity("stale-owner"), CoreGeneration::new(1)))
        .await
        .unwrap();
    store
        .recover_old_owner_after_terminal(snapshot.revision, CoreGeneration::new(1))
        .await
        .unwrap();
    assert!(matches!(
        switch.cancel_before_old_owner_close().await,
        Err(PortError::Rejected(_))
    ));
    assert_eq!(
        active_reference_revision(snapshot.revision),
        ActiveProfileReferenceRevision::new(1)
    );
}

fn assert_wire_stage(config_root: &Path, expected: &str) {
    let text = fs::read_to_string(config_root.join("active-profile-reference.json")).unwrap();
    let record: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        record["owner"]["owner"]["stage"]["stage"], expected,
        "persisted stage drift: {text}"
    );
    assert_eq!(record["owner"]["state"], "switching");
}

/// PUB-003.2.2 option A: a live discard rewinds the journal to the retryable
/// `SelectionChanged` stage instead of inventing an unowned owner. The old
/// owner's exact terminal proof is restored, and the caller can reserve the
/// candidate again through the recovered stopped lease.
#[tokio::test]
async fn discarded_candidate_rewinds_to_selection_changed_and_can_be_retried() {
    let directory = TempDir::new().unwrap();
    let (store, _profile) = selected_store(&directory).await;
    let owner_identity = identity("discard-owner");
    let owner = store
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(owner_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    let initial = store.snapshot().await.unwrap();
    let switch = store
        .begin_owner_switch(
            initial.revision,
            initial.selection.clone(),
            owner_identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    assert!(matches!(
        switch.stage(),
        EngineSwitchStage::ClosingOld { .. }
    ));
    owner
        .release_after_terminal(&terminal(owner_identity.clone(), CoreGeneration::new(1)))
        .await
        .unwrap();
    let stopped = store
        .recover_old_owner_after_terminal(initial.revision, CoreGeneration::new(1))
        .await
        .unwrap();
    let target_engine = EngineId::sing_box();
    let target = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        target_engine.clone(),
        ProfileId::new("replacement").unwrap(),
        ActiveProfileReferenceRevision::new(2),
    ));
    let changed = stopped
        .compare_and_set_target(target.clone(), target_engine.clone())
        .await
        .unwrap();
    let candidate = engine_identity(EngineId::sing_box(), "discard-candidate");
    let binding = changed
        .reserve_new_owner(
            target_engine.clone(),
            candidate.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    assert_wire_stage(directory.path(), "binding_new");

    let discarded = binding
        .discard_unstarted_candidate(CoreGeneration::new(0))
        .await
        .unwrap();

    assert_eq!(discarded.selection, target);
    assert!(matches!(
        &discarded.owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::SelectionChanged {
                previous,
                selection,
                terminal_generation,
                ..
            }
        } if previous == &initial.selection
            && selection == &target
            && *terminal_generation == CoreGeneration::new(1)
    ));
    assert_wire_stage(directory.path(), "reference_changed");
    assert!(matches!(
        store
            .recover_owner_switch(discarded.revision)
            .await
            .unwrap(),
        EngineSwitchRecovery::ResumeSelectionChanged { .. }
    ));
    // The old owner proof is restored exactly, not replaced by the candidate.
    let proof = ActiveProfileReferenceFile::new(directory.path())
        .read_owner_proof_sync()
        .unwrap();
    assert!(matches!(
        proof.state,
        ActiveCoreOwnerProof::Terminal {
            terminal_generation,
            ..
        } if terminal_generation == CoreGeneration::new(1)
    ));

    let recovered = store
        .recover_old_owner_after_terminal(discarded.revision, CoreGeneration::new(1))
        .await
        .unwrap();
    let retried = recovered
        .compare_and_set_target(target.clone(), target_engine.clone())
        .await
        .unwrap();
    let retried_binding = retried
        .reserve_new_owner(target_engine, candidate, CoreGeneration::new(0))
        .await
        .unwrap();
    assert!(matches!(
        retried_binding.stage(),
        EngineSwitchStage::BindingNew { .. }
    ));
    assert_wire_stage(directory.path(), "binding_new");
}
