//! Behavior cases for update.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn update_subscription_delegates_to_conditional_path() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified("proxies:\n  - name: a\n    type: ss\n", None);

    let updated = application
        .update_subscription(&source, "main")
        .await
        .expect("update");
    assert_eq!(updated.name, "main");
    assert_eq!(source.calls.lock().expect("calls lock").len(), 1);
}
