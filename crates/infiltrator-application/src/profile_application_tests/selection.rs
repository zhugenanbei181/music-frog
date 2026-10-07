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
