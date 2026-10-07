//! Behavior cases for import.
//! test-intent: behavior

use super::*;
use infiltrator_contract::subscription_import::SubscriptionImportChannel;

/// DUAL-07-01: a local/clipboard document is normalized and validated before
/// it is committed, reporting the detected format and node count.
#[tokio::test]
async fn import_document_validates_and_reports_format() {
    let store = Arc::new(FakeStore::default());
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);

    let report = application
        .import_document(
            "imported",
            "proxies:\n  - name: a\n    type: ss\n  - name: b\n    type: ss\n",
            SubscriptionImportChannel::LocalFile,
        )
        .await
        .expect("import");
    assert_eq!(report.node_count, 2);
    assert_eq!(report.channel, SubscriptionImportChannel::LocalFile);
    assert!(store.load("imported").await.is_ok());

    let failure = application
        .import_document(
            "broken",
            "proxies: [unbalanced",
            SubscriptionImportChannel::Clipboard,
        )
        .await
        .expect_err("malformed yaml must fail");
    assert_eq!(failure.code, ErrorCode::Configuration);
}
