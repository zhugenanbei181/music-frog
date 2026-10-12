use super::*;

#[tokio::test]
async fn product_root_can_abort_returned_switch_candidate_before_application_creation() {
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

    let candidate = selector
        .switch_backend(
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
        *refs.transition.lock().unwrap(),
        Some(ActiveProfileOwnerSwitchState::BindingNew {
            selection: reference("sing-box", "primary", 2),
            engine_id: engine_id_value("sing-box"),
        })
    );
    assert_eq!(
        *refs.owner.lock().unwrap(),
        FakeOwnerAdmission::Reserved {
            engine_id: engine_id_value("sing-box"),
        }
    );
    let restored = candidate.abort_before_core_application().await.unwrap();

    assert_eq!(restored, reference("sing-box", "primary", 2));
    assert_eq!(refs.read().await.unwrap(), restored);
    assert_eq!(*refs.transition.lock().unwrap(), None);
    assert_eq!(*refs.owner.lock().unwrap(), FakeOwnerAdmission::Unowned);
    assert_eq!(counts.created.load(Ordering::SeqCst), 1);
    assert_eq!(counts.starts.load(Ordering::SeqCst), 0);
    assert_eq!(
        events.lock().unwrap().last().map(String::as_str),
        Some("candidate-aborted")
    );
}
