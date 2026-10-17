use super::*;
use tokio::task::yield_now;
use tokio::time::timeout;

#[tokio::test]
async fn retry_after_owner_close_selects_the_typed_target_when_no_switch_journal_remains() {
    let refs = Arc::new(MemoryReferenceStore::new(reference("mihomo", "main", 4)));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "recovered-instance", events);
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let target = ActiveProfileReferenceTarget::selected(
        engine_id_value("sing-box"),
        ProfileId::new("primary").unwrap(),
    );
    let previous = running_owner_snapshot("mihomo", "closed-old-instance", 5);
    let terminal = terminal_owner_snapshot(&previous);

    let selected = selector
        .resume_owner_switch_for_profile(target.clone(), &prepared, &terminal)
        .await
        .expect("closed prior owner retries through unowned target selection");

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(5),
        )
    );
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::NoPriorOwner
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    drop(selected);

    let selected = selector
        .resume_owner_switch_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared,
            &terminal,
        )
        .await
        .expect("retry a durable BindingNew reservation");
    assert_eq!(counts.created.load(Ordering::SeqCst), 2);
    let committed = selected
        .abort_before_core_application()
        .await
        .expect("release recovered startup candidate");
    assert!(matches!(
        committed,
        ActiveProfileReferenceState::Selected(reference)
            if reference.engine_id == engine_id_value("sing-box")
                && reference.profile_id == ProfileId::new("primary").unwrap()
    ));
}

#[tokio::test]
async fn retry_resumes_closing_old_and_binding_new_owner_journal_stages() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 4),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let (provider, counts) = FakeProvider::new(
        "sing-box",
        "recovered-journal-instance",
        Arc::clone(&events),
    );
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "old-journal-owner", 5);
    let expected_terminal = terminal_owner_snapshot(&before);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(expected_terminal),
        Arc::clone(&events),
    )
    .await;
    let identity = before.identity.clone().expect("old owner identity");
    let switch_lease = refs
        .begin_owner_switch(
            EngineSwitchRevision::new(4),
            reference("mihomo", "main", 4),
            identity,
            CoreGeneration::new(0),
        )
        .await
        .unwrap();
    let terminal = owner.terminate_owner().await.unwrap();
    drop(switch_lease);

    let selected = selector
        .resume_owner_switch_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared,
            &terminal,
        )
        .await
        .expect("recover the persisted ClosingOld phase and resume");

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(5),
        )
    );
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(terminal.generation),
        }
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    selected
        .abort_before_core_application()
        .await
        .expect("release recovered candidate before Core actor construction");
    assert_eq!(*refs.transition.lock().expect("switch transition"), None);
}

#[tokio::test]
async fn retry_resumes_a_reference_changed_owner_journal_with_the_committed_selection() {
    let previous = reference("mihomo", "main", 4);
    let selection = reference("sing-box", "primary", 5);
    let old_identity = CoreIdentity::new(
        engine_id_value("mihomo"),
        CoreInstanceId::new("old-reference-changed-owner").unwrap(),
        None,
    );
    let refs = Arc::new(MemoryReferenceStore::new(selection.clone()));
    *refs.transition.lock().expect("switch transition") = Some(FakeSwitch {
        stage: EngineSwitchStage::SelectionChanged {
            previous,
            selection: selection.clone(),
            owner_identity: old_identity.clone(),
            terminal_generation: CoreGeneration::new(5),
        },
        owner_generation: CoreGeneration::new(5),
    });
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let (provider, counts) = FakeProvider::new(
        "sing-box",
        "reference-changed-retry-instance",
        Arc::new(Mutex::new(Vec::new())),
    );
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let target = ActiveProfileReferenceTarget::selected(
        engine_id_value("sing-box"),
        ProfileId::new("primary").unwrap(),
    );
    let terminal = CoreLifecycleSnapshot {
        identity: Some(old_identity.clone()),
        lifecycle: CoreLifecycle::Stopped,
        generation: 5,
        session_token: None,
        revision: 6,
        failure: None,
    };

    let selected = selector
        .resume_owner_switch_for_profile(target, &prepared, &terminal)
        .await
        .expect("resume the committed target and reserve its owner");

    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: old_identity,
            generation: CoreGeneration::new(5),
        }
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    selected
        .abort_before_core_application()
        .await
        .expect("release recovered candidate before actor construction");
}

