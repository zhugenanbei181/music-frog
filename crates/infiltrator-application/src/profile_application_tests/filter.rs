//! Behavior cases for filter.
//! test-intent: behavior

use super::*;
use crate::command_application::CommandApplication;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::profile_options::FilterDedup;

/// The filter commit compiles the surface draft through the shared parser: a
/// malformed rename line is a typed failure, a valid draft reshapes the stored
/// document and persists the spec.
#[tokio::test]
async fn filter_draft_save_uses_the_shared_parser_and_persists_the_spec() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
        true,
    ));
    let profiles = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles);

    let source = store.load_workspace("main").await.unwrap().source;
    let before = store.load("main").await.expect("original content");
    let invalid_mode = SubscriptionFilterDraft {
        dedup_index: 4,
        ..Default::default()
    };
    let failure = application
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &invalid_mode)
        .await
        .expect_err("unknown strategy must not become disabled");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(store.load("main").await.unwrap(), before);
    assert!(
        store.options.lock().unwrap().is_empty(),
        "rejected strategy does not persist an option sidecar"
    );

    let broken = SubscriptionFilterDraft {
        renames: "missing arrow".to_string(),
        ..SubscriptionFilterDraft::default()
    };
    let failure = application
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &broken)
        .await
        .expect_err("a malformed rename line must fail before any write");
    assert_eq!(failure.code, ErrorCode::InvalidInput);

    let draft = SubscriptionFilterDraft {
        exclude: "广告".to_string(),
        dedup_index: 1,
        ..SubscriptionFilterDraft::default()
    };
    let report = application
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &draft)
        .await
        .expect("filter run")
        .report;
    assert_eq!(report.total_input, 2);
    assert_eq!(report.passed, 1);
    let saved = store.load("main").await.expect("content");
    assert!(!saved.contains("广告-02"));
    let stored = ProfileApplication::new(Arc::clone(&store) as Arc<dyn ProfileStore>)
        .load_options("main")
        .await
        .expect("options")
        .filter
        .expect("filter stored");
    assert_eq!(stored.exclude_keywords, vec!["广告".to_string()]);
    assert_eq!(stored.deduplication, FilterDedup::KeepFirst);
}

#[tokio::test]
async fn filter_commit_rejects_unreadable_options_before_mutating_the_profile() {
    let content = "proxies:\n  - name: keep\n    type: ss\n  - name: remove\n    type: vmess\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    let profiles = ProfileApplication::new(store.clone());
    let application = ProfileOptionsApplication::new(profiles);
    let source = store.load_workspace("main").await.unwrap().source;
    let draft = SubscriptionFilterDraft {
        exclude: "remove".into(),
        ..Default::default()
    };
    for (error, code) in [
        (
            PortError::PermissionDenied("sidecar denied".into()),
            ErrorCode::Permission,
        ),
        (
            PortError::Io("sidecar cannot be decoded".into()),
            ErrorCode::Storage,
        ),
    ] {
        *store.options_read_error.lock().unwrap() = Some(error);
        let failure = application
            .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &draft)
            .await
            .unwrap_err();
        assert_eq!(failure.code, code);
        assert!(failure.message.contains("sidecar"));
        assert_eq!(store.load("main").await.unwrap(), content);
        assert!(store.options.lock().unwrap().is_empty());
        assert!(store.cleared_backups.lock().unwrap().is_empty());
    }
    *store.options_read_error.lock().unwrap() = None;
    let report = application
        .save_filter(None::<Arc<dyn ManagedRuntime>>, &source, &draft)
        .await
        .unwrap()
        .report;
    assert_eq!((report.total_input, report.passed), (2, 1));
    assert!(!store.load("main").await.unwrap().contains("name: remove"));
    assert_eq!(
        store
            .load_options("main")
            .await
            .unwrap()
            .filter
            .unwrap()
            .exclude_keywords,
        vec!["remove"]
    );
}

