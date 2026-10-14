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

#[tokio::test]
async fn age_subscription_import_commits_only_owner_decrypted_content() {
    let store = Arc::new(FakeStore::with_profile("encrypted", "mode: rule\n", false));
    store.decrypt_age_to("proxies: []\n", 7);
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "-----BEGIN AGE ENCRYPTED FILE-----\nopaque-ciphertext\n-----END AGE ENCRYPTED FILE-----",
        None,
    );

    let report = application
        .import_subscription_report(&source, "encrypted", "https://example.com/sub.yaml")
        .await
        .expect("Age subscription import");

    assert_eq!(report.content_bytes, "proxies: []\n".len());
    assert_eq!(store.load("encrypted").await.unwrap(), "proxies: []\n");
    assert_eq!(store.age_decryption_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn age_subscription_import_without_owner_key_preserves_the_existing_profile() {
    let store = Arc::new(FakeStore::with_profile("encrypted", "mode: rule\n", false));
    let previous = store.load("encrypted").await.unwrap();
    *store.age_decryption.lock().expect("Age decryption") =
        FakeAgeDecryption::CredentialNotConfigured;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "-----BEGIN AGE ENCRYPTED FILE-----\nopaque-ciphertext\n-----END AGE ENCRYPTED FILE-----",
        None,
    );

    let failure = application
        .import_subscription_report(&source, "encrypted", "https://example.com/sub.yaml")
        .await
        .expect_err("missing key must reject import");

    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(store.load("encrypted").await.unwrap(), previous);
    assert_eq!(store.age_decryption_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn age_subscription_import_rejects_invalid_ciphertext_without_writing() {
    let store = Arc::new(FakeStore::with_profile("encrypted", "mode: rule\n", false));
    let previous = store.load("encrypted").await.unwrap();
    *store.age_decryption.lock().expect("Age decryption") =
        FakeAgeDecryption::InvalidCredentialOrContent;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "-----BEGIN AGE ENCRYPTED FILE-----\nmalformed-ciphertext\n-----END AGE ENCRYPTED FILE-----",
        None,
    );

    let failure = application
        .import_subscription_report(&source, "encrypted", "https://example.com/sub.yaml")
        .await
        .expect_err("invalid Age ciphertext must reject import");

    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(store.load("encrypted").await.unwrap(), previous);
    assert_eq!(store.age_decryption_calls.load(Ordering::SeqCst), 1);
}
