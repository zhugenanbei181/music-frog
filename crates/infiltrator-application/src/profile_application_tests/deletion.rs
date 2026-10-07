//! Behavior cases for deletion.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn deletion_cleans_the_profile_sidecar() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", false));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    application.delete_profile("main").await.expect("delete");
    assert_eq!(
        store
            .deleted_options
            .lock()
            .expect("options lock")
            .as_slice(),
        &["main".to_string()]
    );
}
