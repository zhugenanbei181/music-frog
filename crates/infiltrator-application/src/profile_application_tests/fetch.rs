//! Behavior cases for fetch.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn fetch_settings_are_persisted_and_blank_ua_cleared() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    application
        .update_subscription_fetch_settings("main", Some("UA-X".to_string()), true)
        .await
        .expect("save settings");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.user_agent.as_deref(), Some("UA-X"));
    assert!(metadata.insecure_skip_verify);

    application
        .update_subscription_fetch_settings("main", Some("   ".to_string()), false)
        .await
        .expect("clear settings");
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.user_agent, None);
    assert!(!metadata.insecure_skip_verify);
}
