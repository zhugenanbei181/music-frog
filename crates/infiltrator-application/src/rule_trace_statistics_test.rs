//! test-intent: behavior
//! Real source loads and commands must isolate local statistics without reader side effects.
use super::*;
use crate::command_application::CommandApplication;
use crate::rule_trace_fixtures::{NAMED_TRACE_DOCUMENT, RuleTraceStore};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_ports::rule_tracer::RuleOverridePort;
use std::sync::atomic::Ordering;

const DOCUMENT: &str = "rules:\n  - DOMAIN,example.com,DIRECT\n  - MATCH,REJECT\n";
fn request() -> RuleTraceRequest {
    RuleTraceRequest {
        query: "example.com".into(),
        context: TrafficContextSnapshot::default(),
    }
}

#[tokio::test]
async fn source_names_and_hashes_fence_counts_latency_and_read_only_replay() {
    let store = Arc::new(RuleTraceStore::default());
    store.select_profile("one.yaml", DOCUMENT);
    let application = RuleTracerApplication::new();
    application.set_override_port(store.clone());
    let first = store.load_rule_workspace().await.unwrap();
    assert!(
        application
            .statistics_for(&first.source, &first.rules)
            .is_none()
    );
    application
        .simulate(RuleTraceOperationId(1), request(), None)
        .await
        .unwrap();
    application
        .simulate(RuleTraceOperationId(2), request(), None)
        .await
        .unwrap();
    let observed = application
        .statistics_for(&first.source, &first.rules)
        .unwrap();
    assert_eq!(observed.audit.total_hits, 2);
    assert_eq!(observed.audit.trace_count, 2);
    assert_eq!(observed.row_count(0), 2);
    assert_eq!(observed.row_count(1), 0);
    assert_eq!(observed.audit.top_hits[0].total_payload_bytes, None);
    let report = application.execution();
    for _ in 0..20 {
        assert_eq!(
            application
                .statistics_for(&first.source, &first.rules)
                .unwrap()
                .audit,
            observed.audit
        );
        application.replay(&first.source);
    }
    assert_eq!(application.execution(), report);
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 3);

    store.select_profile("two.yaml", DOCUMENT);
    let second = store.load_rule_workspace().await.unwrap();
    assert_ne!(first.source.profile, second.source.profile);
    assert_eq!(first.source.document_hash, second.source.document_hash);
    assert!(
        application
            .statistics_for(&second.source, &second.rules)
            .is_none()
    );
    assert_eq!(
        application.replay(&second.source).source,
        Some(first.source.clone())
    );
    application
        .simulate(RuleTraceOperationId(3), request(), None)
        .await
        .unwrap();
    let second_read = application
        .statistics_for(&second.source, &second.rules)
        .unwrap();
    assert_eq!(second_read.audit.total_hits, 1);
    assert_eq!(second_read.audit.trace_count, 1);
    assert_eq!(
        application
            .clear_hits(&first.source)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(
        application
            .statistics_for(&second.source, &second.rules)
            .unwrap()
            .audit,
        second_read.audit
    );

    assert!(
        application
            .statistics_for(&first.source, &first.rules)
            .is_none()
    );

    store.select_profile("two.yaml", &format!("# edited\n{DOCUMENT}"));
    let changed = store.load_rule_workspace().await.unwrap();
    assert_eq!(second.source.profile, changed.source.profile);
    assert_ne!(second.source.document_hash, changed.source.document_hash);
    assert!(
        application
            .statistics_for(&changed.source, &changed.rules)
            .is_none()
    );
    application
        .simulate(RuleTraceOperationId(4), request(), None)
        .await
        .unwrap();
    let changed_read = application
        .statistics_for(&changed.source, &changed.rules)
        .unwrap();
    assert_eq!(changed_read.audit.total_hits, 1);
    assert_eq!(changed_read.audit.trace_count, 1);
    let receipt = CommandApplication::new()
        .with_rule_tracer(application.clone())
        .execute_output(CommandIntent::ResetRuleHitCounters {
            expected_source: changed.source.clone(),
        })
        .await
        .unwrap()
        .into_statistics_reset()
        .unwrap();
    assert_eq!(receipt.source, changed.source);
    assert_eq!(receipt.removed_hits, 1);
    assert_eq!(receipt.removed_rows, 1);
    assert!(receipt.revision > changed_read.audit.revision);
    let cleared = application
        .statistics_for(&changed.source, &changed.rules)
        .unwrap();
    assert_eq!(cleared.audit.revision, receipt.revision);
    assert!(second_read.audit.revision > observed.audit.revision);
    assert!(changed_read.audit.revision > second_read.audit.revision);
    assert_eq!(cleared.row_count(0), 0);
    assert_eq!(cleared.audit.total_hits, 0);
    assert_eq!(cleared.audit.trace_count, 1);
    assert_eq!(
        cleared.audit.last_match_latency_us,
        changed_read.audit.last_match_latency_us
    );
    assert!(!cleared.audit.can_clear);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn failed_source_load_retains_same_source_statistics_without_counting_another_trace() {
    let store = Arc::new(RuleTraceStore::default());
    store.select_profile("one.yaml", DOCUMENT);
    let application = RuleTracerApplication::new();
    application.set_override_port(store.clone());
    application
        .simulate(RuleTraceOperationId(1), request(), None)
        .await
        .unwrap();
    let source = store.load_rule_workspace().await.unwrap();
    let before = application
        .statistics_for(&source.source, &source.rules)
        .unwrap()
        .audit;
    store.deny_read.store(true, Ordering::SeqCst);
    assert_eq!(
        application
            .simulate(RuleTraceOperationId(2), request(), None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(
        application
            .statistics_for(&source.source, &source.rules)
            .unwrap()
            .audit,
        before
    );
}

#[tokio::test]
async fn duplicate_raw_rules_and_named_descents_record_the_actual_root_identity_once() {
    let store = Arc::new(RuleTraceStore::default());
    store.replace("rules:\n  - '# DOMAIN,example.com,DIRECT'\n  - DOMAIN,example.com,DIRECT\n  - MATCH,REJECT\n");
    let application = RuleTracerApplication::new();
    application.set_override_port(store.clone());
    application
        .simulate(RuleTraceOperationId(1), request(), None)
        .await
        .unwrap();
    let source = store.load_rule_workspace().await.unwrap();
    let read = application
        .statistics_for(&source.source, &source.rules)
        .unwrap();
    assert_eq!(source.rules[0].rule, source.rules[1].rule);
    assert_eq!(read.row_count(0), 0);
    assert_eq!(read.row_count(1), 1);
    assert_eq!(read.audit.total_hits, 1);
    store.replace(NAMED_TRACE_DOCUMENT);
    application
        .simulate(
            RuleTraceOperationId(2),
            RuleTraceRequest {
                query: "google.com:443".into(),
                context: TrafficContextSnapshot::default(),
            },
            None,
        )
        .await
        .unwrap();
    let source = store.load_rule_workspace().await.unwrap();
    let read = application
        .statistics_for(&source.source, &source.rules)
        .unwrap();
    assert_eq!(read.row_count(0), 1);
    assert_eq!(read.row_count(1), 0);
    assert_eq!(read.audit.total_hits, 1);
    assert_eq!(read.audit.trace_count, 1);
    assert_eq!(read.audit.top_hits[0].rule_raw, source.rules[0].rule);
}
