//! Behavior cases for mixin.
//! test-intent: behavior

use super::*;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

/// The Mixin commit strips the outgoing mixin's injected rule lines, so
/// re-saving an edited overlay never duplicates rules and never loses the
/// hand-written comments around them.
#[tokio::test]
async fn mixin_save_is_idempotent_and_keeps_handwritten_comments() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "# 手写注释\nmode: rule\nrules:\n  - MATCH,DIRECT\n",
        true,
    ));
    let profiles = profiles_with_mihomo_codec(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles.clone());
    let overlay = "rules:\n  append:\n    - DOMAIN-SUFFIX,example.com,DIRECT\n";

    for _ in 0..2 {
        application
            .save_mixin(
                crate::profile_application::ProfileRuntimeAccess::<dyn ManagedRuntime>::Offline,
                &load_workspace(&store, "main").await.identity(),
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
    let options = application
        .load(Some("main"))
        .await
        .expect("sidecar stored");
    assert!(
        options
            .mixin_yaml
            .contains("DOMAIN-SUFFIX,example.com,DIRECT")
    );
}

#[tokio::test]
async fn failed_workspace_pair_commit_keeps_exact_document_and_sidecar_bytes() {
    use infiltrator_ports::runtime_gateway::ManagedRuntime;

    let content = "# retained document bytes\r\nmode: rule\r\n";
    let sidecar = b"# retained sidecar bytes\r\nfilter:\r\n  include-keywords: [HK]\r\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    store
        .sidecars
        .lock()
        .unwrap()
        .insert("main".into(), sidecar.to_vec());
    let profiles = profiles_with_mihomo_codec(store.clone());
    let options = ProfileOptionsApplication::new(profiles.clone());
    let before = profiles.load_workspace(&profile_id("main")).await.unwrap();
    *store.workspace_write_error.lock().unwrap() = Some(PortError::PermissionDenied(
        "pair publication denied".into(),
    ));

    let failure = options
        .save_mixin(
            crate::profile_application::ProfileRuntimeAccess::<dyn ManagedRuntime>::Offline,
            &before.identity(),
            "mode: global\n",
        )
        .await
        .unwrap_err();

    assert_eq!(failure.code, ErrorCode::Permission);
    assert_eq!(
        store.load("main").await.unwrap().as_bytes(),
        content.as_bytes()
    );
    assert_eq!(
        store
            .sidecars
            .lock()
            .unwrap()
            .get("main")
            .unwrap()
            .as_slice(),
        sidecar.as_slice()
    );
    assert_eq!(store.workspace_writes.load(Ordering::SeqCst), 0);
    *store.workspace_write_error.lock().unwrap() = None;
    assert_eq!(
        profiles.load_workspace(&profile_id("main")).await.unwrap(),
        before
    );
}
