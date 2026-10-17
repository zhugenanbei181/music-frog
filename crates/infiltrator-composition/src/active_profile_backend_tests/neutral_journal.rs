use super::*;

#[tokio::test]
async fn crash_candidate_recovery_lands_unowned_without_a_live_lease() {
    let selection = reference("sing-box", "primary", 5);
    let refs = Arc::new(MemoryReferenceStore::new(selection.clone()));
    *refs.transition.lock().expect("switch transition") = Some(FakeSwitch {
        stage: EngineSwitchStage::BindingNew {
            selection: selection.clone(),
            engine_id: engine_id_value("sing-box"),
        },
        owner_generation: CoreGeneration::new(0),
    });
    *refs.owner.lock().expect("owner state") = FakeOwnerAdmission::Reserved {
        engine_id: engine_id_value("sing-box"),
    };

    let recovered = refs
        .recover_candidate_after_terminal(EngineSwitchRevision::new(5), CoreGeneration::new(0))
        .await
        .unwrap();

    assert_eq!(recovered.selection, selection);
    assert_eq!(recovered.owner, EngineOwnerState::Unowned);
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn stale_revision_is_rejected_before_any_switch_write() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 3),
        Arc::clone(&events),
    ));
    let before = running_owner_snapshot("mihomo", "old-neutral-owner", 2);
    let terminal = terminal_owner_snapshot(&before);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal.clone()),
        Arc::clone(&events),
    )
    .await;
    let identity = before.identity.clone().expect("old owner identity");

    // Revision 2 is stale: the journal is at the revision of the persisted
    // reference. Nothing may be written and no process may be touched.
    let stale = refs
        .begin_owner_switch(
            EngineSwitchRevision::new(2),
            reference("mihomo", "main", 3),
            identity.clone(),
            CoreGeneration::new(0),
        )
        .await;
    assert!(matches!(stale, Err(PortError::Rejected(_))));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert!(events.lock().unwrap().is_empty());
    assert!(
        refs.recover_owner_switch(EngineSwitchRevision::new(2))
            .await
            .is_err()
    );
    assert!(
        refs.recover_old_owner_after_terminal(EngineSwitchRevision::new(2), CoreGeneration::new(2))
            .await
            .is_err()
    );
    assert!(
        refs.recover_candidate_after_terminal(EngineSwitchRevision::new(2), CoreGeneration::new(0))
            .await
            .is_err()
    );
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 3));
}

#[tokio::test]
async fn a_lease_from_a_superseded_stage_cannot_cancel_or_confirm() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 4),
        Arc::clone(&events),
    ));
    let before = running_owner_snapshot("mihomo", "superseded-owner", 2);
    let terminal = terminal_owner_snapshot(&before);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal.clone()),
        Arc::clone(&events),
    )
    .await;
    let identity = before.identity.clone().expect("old owner identity");
    let lease = refs
        .begin_owner_switch(
            EngineSwitchRevision::new(4),
            reference("mihomo", "main", 4),
            identity.clone(),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    owner.terminate_owner().await.unwrap();

    // Promote the journal through the exact terminal, then confirm through the
    // recovered lease. The original `ClosingOld` lease is now stale.
    let stopped = refs
        .recover_old_owner_after_terminal(
            EngineSwitchRevision::new(4),
            CoreGeneration::new(terminal.generation),
        )
        .await
        .unwrap();
    assert!(matches!(
        lease.cancel_before_old_owner_close().await,
        Err(PortError::Rejected(_))
    ));

    let target_engine = engine_id_value("sing-box");
    let selection = reference("sing-box", "primary", 5);
    let changed = stopped
        .compare_and_set_target(selection.clone(), target_engine.clone())
        .await
        .unwrap();
    let binding = changed
        .reserve_new_owner(
            target_engine.clone(),
            {
                CoreIdentity::new(
                    target_engine.clone(),
                    CoreInstanceId::new("neutral-candidate").unwrap(),
                    None,
                )
            },
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    let rebound = binding
        .finalize(
            CoreIdentity::new(
                target_engine,
                CoreInstanceId::new("neutral-candidate").unwrap(),
                None,
            ),
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    assert_eq!(rebound.generation(), CoreGeneration::new(0));
    assert_eq!(refs.read().await.unwrap(), selection);
    assert_eq!(*refs.transition.lock().unwrap(), None);
}
