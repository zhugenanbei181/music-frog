//! Behavior cases for selection.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn selection_updates_the_current_profile() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let selected = application.select_profile("main").await.expect("select");
    assert!(selected.active);
    assert_eq!(
        application.current_profile().await.expect("current"),
        "main"
    );
}

#[tokio::test]
async fn selection_target_resolves_without_publishing_an_offline_reference() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let target = application
        .active_reference_target_for("main")
        .await
        .expect("resolve selection target");

    assert_eq!(
        target,
        ActiveProfileReferenceTarget::selected(
            EngineId::new("mihomo").expect("test engine ID"),
            ProfileId::new("main").expect("test profile ID"),
        )
    );
    assert_eq!(store.reference_write_count.load(Ordering::SeqCst), 0);
    assert!(matches!(
        &*store.active_reference.lock().expect("active reference"),
        ActiveProfileReferenceState::Unselected { .. }
    ));
    assert!(
        application
            .active_reference_target_for("missing")
            .await
            .is_err()
    );
}