#[tokio::test]
async fn retry_rejects_a_binding_new_candidate_without_exact_terminal_evidence() {
    let selection = reference("sing-box", "primary", 5);
    let refs = Arc::new(MemoryReferenceStore::new(selection.clone()));
    refs.reject_candidate_terminal();
    *refs.transition.lock().expect("switch transition") = Some(FakeSwitch {
        stage: EngineSwitchStage::BindingNew {
            selection: selection.clone(),
            engine_id: engine_id_value("sing-box"),
        },
        owner_generation: CoreGeneration::new(5),
    });
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let prepared = load_prepared_profile_document(store.as_ref()).await;
    let (provider, counts) = FakeProvider::new(
        "sing-box",
        "candidate-evidence-instance",
        Arc::new(Mutex::new(Vec::new())),
    );
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let target = ActiveProfileReferenceTarget::selected(
        engine_id_value("sing-box"),
        ProfileId::new("primary").unwrap(),
    );
    let terminal = CoreLifecycleSnapshot {
        identity: Some(CoreIdentity::new(
            engine_id_value("mihomo"),
            CoreInstanceId::new("old-candidate-owner").unwrap(),
            None,
        )),
        lifecycle: CoreLifecycle::Stopped,
        generation: 5,
        session_token: None,
        revision: 6,
        failure: None,
    };

    let failure = match selector
        .resume_owner_switch_for_profile(target, &prepared, &terminal)
        .await
    {
        Err(failure) => failure,
        Ok(_) => panic!("candidate recovery must require its own terminal evidence"),
    };

    assert!(matches!(failure, PortError::Rejected(_)));
    assert_eq!(refs.read().await.unwrap(), selection);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn deleted_target_after_preflight_is_revalidated_before_the_durable_reservation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let source_reference = reference("mihomo", "main", 7);
    let target_deleted = Arc::new(AtomicBool::new(false));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        source_reference.clone(),
        Arc::clone(&events),
    ));
    let profile_store = Arc::new(
        FakeProfileStore::new("sing-box", "sing-box", "primary")
            .with_deletion_flag(Arc::clone(&target_deleted)),
    );
    let store: Arc<dyn ProfileStore> = profile_store.clone();
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let provider = provider.deleting_target_on_dispatch(Arc::clone(&target_deleted));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 12);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(result, EngineSwitchFailurePhase::BeforeReservation),
        PortError::Rejected(_)
    ));
    // Preflight, the transaction validation and the candidate preparation all
    // re-read the exact target document.
    assert_eq!(profile_store.workspace_reads.load(Ordering::SeqCst), 3);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(refs.read().await.unwrap(), source_reference);
    assert_eq!(
        *refs.owner.lock().expect("owner state"),
        FakeOwnerAdmission::Bound {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(0),
        }
    );
    assert_eq!(*refs.transition.lock().expect("switch state"), None);
    // The candidate was dispatched (that is what deleted the target) and then
    // rejected before any durable reservation existed.
    assert_eq!(
        *events.lock().expect("event lock"),
        vec!["target-deleted-before-reservation", "provider:sing-box",]
    );
}

#[tokio::test]
async fn active_reference_compare_and_set_rejects_a_stale_revision_without_write() {
    let store = MemoryReferenceStore::new(reference("mihomo", "main", 4));
    let before = store.read().await.unwrap();

    let error = store
        .compare_and_set(
            ActiveProfileReferenceRevision::new(3),
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            ActiveProfileReferenceWriteAuthorization::Unowned,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(store.read().await.unwrap(), before);
}

#[tokio::test]
async fn selected_candidate_holds_revision_lease_against_concurrent_switches() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 2,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (sing_box, counts) = FakeProvider::new("sing-box", "singbox-instance", events);
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(sing_box)],
        supported_capabilities(),
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .unwrap();
    let write_store = Arc::clone(&refs);
    let write = tokio::spawn(async move {
        write_store
            .compare_and_set(
                ActiveProfileReferenceRevision::new(2),
                ActiveProfileReferenceTarget::selected(
                    engine_id_value("mihomo"),
                    ProfileId::new("other").unwrap(),
                ),
                ActiveProfileReferenceWriteAuthorization::Unowned,
            )
            .await
    });
    assert!(
        timeout(Duration::from_millis(10), async {
            let _ = &write;
            yield_now().await;
            assert!(!write.is_finished());
        })
        .await
        .is_ok()
    );
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    drop(selected);
    let updated = write.await.unwrap().unwrap();
    assert_eq!(updated.revision(), ActiveProfileReferenceRevision::new(3));
}

