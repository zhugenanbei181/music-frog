//! Behavior cases for auto.
//! test-intent: behavior

use super::*;

/// DUAL-07-09: the auto-reload preference round-trips through the shared
/// application so the refresh path can consume it.
#[tokio::test]
async fn auto_reload_preference_is_persisted_and_readable() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    assert!(
        !application
            .load_metadata("main")
            .await
            .expect("metadata")
            .auto_reload_core,
        "default metadata starts opted out"
    );
    application
        .update_subscription_auto_reload("main", true)
        .await
        .expect("enable persists");
    assert!(
        application
            .load_metadata("main")
            .await
            .expect("metadata")
            .auto_reload_core
    );
    application
        .update_subscription_auto_reload("main", false)
        .await
        .expect("disable persists");
    assert!(
        !application
            .load_metadata("main")
            .await
            .expect("metadata")
            .auto_reload_core
    );
}
