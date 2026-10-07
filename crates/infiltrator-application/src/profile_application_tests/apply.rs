//! Behavior cases for apply.
//! test-intent: behavior

use super::*;
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::profile_options::FilterSpec;

/// DUAL-07-08: the shared filter runner reshapes the stored document and
/// persists the spec sidecar so the next subscription update recomposes from
/// the same node strategy.
#[tokio::test]
async fn apply_subscription_filter_reshapes_document_and_persists_spec() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
        true,
    ));
    let application = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let spec = FilterSpec {
        exclude_keywords: vec!["广告".to_string()],
        ..Default::default()
    };

    let runtime: Option<Arc<dyn ManagedRuntime>> = None;
    let source = application.load_workspace("main").await.unwrap().source;
    let report = application
        .apply_subscription_filter(runtime, &source, spec)
        .await
        .expect("filter")
        .report;
    assert_eq!(report.total_input, 2);
    assert_eq!(report.passed, 1);
    let saved = store.load("main").await.expect("load");
    assert!(!saved.contains("广告-02"), "excluded node must be dropped");
    assert!(saved.contains("香港-01"));

    let options = application.load_options("main").await.expect("options");
    assert_eq!(
        options.filter.expect("filter stored").exclude_keywords,
        vec!["广告".to_string()]
    );
}

#[tokio::test]
async fn bound_filter_rejects_changed_document_or_options_and_returns_actual_commit_identity() {
    let original = "proxies:\n  - name: keep\n    type: ss\n  - name: remove\n    type: vmess\n";
    let store = Arc::new(FakeStore::with_profile("main", original, true));
    let application = ProfileApplication::new(store.clone());
    let source = application.load_workspace("main").await.unwrap().source;
    let spec = FilterSpec {
        exclude_keywords: vec!["remove".into()],
        ..Default::default()
    };
    store
        .save("main", &format!("# later edit\n{original}"))
        .await
        .unwrap();
    let later = application.load_workspace("main").await.unwrap();
    let failure = application
        .apply_subscription_filter(None::<Arc<dyn ManagedRuntime>>, &source, spec.clone())
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(application.load_workspace("main").await.unwrap(), later);
    let newer_options = ProfileOptions {
        mixin: MixinConfig {
            mode: Some("direct".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    store.save_options("main", &newer_options).await.unwrap();
    let latest = application.load_workspace("main").await.unwrap();
    let failure = application
        .apply_subscription_filter(None::<Arc<dyn ManagedRuntime>>, &later.source, spec.clone())
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(application.load_workspace("main").await.unwrap(), latest);
    let applied = application
        .apply_subscription_filter(None::<Arc<dyn ManagedRuntime>>, &latest.source, spec)
        .await
        .unwrap();
    assert_eq!((applied.report.total_input, applied.report.passed), (2, 1));
    let committed = application.load_workspace("main").await.unwrap();
    assert_eq!(applied.source, committed.source);
    assert_ne!(applied.source, latest.source);
    assert_eq!(committed.options.mixin, newer_options.mixin);
    assert!(!committed.content.contains("name: remove"));
}
