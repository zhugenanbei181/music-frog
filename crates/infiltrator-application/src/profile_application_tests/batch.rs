//! Behavior cases for batch.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn batch_update_skips_url_less_profiles_and_aggregates_counts() {
    let store = subscription_store(None, None, None, false).await;
    store.profiles.lock().expect("profiles lock").insert(
        "local".to_string(),
        ("mode: rule\n".to_string(), ProfileMetadata::default()),
    );
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified("proxies:\n  - name: n2\n    type: ss\n", None);

    let report = application
        .update_all_subscriptions(&source, 4)
        .await
        .expect("batch");
    assert_eq!(report.total, 2);
    assert_eq!(report.skipped, 1, "profile without a URL is skipped");
    assert_eq!(report.updated, 1);
    assert_eq!(report.failed, 0);
    assert_eq!(report.not_modified, 0);
    assert_eq!(report.outcomes.len(), 1);
    assert_eq!(source.calls.lock().expect("calls lock").len(), 1);
}
