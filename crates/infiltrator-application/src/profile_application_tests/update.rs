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

#[tokio::test]
async fn age_subscription_refresh_commits_owner_decrypted_content() {
    let store = subscription_store(None, None, None, false).await;
    store.decrypt_age_to("proxies:\n  - name: decrypted\n    type: ss\n", 4);
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "-----BEGIN AGE ENCRYPTED FILE-----\nopaque-ciphertext\n-----END AGE ENCRYPTED FILE-----",
        None,
    );

    let report = application
        .update_subscription_conditional(&source, "main")
        .await
        .expect("Age subscription refresh");

    assert!(matches!(
        report.outcome,
        SubscriptionUpdateOutcome::Updated { .. }
    ));
    assert_eq!(
        store.load("main").await.unwrap(),
        "proxies:\n  - name: decrypted\n    type: ss\n"
    );
    assert_eq!(store.age_decryption_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn age_subscription_refresh_without_owner_key_preserves_the_previous_document() {
    let store = subscription_store(None, None, None, false).await;
    let previous = store.load("main").await.unwrap();
    *store.age_decryption.lock().expect("Age decryption") =
        FakeAgeDecryption::CredentialNotConfigured;
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = FakeSource::modified(
        "-----BEGIN AGE ENCRYPTED FILE-----\nopaque-ciphertext\n-----END AGE ENCRYPTED FILE-----",
        None,
    );

    let failure = application
        .update_subscription_conditional(&source, "main")
        .await
        .expect_err("refresh without the owner key must fail");

    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(store.load("main").await.unwrap(), previous);
    assert_eq!(store.age_decryption_calls.load(Ordering::SeqCst), 1);
}
