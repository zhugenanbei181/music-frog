//! Behavior cases for adapter-scoped filter operations.
//! test-intent: behavior

use super::*;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;

/// A neutral filter draft is transformed by the selected codec and the
/// resulting opaque document/sidecar pair is committed as one workspace.
#[tokio::test]
async fn filter_application_reshapes_document_and_persists_projection() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
        true,
    ));
    let profiles = profiles_with_mihomo_codec(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let options = ProfileOptionsApplication::new(profiles.clone());
    let source = profiles.load_workspace("main").await.unwrap().identity();
    let draft = SubscriptionFilterDraft {
        exclude: "广告".into(),
        ..Default::default()
    };

    let applied = options
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &draft)
        .await
        .expect("filter");
    assert_eq!((applied.report.total_input, applied.report.passed), (2, 1));
    let committed = profiles.load_workspace("main").await.unwrap();
    let saved = committed.document.utf8_text().unwrap();
    assert!(!saved.contains("广告-02"), "excluded node must be dropped");
    assert!(saved.contains("香港-01"));
    assert_eq!(applied.workspace_identity, committed.identity());
    let projection = options.load(Some("main")).await.unwrap();
    assert_eq!(projection.filter.exclude, "广告");
}

#[tokio::test]
async fn bound_filter_rejects_changed_document_or_options_and_returns_actual_commit_identity() {
    let original = "proxies:\n  - name: keep\n    type: ss\n  - name: remove\n    type: vmess\n";
    let store = Arc::new(FakeStore::with_profile("main", original, true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    let options = ProfileOptionsApplication::new(profiles.clone());
    let source = profiles.load_workspace("main").await.unwrap().identity();
    let draft = SubscriptionFilterDraft {
        exclude: "remove".into(),
        ..Default::default()
    };
    store
        .save("main", &format!("# later edit\n{original}"))
        .await
        .unwrap();
    let later = profiles.load_workspace("main").await.unwrap();
    let failure = options
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &draft)
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(profiles.load_workspace("main").await.unwrap(), later);

    let mixin = options
        .save_mixin(
            None::<Arc<dyn ManagedRuntime>>,
            &later.identity(),
            "mode: direct\n",
        )
        .await
        .unwrap();
    let latest = profiles.load_workspace("main").await.unwrap();
    assert_eq!(mixin.options.workspace_identity, latest.identity());
    let failure = options
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &later.identity(), &draft)
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(profiles.load_workspace("main").await.unwrap(), latest);

    let applied = options
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &latest.identity(), &draft)
        .await
        .unwrap();
    assert_eq!((applied.report.total_input, applied.report.passed), (2, 1));
    let committed = profiles.load_workspace("main").await.unwrap();
    assert_eq!(applied.workspace_identity, committed.identity());
    assert_ne!(applied.workspace_identity, latest.identity());
    assert!(
        options
            .load(Some("main"))
            .await
            .unwrap()
            .mixin_yaml
            .contains("mode: direct")
    );
    assert!(
        !committed
            .document
            .utf8_text()
            .unwrap()
            .contains("name: remove")
    );
}
