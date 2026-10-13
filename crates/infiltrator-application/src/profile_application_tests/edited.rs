//! Behavior cases for edited.
//! test-intent: behavior

use super::*;
use crate::profile_application::ProfileRuntimeAccess;
use crate::profile_document_application::ProfileDocumentApplication;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use std::sync::Arc;

#[tokio::test]
async fn edited_writes_refuse_protected_subscriptions_until_unlocked() {
    let store = Arc::new(FakeStore::with_profile("main", "mode: rule\n", true));
    mark_subscription(&store, "main", "https://example.com/sub");
    let profiles = profiles_with_mihomo_codec(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = profiles
        .load_workspace(&profile_id("main"))
        .await
        .unwrap()
        .identity();
    let application = ProfileDocumentApplication::new(profiles);

    let failure = application
        .save(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
            ProfileWriteAuthorization::RequireUnlock,
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
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
            ProfileWriteAuthorization::ExplicitUnlock,
        )
        .await
        .expect("the explicit unlock commits");
    assert_eq!(store.load("main").await.expect("content"), "mode: global\n");
}
