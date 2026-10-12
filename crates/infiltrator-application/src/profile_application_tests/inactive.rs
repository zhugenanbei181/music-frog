//! Behavior cases for inactive.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn inactive_save_clears_the_transient_backup() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    application
        .save_profile_content(
            ProfileRuntimeAccess::<
                dyn infiltrator_ports::runtime_gateway::ActiveProfileReferenceApplier,
            >::Offline,
            "main".to_string(),
            "mode: direct\n".to_string(),
            ApplyStrategy::PreferReload,
        )
        .await
        .expect("save");
    assert_eq!(store.load("main").await.expect("load"), "mode: direct\n");
    assert_eq!(
        store
            .cleared_backups
            .lock()
            .expect("backup lock")
            .as_slice(),
        &["main".to_string()]
    );
}
