//! Behavior cases for local.
//! test-intent: behavior

use super::*;
use crate::profile_document_application::ProfileDocumentApplication;

#[tokio::test]
async fn local_profiles_stay_directly_editable() {
    let store = Arc::new(FakeStore::with_profile("lab", "mode: rule\n", true));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let source = profiles.load_workspace("lab").await.unwrap().source;
    let application = ProfileDocumentApplication::new(profiles);
    application
        .save(
            None::<Arc<dyn ManagedRuntime>>,
            &source,
            "mode: global\n",
            false,
        )
        .await
        .expect("local profile edits need no unlock");
    assert_eq!(store.load("lab").await.expect("content"), "mode: global\n");
}