#[tokio::test]
async fn profile_document_engine_and_id_must_match_the_global_reference() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 1,
    )));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "mihomo", "primary"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", events);
    let selector = selector(
        refs,
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );

    let result = selector.select_backend(&PreviousCoreOwner::Unowned).await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn reference_selects_exactly_one_registered_provider_and_never_reads_namespace_current() {
    let refs = Arc::new(MemoryReferenceStore::new(reference(
        "sing-box", "primary", 7,
    )));
    let store = Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let store_as_port: Arc<dyn ProfileStore> = store.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let (mihomo, mihomo_counts) =
        FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
    let (sing_box, sing_box_counts) =
        FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        refs,
        vec![store_as_port],
        vec![Arc::new(mihomo), Arc::new(sing_box)],
        supported_capabilities(),
    );

    let selected = selector
        .select_backend(&PreviousCoreOwner::Unowned)
        .await
        .unwrap();

    assert_eq!(selected.reference.engine_id, engine_id_value("sing-box"));
    assert_eq!(
        selected.reference.profile_id,
        ProfileId::new("primary").unwrap()
    );
    assert_eq!(
        selected.reference.revision,
        ActiveProfileReferenceRevision::new(7)
    );
    assert_eq!(
        selected.document.identity.engine_id,
        engine_id_value("sing-box")
    );
    assert_eq!(selected.core_identity.engine, engine_id_value("sing-box"));
    assert_eq!(selected.core_identity.instance.as_str(), "singbox-instance");
    assert_eq!(selected.initial_generation, CoreGeneration::new(0));
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::NoPriorOwner
    );
    assert_eq!(mihomo_counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(sing_box_counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(store.current_reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cross_engine_switch_uses_stable_binding_generation_after_lifecycle_advances() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 7),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 12);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    assert_eq!(owner.generation(), 12);
    assert_eq!(
        owner
            .active_profile_reference_write_authorization()
            .unwrap(),
        ActiveProfileReferenceWriteAuthorization::BoundOwner {
            identity: before.identity.clone().unwrap(),
            generation: CoreGeneration::new(0),
        }
    );
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await
        .unwrap();

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(8),
        )
    );
    assert_eq!(selected.initial_generation, CoreGeneration::new(0));
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(12),
        }
    );
    let application = CoreApplication::new(
        selected.backend.process(),
        selected.backend.readiness(),
        Arc::new(SelectorTestRuntime),
    );
    selected
        .install_owner_binding_for_application(&application)
        .await
        .unwrap();
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 8)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "provider:sing-box",
            "switch-reserved",
            "old-owner-close",
            "close-root-read",
            "ordinary-cas-blocked",
            "ordinary-provider-blocked",
            "old-owner-released",
            "old-owner-confirmed",
            "reference-cas",
            "new-owner-reserved",
            "new-owner-bound",
        ]
    );
    application.close().await.unwrap();
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn same_engine_profile_replacement_uses_the_same_durable_owner_transaction() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("sing-box", "previous", 7),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) =
        FakeProvider::new("sing-box", "singbox-next-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("sing-box", "singbox-old-instance", 12);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        Arc::clone(&events),
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let selected = selector
        .replace_owner_for_profile(
            ActiveProfileReferenceTarget::selected(
                engine_id_value("sing-box"),
                ProfileId::new("primary").unwrap(),
            ),
            &prepared_target_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await
        .unwrap();

    assert_eq!(
        selected.reference,
        ActiveProfileReference::new(
            engine_id_value("sing-box"),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(8),
        )
    );
    assert_eq!(
        selected.previous_owner,
        PreviousCoreDisposition::Stopped {
            identity: before.identity.unwrap(),
            generation: CoreGeneration::new(12),
        }
    );
    let application = CoreApplication::new(
        selected.backend.process(),
        selected.backend.readiness(),
        Arc::new(SelectorTestRuntime),
    );
    selected
        .install_owner_binding_for_application(&application)
        .await
        .unwrap();
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 8)
    );
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(
        *events.lock().unwrap(),
        vec![
            "provider:sing-box",
            "switch-reserved",
            "old-owner-close",
            "close-root-read",
            "ordinary-cas-blocked",
            "ordinary-provider-blocked",
            "old-owner-released",
            "old-owner-confirmed",
            "reference-cas",
            "new-owner-reserved",
            "new-owner-bound",
        ]
    );
    application.close().await.unwrap();
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}
