//! Behavior cases for filter.
//! test-intent: behavior

use super::*;
use crate::command_application::CommandApplication;
use crate::core_capability_test_support::{execute_core_output_command, with_test_capabilities};
use crate::profile_application::ProfileRuntimeAccess;
use crate::profile_options_application::ProfileOptionsApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_domain::profile_options::{FilterSpec, ProfileOptions};
use infiltrator_ports::active_profile_reference_applier::ActiveProfileReferenceApplier;
use std::sync::Arc;

/// The scoped Mihomo codec rejects malformed drafts before a write, then
/// reshapes the document and returns the persisted neutral projection.
#[tokio::test]
async fn filter_draft_save_uses_the_scoped_codec_and_persists_projection() {
    let store = Arc::new(FakeStore::with_profile(
        "main",
        "proxies:\n  - name: 香港-01\n    type: ss\n  - name: 广告-02\n    type: vmess\n",
        true,
    ));
    let profiles = profiles_with_mihomo_codec(Arc::clone(&store) as Arc<dyn ProfileStore>);
    let application = ProfileOptionsApplication::new(profiles.clone());

    let source = load_workspace(&store, "main").await.identity();
    let before = store.load("main").await.expect("original content");
    let invalid_mode = SubscriptionFilterDraft {
        dedup_index: 4,
        ..Default::default()
    };
    let failure = application
        .save_filter(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            &invalid_mode,
        )
        .await
        .expect_err("unknown strategy must not become disabled");
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(store.load("main").await.unwrap(), before);
    assert!(
        store.sidecars.lock().unwrap().is_empty(),
        "rejected strategy does not persist an option sidecar"
    );

    let broken = SubscriptionFilterDraft {
        renames: "missing arrow".to_string(),
        ..SubscriptionFilterDraft::default()
    };
    let failure = application
        .save_filter(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            &broken,
        )
        .await
        .expect_err("a malformed rename line must fail before any write");
    assert_eq!(failure.code, ErrorCode::InvalidInput);

    let draft = SubscriptionFilterDraft {
        exclude: "广告".to_string(),
        dedup_index: 1,
        ..SubscriptionFilterDraft::default()
    };
    let report = application
        .save_filter(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            &draft,
        )
        .await
        .expect("filter run")
        .report;
    assert_eq!(report.total_input, 2);
    assert_eq!(report.passed, 1);
    let saved = store.load("main").await.expect("content");
    assert!(!saved.contains("广告-02"));
    let stored = application
        .load(Some("main"))
        .await
        .expect("options")
        .filter;
    assert_eq!(stored.exclude, "广告");
    assert_eq!(stored.dedup_index, 1);
}

#[tokio::test]
async fn filter_commit_rejects_unreadable_options_before_mutating_the_profile() {
    let content = "proxies:\n  - name: keep\n    type: ss\n  - name: remove\n    type: vmess\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    let application = ProfileOptionsApplication::new(profiles.clone());
    let source = load_workspace(&store, "main").await.identity();
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
            .save_filter(
                ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
                &source,
                &draft,
            )
            .await
            .unwrap_err();
        assert_eq!(failure.code, code);
        assert!(failure.message.contains("sidecar"));
        assert_eq!(store.load("main").await.unwrap(), content);
        assert!(store.sidecars.lock().unwrap().is_empty());
        assert!(store.cleared_backups.lock().unwrap().is_empty());
    }
    *store.options_read_error.lock().unwrap() = None;
    let report = application
        .save_filter(
            ProfileRuntimeAccess::<dyn ActiveProfileReferenceApplier>::Offline,
            &source,
            &draft,
        )
        .await
        .unwrap()
        .report;
    assert_eq!((report.total_input, report.passed), (2, 1));
    assert!(!store.load("main").await.unwrap().contains("name: remove"));
    assert_eq!(
        application.load(Some("main")).await.unwrap().filter.exclude,
        "remove"
    );
}

