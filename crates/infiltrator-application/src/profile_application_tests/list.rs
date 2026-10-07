//! Behavior cases for list.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn list_and_detail_are_projected_from_the_store() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(store);

    let profiles = application.list_profiles().await.expect("list");
    assert_eq!(profiles.len(), 1);
    assert!(profiles[0].active);

    let detail = application
        .load_profile_detail("main")
        .await
        .expect("detail");
    assert_eq!(detail.content, "mode: rule\n");
    assert!(detail.active);
}
