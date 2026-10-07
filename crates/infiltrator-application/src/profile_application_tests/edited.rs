//! Behavior cases for edited.
//! test-intent: behavior

use super::*;
use crate::profile_document_application::ProfileDocumentApplication;

#[tokio::test]
async fn edited_writes_refuse_protected_subscriptions_until_unlocked() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    mark_subscription(&store, "main", "https://example.com/sub");
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = profiles.load_workspace("main").await.unwrap().source;
    let application = ProfileDocumentApplication::new(profiles);

    let failure = application
        .save(
            None::<Arc<dyn ManagedRuntime>>,
            &source,
            "mode: global\n",
            false,
        )
        .await
        .expect_err("direct edit of a protected subscription must fail");
    assert_eq!(failure.code, ErrorCode::Configuration);
    assert_eq!(
        store.load("main").await.expect("content"),
        "mode: rule\n",
        "the refused write must not touch the profile"
    );

    application
        .save(
            None::<Arc<dyn ManagedRuntime>>,
            &source,
            "mode: global\n",
            true,
        )
        .await
        .expect("the explicit unlock commits");
    assert_eq!(store.load("main").await.expect("content"), "mode: global\n");
}