#[tokio::test]
async fn typed_filter_command_keeps_advanced_policy_and_returns_real_source_and_report() {
    let content = "proxies:\n  - name: public\n    type: ss\n    server: 1.1.1.1\n  - name: private\n    type: ss\n    server: 192.168.1.1\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    store.set_options(
        "main",
        &ProfileOptions {
            filter: Some(FilterSpec {
                drop_private_ip: true,
                remove_emojis: true,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    let source = load_workspace(&store, "main").await.identity();
    let commands = with_test_capabilities(CommandApplication::new().with_profile(profiles.clone()));
    let output = execute_core_output_command(
        &commands,
        CommandIntent::SaveSubscriptionFilter {
            workspace_identity: source.clone(),
            filter: SubscriptionFilterDraft::default(),
        },
    )
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
    let committed = load_workspace(&store, "main").await;
    assert_eq!(applied.workspace_identity, committed.identity());
    assert_ne!(applied.workspace_identity, source);
    let policy = ProfileOptionsApplication::new(profiles.clone())
        .load(Some("main"))
        .await
        .unwrap()
        .filter;
    assert!(
        policy
            .advanced_policy
            .as_deref()
            .is_some_and(|value| value.contains("drop-private-ip"))
    );
    let committed_text = committed.document.utf8_text().unwrap();
    assert!(committed_text.contains("name: public"));
    assert!(!committed_text.contains("name: private"));
    let failure = execute_core_output_command(
        &commands,
        CommandIntent::SaveSubscriptionFilter {
            workspace_identity: source,
            filter: SubscriptionFilterDraft::default(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(load_workspace(&store, "main").await, committed);
}

#[tokio::test]
async fn advanced_filter_command_validates_before_writing_and_explicit_clear_removes_saved_policy()
{
    let content = "proxies:\n  - {name: public, type: ss, server: 1.1.1.1, port: 443}\n  - {name: private, type: ss, server: 192.168.1.1, port: 443}\n";
    let store = Arc::new(FakeStore::with_profile("main", content, true));
    let profiles = profiles_with_mihomo_codec(store.clone());
    let commands = with_test_capabilities(CommandApplication::new().with_profile(profiles.clone()));
    let original = load_workspace(&store, "main").await;
    for invalid in [
        "drop-private-ips: true",
        "allowed-ports: [0]",
        "include-keywords: [public]",
    ] {
        let failure = execute_core_output_command(
            &commands,
            CommandIntent::SaveSubscriptionFilter {
                workspace_identity: original.identity(),
                filter: SubscriptionFilterDraft {
                    advanced_policy: Some(invalid.into()),
                    ..Default::default()
                },
            },
        )
        .await
        .unwrap_err();
        assert_eq!(failure.code, ErrorCode::InvalidInput);
        assert_eq!(load_workspace(&store, "main").await, original);
        assert!(store.cleared_backups.lock().unwrap().is_empty());
    }
    let output = execute_core_output_command(
        &commands,
        CommandIntent::SaveSubscriptionFilter {
            workspace_identity: original.identity(),
            filter: SubscriptionFilterDraft {
                advanced_policy: Some(
                    "{drop-private-ip: true, node-mutator: {force-udp: true}}".into(),
                ),
                ..Default::default()
            },
        },
    )
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
    let saved = load_workspace(&store, "main").await;
    assert_eq!(output.workspace_identity, saved.identity());
    assert!(
        ProfileOptionsApplication::new(profiles.clone())
            .load(Some("main"))
            .await
            .unwrap()
            .filter
            .advanced_policy
            .as_deref()
            .is_some_and(|value| value.contains("drop-private-ip"))
    );
    assert!(saved.document.utf8_text().unwrap().contains("udp: true"));
    let loaded = ProfileOptionsApplication::new(profiles.clone())
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
    execute_core_output_command(
        &commands,
        CommandIntent::SaveSubscriptionFilter {
            workspace_identity: saved.identity(),
            filter: SubscriptionFilterDraft {
                advanced_policy: Some("{}".into()),
                ..Default::default()
            },
        },
    )
    .await
    .unwrap();
    assert!(
        ProfileOptionsApplication::new(profiles)
            .load(Some("main"))
            .await
            .unwrap()
            .filter
            .advanced_policy
            .is_none(),
        "an explicitly cleared policy removes the stored filter"
    );
}
