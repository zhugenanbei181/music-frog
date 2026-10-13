//! Behavior cases for local.
//! test-intent: behavior

use super::*;
use crate::profile_application::ProfileRuntimeAccess;
use crate::profile_document_application::ProfileDocumentApplication;
use infiltrator_contract::profile_protection::ProfileWriteAuthorization;
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use std::sync::Arc;

#[tokio::test]
async fn local_profiles_stay_directly_editable() {
    let store = Arc::new(FakeStore::with_profile("lab", "mode: rule\n", true));
    let profiles = profiles_with_mihomo_codec(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = profiles
        .load_workspace(&profile_id("lab"))
        .await
        .unwrap()
        .identity();
    let application = ProfileDocumentApplication::new(profiles);
    application
        .save(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            "mode: global\n",
            ProfileWriteAuthorization::RequireUnlock,
        )
        .await
        .expect("local profile edits need no unlock");
    assert_eq!(store.load("lab").await.expect("content"), "mode: global\n");
}