#[tokio::test]
async fn typed_filter_command_keeps_advanced_policy_and_returns_real_source_and_report() {
    let content = "proxies:\n  - name: public\n    type: ss\n    server: 1.1.1.1\n  - name: private\n    type: ss\n    server: 192.168.1.1\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    store
        .save_options(
            "main",
            &ProfileOptions {
                filter: Some(FilterSpec {
                    drop_private_ip: true,
                    remove_emojis: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let source = store.load_workspace("main").await.unwrap().source;
    let commands = CommandApplication::new().with_profile(ProfileApplication::new(store.clone()));
    let output = commands
        .execute_output(CommandIntent::SaveSubscriptionFilter {
            source: source.clone(),
            filter: SubscriptionFilterDraft::default(),
        })
        .await
        .unwrap();
    let CommandOutput::SubscriptionFilterApplied(applied) = output else {
        panic!("typed report");
    };
    assert_eq!(
        (
            applied.report.total_input,
            applied.report.passed,
            applied.report.excluded_by_server
        ),
        (2, 1, 1)
    );
    let committed = store.load_workspace("main").await.unwrap();
    assert_eq!(applied.source, committed.source);
    assert_ne!(applied.source, source);
    let policy = committed.options.filter.as_ref().unwrap();
    assert!(policy.drop_private_ip && policy.remove_emojis);
    assert!(committed.content.contains("name: public"));
    assert!(!committed.content.contains("name: private"));
    let failure = commands
        .execute_output(CommandIntent::SaveSubscriptionFilter {
            source,
            filter: SubscriptionFilterDraft::default(),
        })
        .await
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(store.load_workspace("main").await.unwrap(), committed);
}

#[tokio::test]
async fn advanced_filter_command_validates_before_writing_and_explicit_clear_removes_saved_policy()
{
    let content = "proxies:\n  - {name: public, type: ss, server: 1.1.1.1, port: 443}\n  - {name: private, type: ss, server: 192.168.1.1, port: 443}\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    let commands = CommandApplication::new().with_profile(ProfileApplication::new(store.clone()));
    let original = store.load_workspace("main").await.unwrap();
    for invalid in [
        "drop-private-ips: true",
        "allowed-ports: [0]",
        "include-keywords: [public]",
    ] {
        let failure = commands
            .execute_output(CommandIntent::SaveSubscriptionFilter {
                source: original.source.clone(),
                filter: SubscriptionFilterDraft {
                    advanced_policy: Some(invalid.into()),
                    ..Default::default()
                },
            })
            .await
            .unwrap_err();
        assert_eq!(failure.code, ErrorCode::InvalidInput);
        assert_eq!(store.load_workspace("main").await.unwrap(), original);
        assert!(store.cleared_backups.lock().unwrap().is_empty());
    }
    let output = commands
        .execute_output(CommandIntent::SaveSubscriptionFilter {
            source: original.source,
            filter: SubscriptionFilterDraft {
                advanced_policy: Some(
                    "{drop-private-ip: true, node-mutator: {force-udp: true}}".into(),
                ),
                ..Default::default()
            },
        })
        .await
        .unwrap()
        .into_subscription_filter()
        .unwrap();
    assert_eq!(
        (
            output.report.total_input,
            output.report.passed,
            output.report.excluded_by_server
        ),
        (2, 1, 1)
    );
    let saved = store.load_workspace("main").await.unwrap();
    assert_eq!(output.source, saved.source);
    assert!(saved.options.filter.as_ref().unwrap().drop_private_ip);
    assert!(saved.content.contains("udp: true"));
    let loaded = ProfileOptionsApplication::new(ProfileApplication::new(store.clone()))
        .load(Some("main"))
        .await
        .unwrap();
    assert!(
        loaded
            .filter
            .advanced_policy
            .as_ref()
            .unwrap()
            .contains("drop-private-ip")
    );
    commands
        .execute_output(CommandIntent::SaveSubscriptionFilter {
            source: saved.source,
            filter: SubscriptionFilterDraft {
                advanced_policy: Some("{}".into()),
                ..Default::default()
            },
        })
        .await
        .unwrap();
    assert_eq!(
        store.load_workspace("main").await.unwrap().options.filter,
        None,
        "an explicitly cleared policy removes the stored filter"
    );
}
