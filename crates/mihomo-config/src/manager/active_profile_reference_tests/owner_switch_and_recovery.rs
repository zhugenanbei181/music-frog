use super::*;
use lurker_profile_store::owner_switch::registry;

#[tokio::test]
async fn active_reference_cas_updates_only_the_global_reference() {
    let directory = TempDir::new().unwrap();
    let manager = manager(&directory);
    manager
        .save_inactive_profile("default", "mixed-port: 7890\n")
        .await
        .unwrap();
    manager
        .save_inactive_profile("other", "mixed-port: 7891\n")
        .await
        .unwrap();
    let initial = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let legacy_pointer = manager.get_current_unlocked().await.unwrap();

    let selected = manager
        .engine_profile_store()
        .compare_and_set(
            initial.revision(),
            ActiveProfileReferenceTarget::selected(
                crate::engine_id(),
                ProfileId::new("other").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap();

    assert!(matches!(
        &selected,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == crate::engine_id()
                && reference.profile_id.as_str() == "other"
                && reference.revision.value() == 2
    ));
    assert_eq!(
        manager.get_current_unlocked().await.unwrap(),
        legacy_pointer
    );
    assert_eq!(
        manager.migrate_legacy_mihomo_selection().await.unwrap(),
        selected
    );
}

#[tokio::test]
async fn selection_lease_blocks_cas_until_candidate_owner_is_bound() {
    let directory = TempDir::new().unwrap();
    let manager = Arc::new(manager(&directory));
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let lease = manager
        .engine_profile_store()
        .acquire_selection_lease()
        .await
        .unwrap();
    assert_eq!(lease.state(), &selected);

    let writer = Arc::clone(&manager);
    let expected = selected.revision();
    let pending = tokio::spawn(async move {
        writer
            .engine_profile_store()
            .compare_and_set(
                expected,
                ActiveProfileReferenceTarget::Unselected,
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
    });
    yield_now().await;
    assert!(!pending.is_finished());

    drop(lease);
    let cleared = pending.await.unwrap().unwrap();
    assert_eq!(cleared.revision().value(), 2);
}

#[tokio::test]
async fn owner_binding_allows_same_engine_profile_updates_but_fences_provider_switches() {
    let directory = TempDir::new().unwrap();
    let manager = Arc::new(manager(&directory));
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();

    let lease = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap();
    let identity = CoreIdentity::new(
        crate::engine_id(),
        CoreInstanceId::new("mihomo-owner").unwrap(),
        None,
    );
    let owner_identity = identity.clone();
    let owner = lease
        .into_owner_binding(identity, CoreGeneration::new(1))
        .await
        .unwrap();

    let same_engine_update = manager
        .engine_profile_store()
        .compare_and_set(
            selected.revision(),
            ActiveProfileReferenceTarget::selected(
                crate::engine_id(),
                ProfileId::new("default").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::BoundOwner {
                identity: owner_identity.clone(),
                generation: CoreGeneration::new(1),
            },
        )
        .await
        .unwrap();
    assert_eq!(same_engine_update.revision().value(), 2);

    let foreign_engine = EngineId::new("sing-box").unwrap();
    let denied = manager
        .engine_profile_store()
        .compare_and_set(
            same_engine_update.revision(),
            ActiveProfileReferenceTarget::selected(
                foreign_engine,
                ProfileId::new("default").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::BoundOwner {
                identity: owner_identity.clone(),
                generation: CoreGeneration::new(1),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(denied, PortError::Rejected(_)));

    match manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
    {
        Err(PortError::Rejected(_)) => {}
        Ok(_) => panic!("a second provider lease bypassed the active owner"),
        Err(other) => panic!("unexpected provider lease failure: {other}"),
    }

    let terminal = CoreLifecycleSnapshot {
        identity: Some(owner_identity),
        lifecycle: CoreLifecycle::Stopped,
        generation: 1,
        session_token: None,
        revision: 1,
        failure: None,
    };
    owner.release_after_terminal(&terminal).await.unwrap();
    owner.release_after_terminal(&terminal).await.unwrap();
    drop(owner);
    let next_provider = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap();
    assert_eq!(next_provider.state(), &same_engine_update);
}

#[tokio::test]
async fn persisted_owner_release_and_recovery_ignore_runtime_version_changes() {
    let directory = TempDir::new().unwrap();
    let manager = manager(&directory);
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let bound_identity = identity(crate::engine_id().as_str(), "version-refresh-owner");
    let owner = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(bound_identity.clone(), CoreGeneration::new(1))
        .await
        .unwrap();
    let session_context = CoreProcessStartContext::managed(
        bound_identity.clone(),
        CoreGeneration::new(2),
        SessionToken::new(52),
    );
    owner.record_start_context(&session_context).await.unwrap();

    let mut ready_identity = bound_identity.clone();
    ready_identity.version = Some(CoreVersion::new("1.19.12").unwrap());
    let ready_terminal = stopped_snapshot(ready_identity.clone(), 2);
    owner.release_after_terminal(&ready_terminal).await.unwrap();

    // A later observation can refresh capability evidence again while
    // the persisted owner proof remains bound to the same engine instance.
    ready_identity.version = Some(CoreVersion::new("1.19.13").unwrap());
    let refreshed_terminal = stopped_snapshot(ready_identity, 2);
    owner
        .release_after_terminal(&refreshed_terminal)
        .await
        .unwrap();
    drop(owner);

    assert_eq!(
        manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::ManagedChildExited {
                context: session_context,
                terminal: refreshed_terminal,
            })
            .await
            .unwrap(),
        OwnerRecoveryOutcome::Recovered(selected.clone())
    );
    let provider_lease = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap();
    assert_eq!(provider_lease.state(), &selected);
}

#[tokio::test]
async fn never_started_recovers_owner_when_start_context_was_recorded_without_spawning() {
    let directory = TempDir::new().unwrap();
    let manager = manager(&directory);
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let bound_identity = identity(crate::engine_id().as_str(), "unspawned-owner");
    let owner = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(bound_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    let session_context = CoreProcessStartContext::managed(
        bound_identity.clone(),
        CoreGeneration::new(1),
        SessionToken::new(77),
    );
    owner.record_start_context(&session_context).await.unwrap();
    registry::lock_local_owner_registry()
        .unwrap()
        .remove_config_root(&manager.config_root());
    drop(owner);

    let terminal = stopped_snapshot(bound_identity.clone(), 0);
    assert_eq!(
        manager
            .engine_profile_store()
            .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::NeverStarted { terminal })
            .await
            .unwrap(),
        OwnerRecoveryOutcome::Recovered(selected.clone())
    );
    let provider_lease = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap();
    assert_eq!(provider_lease.state(), &selected);
}

#[tokio::test]
async fn never_started_terminal_evidence_is_a_noop_without_a_persisted_owner() {
    let directory = TempDir::new().unwrap();
    let manager = manager(&directory);

    let outcome = manager
        .engine_profile_store()
        .recover_bound_owner_after_terminal(OwnerRecoveryEvidence::NeverStarted {
            terminal: stopped_snapshot(identity("mihomo", "android-active"), 0),
        })
        .await
        .unwrap();

    assert_eq!(outcome, OwnerRecoveryOutcome::NoRecoveryRequired);
}

#[tokio::test]
async fn terminal_release_never_waits_for_the_root_selection_lock() {
    let directory = TempDir::new().unwrap();
    let manager = Arc::new(manager(&directory));
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let old_identity = identity(crate::engine_id().as_str(), "close-cas-owner");
    let owner = manager
        .engine_profile_store()
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
        .await
        .unwrap();
    let terminal = stopped_snapshot(old_identity, 1);

    // A normal selector may hold the root reference lock. Terminal
    // release takes only owner-state, so CoreApplication::close cannot
    // deadlock behind that selector's lease.
    let selection_lease = manager
        .engine_profile_store()
        .acquire_selection_lease()
        .await
        .unwrap();
    let release = tokio::spawn(async move { owner.release_after_terminal(&terminal).await });
    timeout(Duration::from_secs(3), release)
        .await
        .expect("terminal release waited for root selection lock")
        .expect("terminal release task panicked")
        .unwrap();

    let writer = Arc::clone(&manager);
    let pending = tokio::spawn(async move {
        writer
            .engine_profile_store()
            .compare_and_set(
                selected.revision(),
                ActiveProfileReferenceTarget::selected(
                    EngineId::new("sing-box").unwrap(),
                    ProfileId::new("default").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
    });
    yield_now().await;
    assert!(!pending.is_finished());
    drop(selection_lease);
    let changed = pending.await.unwrap().unwrap();
    assert_eq!(changed.revision().value(), 2);
}

#[tokio::test]
async fn owner_switch_requires_terminal_then_cas_and_candidate_binding() {
    let directory = TempDir::new().unwrap();
    let manager = manager(&directory);
    let store = manager.engine_profile_store();
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let old_identity = identity(crate::engine_id().as_str(), "old-owner");
    let owner = store
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
        .await
        .unwrap();
    let previous_revision = EngineSwitchRevision::new(selected.revision().value());
    store
        .begin_owner_switch(
            previous_revision,
            selected.clone(),
            old_identity.clone(),
            CoreGeneration::new(1),
        )
        .await
        .unwrap();
    assert!(matches!(
        store.snapshot().await.unwrap().owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::ClosingOld { .. }
        }
    ));

    let blocked_cas = store
        .compare_and_set(
            selected.revision(),
            ActiveProfileReferenceTarget::selected(
                EngineId::new("sing-box").unwrap(),
                ProfileId::new("default").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await;
    assert!(matches!(blocked_cas, Err(PortError::Rejected(_))));
    assert!(matches!(
        store.acquire_provider_selection_lease().await,
        Err(PortError::Rejected(_))
    ));

    let old_terminal = stopped_snapshot(old_identity.clone(), 1);
    owner.release_after_terminal(&old_terminal).await.unwrap();
    assert!(matches!(
        store.recover_owner_switch(previous_revision).await.unwrap(),
        EngineSwitchRecovery::ResumeClosingOld { .. }
    ));
    let old_stopped = store
        .recover_old_owner_after_terminal(previous_revision, CoreGeneration::new(1))
        .await
        .unwrap();
    assert!(matches!(
        store.snapshot().await.unwrap().owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::OldStopped {
                terminal_generation,
                ..
            }
        } if terminal_generation == CoreGeneration::new(1)
    ));
    let target_engine = EngineId::new("sing-box").unwrap();
    let changed_selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        target_engine.clone(),
        ProfileId::new("default").unwrap(),
        selected.revision().next().unwrap(),
    ));
    let changed_revision = EngineSwitchRevision::new(changed_selection.revision().value());
    let changed = old_stopped
        .compare_and_set_target(changed_selection.clone(), target_engine.clone())
        .await
        .unwrap();
    drop(changed);
    assert!(matches!(
        store.recover_owner_switch(changed_revision).await.unwrap(),
        EngineSwitchRecovery::ResumeSelectionChanged { .. }
    ));
    let changed = store
        .recover_old_owner_after_terminal(changed_revision, CoreGeneration::new(1))
        .await
        .unwrap()
        .compare_and_set_target(changed_selection.clone(), target_engine.clone())
        .await
        .unwrap();
    let candidate_identity = identity(target_engine.as_str(), "new-owner");
    let candidate = changed
        .reserve_new_owner(
            target_engine.clone(),
            candidate_identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    // A started candidate receipt is not the exact never-started terminal:
    // candidate recovery must reject it and keep the durable reservation
    // resumable.
    assert!(matches!(
        store
            .recover_candidate_after_terminal(changed_revision, CoreGeneration::new(1))
            .await,
        Err(PortError::Rejected(_))
    ));
    assert!(matches!(
        store.recover_owner_switch(changed_revision).await.unwrap(),
        EngineSwitchRecovery::ResumeBindingNew { engine_id, .. } if engine_id == target_engine
    ));
    candidate
        .finalize(candidate_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    // The neutral `finalize` returns only the engine-neutral owner fence; the
    // product lease (with terminal release) is rehydrated through adoption.
    let new_owner = store
        .adopt_finalized_owner(candidate_identity.clone(), CoreGeneration::new(0))
        .await
        .unwrap();
    let committed = store.read().await.unwrap();
    assert!(matches!(
        &committed,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == target_engine
                && reference.profile_id.as_str() == "default"
                && reference.revision.value() == 2
    ));
    new_owner
        .release_after_terminal(&stopped_snapshot(candidate_identity, 0))
        .await
        .unwrap();
}

#[tokio::test]
async fn crashed_closing_old_requires_explicit_terminal_evidence_and_can_resume() {
    let directory = TempDir::new().unwrap();
    let manager = manager(&directory);
    let store = manager.engine_profile_store();
    let selected = manager.migrate_legacy_mihomo_selection().await.unwrap();
    let old_identity = identity(crate::engine_id().as_str(), "crashed-old-owner");
    let mut observed_identity = old_identity.clone();
    observed_identity.version = Some(CoreVersion::new("1.19.12").unwrap());
    let owner = store
        .acquire_provider_selection_lease()
        .await
        .unwrap()
        .into_owner_binding(old_identity.clone(), CoreGeneration::new(1))
        .await
        .unwrap();
    let previous_revision = EngineSwitchRevision::new(selected.revision().value());
    // The reservation is durable; the returned lease holds no lock, so the
    // simulated process death below only drops the owner's kernel guard.
    store
        .begin_owner_switch(
            previous_revision,
            selected.clone(),
            observed_identity.clone(),
            CoreGeneration::new(1),
        )
        .await
        .unwrap();

    // Simulate process death: release the kernel owner guard without
    // publishing terminal state. Recovery must ask the platform seam.
    let config_root = fs::canonicalize(manager.config_root()).await.unwrap();
    let record = sync_read_active_profile_record(&config_root).unwrap();
    let owner_token = match record.owner {
        ActiveCoreOwnerState::Switching { owner_token, .. } => owner_token,
        other => panic!("expected durable switch reservation, got {other:?}"),
    };
    remove_local_owner_registration(&config_root, &owner_token).unwrap();
    drop(owner);
    assert!(matches!(
        store.recover_owner_switch(previous_revision).await.unwrap(),
        EngineSwitchRecovery::ResumeClosingOld {
            owner_identity,
            owner_generation,
            ..
        } if owner_identity == old_identity && owner_generation == CoreGeneration::new(1)
    ));
    // A missing process-local owner lock is not terminal evidence: a terminal
    // generation that predates the binding must be rejected instead of
    // silently resuming the switch.
    assert!(matches!(
        store
            .recover_old_owner_after_terminal(previous_revision, CoreGeneration::new(0))
            .await,
        Err(PortError::Rejected(_))
    ));

    let old_terminal = stopped_snapshot(observed_identity, 2);
    let old_stopped = store
        .recover_old_owner_after_terminal(
            previous_revision,
            CoreGeneration::new(old_terminal.generation),
        )
        .await
        .unwrap();
    assert!(matches!(
        store.snapshot().await.unwrap().owner,
        EngineOwnerState::Switching {
            stage: EngineSwitchStage::OldStopped {
                terminal_generation,
                ..
            }
        } if terminal_generation == CoreGeneration::new(2)
    ));
    let target_engine = EngineId::new("sing-box").unwrap();
    let next_revision = selected.revision().next().unwrap();
    let changed_selection = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
        target_engine.clone(),
        ProfileId::new("after-crash").unwrap(),
        next_revision,
    ));
    let changed = old_stopped
        .compare_and_set_target(changed_selection, target_engine.clone())
        .await
        .unwrap();
    let candidate = changed
        .reserve_new_owner(
            target_engine.clone(),
            identity(target_engine.as_str(), "after-crash-candidate"),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    let selected_after_abort = candidate
        .abort_before_core_application()
        .await
        .unwrap()
        .selection;
    assert!(matches!(
        &selected_after_abort,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == target_engine
                && reference.profile_id.as_str() == "after-crash"
                && reference.revision.value() == 2
    ));
    assert!(matches!(
        store
            .recover_owner_switch(EngineSwitchRevision::new(next_revision.value()))
            .await
            .unwrap(),
        EngineSwitchRecovery::NoPendingSwitch
    ));
}
