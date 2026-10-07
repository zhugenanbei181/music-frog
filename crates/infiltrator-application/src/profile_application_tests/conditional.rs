//! Behavior cases for conditional.
//! test-intent: behavior

use super::*;

#[tokio::test]
async fn conditional_update_sends_stored_validators_and_persists_new_etag() {
    let store = subscription_store(
        Some("\"old\""),
        Some("Wed, 21 Oct 2026 07:28:00 GMT"),
        Some("UA-1"),
        true,
    )
    .await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "proxies:\n  - name: a\n    type: ss\n  - name: b\n    type: ss\n",
        Some("\"new\""),
    );

    let report = application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("conditional update");
    assert_eq!(
        report.outcome,
        SubscriptionUpdateOutcome::Updated {
            new_bytes: 59,
            node_count: 2
        }
    );
    assert!(report.backed_up);
    assert!(report.usage_warning); // 1000/1000 used
    let headers = source.calls.lock().expect("calls lock")[0].clone();
    assert_eq!(headers.etag.as_deref(), Some("\"old\""));
    assert_eq!(headers.custom_user_agent.as_deref(), Some("UA-1"));
    assert!(headers.insecure_skip_verify);
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.etag.as_deref(), Some("\"new\""));
    assert_eq!(metadata.traffic_total, Some(1000));
}

#[tokio::test]
async fn conditional_update_not_modified_keeps_content_and_keeps_last_updated() {
    let store = subscription_store(None, None, None, false).await;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::not_modified(Some("\"same\""));
    let original = store.load("main").await.expect("load");

    let report = application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("conditional update");
    assert!(matches!(
        report.outcome,
        SubscriptionUpdateOutcome::NotModified { .. }
    ));
    assert!(!report.backed_up);
    assert_eq!(store.load("main").await.expect("load"), original);
    let metadata = application.load_metadata("main").await.expect("metadata");
    assert_eq!(metadata.etag.as_deref(), Some("\"same\""));
    assert!(metadata.last_updated.is_none());
}
