//! Behavior cases for write.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn write_protection_is_derived_from_the_subscription_source() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    // assert_eq fails on ProfileWriteProtection without Debug; use is_protected.
    assert!(
        !application
            .write_protection("main")
            .await
            .expect("protection")
            .is_protected()
    );

    mark_subscription(&store, "main", "https://example.com/sub");
    let protected = application
        .write_protection("main")
        .await
        .expect("protection");
    assert!(protected.is_protected());
    assert_eq!(protected, ProfileWriteProtection::RemoteSubscription);
}
