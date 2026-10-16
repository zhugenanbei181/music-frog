use crate::engine_backend_factory::EngineBackendFactoryRegistry;
use infiltrator_ports::engine_backend_factory::EngineBackendFactory;
use super::*;

#[tokio::test]
async fn provider_prepared_from_another_document_fails_before_old_owner_close() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let mut prepared_document = load_prepared_profile_document(store.as_ref()).await;
    prepared_document.raw_bytes.push(b' ');
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
            &prepared_document,
            &PreviousCoreOwner::Owned(owner_as_port),
        )
        .await;

    assert!(matches!(
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn mismatching_constructed_application_is_closed_then_explicitly_discarded() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
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
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
    let wrong_identity = CoreIdentity::new(
        engine_id_value("sing-box"),
        CoreInstanceId::new("wrong-instance").unwrap(),
        Some(CoreVersion::new("test-version").unwrap()),
    );
    let application = CoreApplication::new(
        Arc::new(FakeProcess {
            identity: wrong_identity.clone(),
            starts: Arc::clone(&counts),
        }),
        Arc::new(FakeReadiness),
        Arc::new(SelectorTestRuntime),
    );

    let result = selected
        .into_owner_binding_for_application(&application)
        .await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    let terminal = application.snapshot().lifecycle_snapshot();
    assert_eq!(terminal.identity, Some(wrong_identity));
    assert_eq!(terminal.lifecycle, CoreLifecycle::Stopped);
    assert_eq!(terminal.session_token, None);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-discarded")
    );
}

#[tokio::test]
async fn failed_owner_install_releases_only_after_exact_application_terminal() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
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
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
    let application = CoreApplication::new(
        selected.backend.process(),
        selected.backend.readiness(),
        Arc::new(SelectorTestRuntime),
    );
    application.close().await.unwrap();

    let result = selected
        .install_owner_binding_for_application(&application)
        .await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    let terminal = application.snapshot().lifecycle_snapshot();
    assert_eq!(
        terminal.identity.as_ref().unwrap().engine,
        engine_id_value("sing-box")
    );
    assert_eq!(terminal.lifecycle, CoreLifecycle::Stopped);
    assert_eq!(terminal.session_token, None);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn legacy_selector_cannot_close_an_owner_without_the_switch_transaction() {
    let refs = Arc::new(MemoryReferenceStore::new(reference("mihomo", "main", 1)));
    let store: Arc<dyn ProfileStore> = Arc::new(FakeProfileStore::new("mihomo", "mihomo", "main"));
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before.clone(),
        Ok(terminal_owner_snapshot(&before)),
        events,
    )
    .await;
    let owner_as_port: Arc<dyn CoreOwnerTerminationPort> = owner.clone();

    let result = selector
        .select_backend(&PreviousCoreOwner::Owned(owner_as_port))
        .await;

    assert!(matches!(result, Err(PortError::Rejected(_))));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn foreign_target_document_fails_before_close_or_provider_dispatch() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "mihomo", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider)],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn missing_provider_or_target_capability_fails_before_close_or_dispatch() {
    for (capability_state, target_provider_registered) in [
        (CapabilityFixtureState::Available, false),
        (CapabilityFixtureState::TargetUnavailable, true),
        (CapabilityFixtureState::StartUnsupported, true),
    ] {
        let events = Arc::new(Mutex::new(Vec::new()));
        let refs = Arc::new(MemoryReferenceStore::with_events(
            reference("mihomo", "main", 1),
            Arc::clone(&events),
        ));
        let store: Arc<dyn ProfileStore> =
            Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
        let (mihomo, mihomo_counts) =
            FakeProvider::new("mihomo", "mihomo-instance", Arc::clone(&events));
        let mut providers: Vec<Arc<dyn EngineBackendProvider>> = vec![Arc::new(mihomo)];
        let mut sing_box_counts = None;
        if target_provider_registered {
            let (sing_box, counts) =
                FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
            sing_box_counts = Some(counts);
            providers.push(Arc::new(sing_box));
        }
        let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
        let selector = selector(
            Arc::clone(&refs),
            vec![store],
            providers,
            Arc::new(FakeCapabilities {
                state: capability_state,
            }),
        );
        let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
            switch_failure(
                result,
                ActiveProfileBackendSwitchFailurePhase::BeforeReservation
            ),
            PortError::Unsupported { .. }
        ));
        assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
        assert_eq!(mihomo_counts.created.load(Ordering::SeqCst), 0);
        if let Some(counts) = sing_box_counts {
            assert_eq!(counts.created.load(Ordering::SeqCst), 0);
        }
        assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
        assert_eq!(*refs.transition.lock().unwrap(), None);
        assert!(events.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn stale_revision_at_reservation_fails_before_close_or_provider_dispatch() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    refs.reject_next_begin_as_stale();
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
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::BeforeReservation
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 0);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*events.lock().unwrap(), Vec::<String>::new());
}

