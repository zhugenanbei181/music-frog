//! Behavior cases for mixin.
//! test-intent: behavior

use super::*;

/// The Mixin commit strips the outgoing mixin's injected rule lines, so
/// re-saving an edited overlay never duplicates rules and never loses the
/// hand-written comments around them.
#[tokio::test]
async fn mixin_save_is_idempotent_and_keeps_handwritten_comments() {
    use crate::profile_options_application::ProfileOptionsApplication;

    let store = Arc::new(FakeStore::with_profile(
        "main",
        "# 手写注释\nmode: rule\nrules:\n  - MATCH,DIRECT\n",
        true,
    ));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles);
    let overlay = "rules:\n  append:\n    - DOMAIN-SUFFIX,example.com,DIRECT\n";

    for _ in 0..2 {
        application
            .save_mixin(
                None::<Arc<dyn ManagedRuntime>>,
                &store.load_workspace("main").await.unwrap().source,
                overlay,
            )
            .await
            .expect("mixin save");
    }

    let saved = store.load("main").await.expect("content");
    assert!(
        saved.contains("# 手写注释"),
        "the byte-faithful merge keeps the hand-written comment: {saved}"
    );
    assert_eq!(
        saved.matches("DOMAIN-SUFFIX,example.com,DIRECT").count(),
        1,
        "the second save strips the first injection before re-applying: {saved}"
    );
    let options = store
        .options
        .lock()
        .expect("options lock")
        .get("main")
        .cloned()
        .expect("sidecar stored");
    assert_eq!(
        options.mixin.rules.expect("rule mixin").append,
        vec!["DOMAIN-SUFFIX,example.com,DIRECT".to_string()]
    );
}