#[tokio::test]
async fn terminal_receipt_must_match_exact_preclose_generation_before_cas() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
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
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let mut wrong_terminal = terminal_owner_snapshot(&before);
    wrong_terminal.generation += 1;
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before,
        Ok(wrong_terminal),
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert!(matches!(
        refs.transition.lock().unwrap().as_ref(),
        Some(ActiveProfileOwnerSwitchState::ClosingOld { .. })
    ));
}

#[tokio::test]
async fn close_failure_keeps_closing_reservation_and_never_dispatches_target() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
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
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
    let owner = bound_fake_owner(
        Arc::clone(&refs),
        before,
        Err(PortError::Failed("fake old owner failed to close".into())),
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OwnerTerminationUnconfirmed
        ),
        PortError::Failed(_)
    ));
    assert_eq!(owner.terminations.load(Ordering::SeqCst), 1);
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(refs.read().await.unwrap(), reference("mihomo", "main", 1));
    assert!(matches!(
        refs.transition.lock().unwrap().as_ref(),
        Some(ActiveProfileOwnerSwitchState::ClosingOld { .. })
    ));
    assert!(matches!(
        *refs.owner.lock().unwrap(),
        FakeOwnerAdmission::Bound { .. }
    ));
}

#[tokio::test]
async fn failed_new_owner_reservation_has_zero_provider_dispatch() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    refs.reject_next_reservation();
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
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed
        ),
        PortError::Rejected(_)
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert!(matches!(
        refs.transition.lock().unwrap().as_ref(),
        Some(ActiveProfileOwnerSwitchState::ReferenceChanged { .. })
    ));
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
}

#[tokio::test]
async fn provider_dispatch_failure_explicitly_aborts_only_pre_application_reservation() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider.failing_creation())],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed
        ),
        PortError::Failed(_)
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-aborted")
    );
}

#[tokio::test]
async fn provider_identity_preflight_failure_aborts_before_core_application() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let refs = Arc::new(MemoryReferenceStore::with_events(
        reference("mihomo", "main", 1),
        Arc::clone(&events),
    ));
    let store: Arc<dyn ProfileStore> =
        Arc::new(FakeProfileStore::new("sing-box", "sing-box", "primary"));
    let (provider, counts) = FakeProvider::new("sing-box", "singbox-instance", Arc::clone(&events));
    let prepared_target_document = load_prepared_profile_document(store.as_ref()).await;
    let selector = selector(
        Arc::clone(&refs),
        vec![store],
        vec![Arc::new(provider.with_identity_engine("mihomo"))],
        supported_capabilities(),
    );
    let before = running_owner_snapshot("mihomo", "mihomo-old-instance", 2);
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
        switch_failure(
            result,
            ActiveProfileBackendSwitchFailurePhase::OldOwnerClosed
        ),
        PortError::Failed(_)
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        refs.read().await.unwrap(),
        reference("sing-box", "primary", 2)
    );
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-aborted")
    );
}

#[test]
fn registry_registration_probe_does_not_dispatch_a_provider() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let (provider, counts) = FakeProvider::new("mihomo", "mihomo-instance", events);
    let provider: Arc<dyn EngineBackendProvider> = Arc::new(provider);
    let registry =
        EngineBackendFactoryRegistry::new([provider]).unwrap();
    let factory: &dyn EngineBackendFactory = &registry;

    assert_eq!(
        factory.provider_availability(&engine_id_value("mihomo")),
        EngineBackendProviderAvailability::Registered
    );
    assert!(matches!(
        factory.provider_availability(&engine_id_value("sing-box")),
        EngineBackendProviderAvailability::Unsupported { .. }
    ));
    assert_eq!(counts.created.load(Ordering::SeqCst), 0);
}
