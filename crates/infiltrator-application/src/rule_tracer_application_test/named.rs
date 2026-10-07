//! Actual profile-backed named-table simulation and source-fenced leaf transactions.
use super::*;
use crate::rule_trace_actions::RuleTraceActions;
use crate::rule_trace_fixtures::{NAMED_TRACE_DOCUMENT, RuleTraceStore};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};

#[tokio::test]
async fn named_leaf_report_confirmation_and_commit_preserve_parent_tables_and_source_fence() {
    let store = Arc::new(RuleTraceStore::default());
    store.replace(NAMED_TRACE_DOCUMENT);
    let app = RuleTracerApplication::new();
    app.set_override_port(store.clone());
    let request = RuleTraceRequest {
        query: "google.com:443".into(),
        context: TrafficContextSnapshot::default(),
    };
    app.simulate(RuleTraceOperationId(50), request.clone(), None)
        .await
        .unwrap();
    let report = app.execution().report.unwrap();
    let chain = report.decision_chain.as_ref().unwrap();
    assert_eq!(chain.hit_rule_table.as_deref(), Some("secure"));
    assert_eq!(chain.rule_path.len(), 3);
    assert_eq!(
        chain.rule_path[2].location,
        RuleLocation::named("secure".into(), 0)
    );
    assert_eq!(chain.matched_rule_raw, "DOMAIN-SUFFIX,google.com,PROXY");
    assert_eq!(chain.target_proxy, "PROXY");
    let copy = present_chain(chain, "en-US");
    assert!(
        copy.nodes[2]
            .sub_evaluations
            .iter()
            .any(|line| line.contains("Table secure rule #1"))
    );
    let mut actions = RuleTraceActions {
        query: request.query,
        snapshot: app.execution(),
        ..RuleTraceActions::default()
    };
    assert!(actions.prepare_override(&report, 0, "DIRECT".into()));
    let override_request = actions.confirmation.clone().unwrap();
    assert_eq!(override_request.rule_table.as_deref(), Some("secure"));
    assert!(actions.cancel_override());
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert!(actions.prepare_override(&report, 0, "DIRECT".into()));
    let result = app.apply_override(&override_request).await;
    assert_eq!(result.status, TracerRuleOverrideStatus::Applied);
    assert_eq!(
        store.content(),
        NAMED_TRACE_DOCUMENT.replace(
            "'DOMAIN-SUFFIX,google.com,PROXY'",
            "'DOMAIN-SUFFIX,google.com,DIRECT'"
        )
    );
    let previous = store.content();
    assert_eq!(
        app.apply_override(&override_request).await.status,
        TracerRuleOverrideStatus::StaleRuleIndex
    );
    assert_eq!(store.content(), previous);
    let workspace = rule_workspace("trace.yaml".into(), &previous).unwrap();
    assert!(!app.replay(&workspace.source).can_reverse_apply);
}
#[tokio::test]
async fn named_match_leaf_is_explicit_and_writable_while_missing_or_cyclic_tables_preserve_prior_report()
 {
    let store = Arc::new(RuleTraceStore::default());
    store.replace("rules:\n  - SUB-RULE,(DOMAIN,google.com),media\n  - MATCH,REJECT\nsub-rules:\n  media:\n    - MATCH,DIRECT\n");
    let app = RuleTracerApplication::new();
    app.set_override_port(store.clone());
    let request = RuleTraceRequest {
        query: "google.com".into(),
        context: TrafficContextSnapshot::default(),
    };
    app.simulate(RuleTraceOperationId(1), request.clone(), None)
        .await
        .unwrap();
    let report = app.execution().report.unwrap();
    assert!(!report.decision_chain.as_ref().unwrap().is_fallback);
    assert!(report.can_reverse_apply);
    store.replace("rules:\n  - SUB-RULE,(DOMAIN,google.com),missing\n  - MATCH,DIRECT\n");
    assert_eq!(
        app.simulate(RuleTraceOperationId(2), request.clone(), None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Configuration
    );
    assert_eq!(app.execution().report, Some(report.clone()));
    store.replace("rules:\n  - SUB-RULE,(DOMAIN,google.com),cycle\nsub-rules:\n  cycle:\n    - SUB-RULE,(DOMAIN,google.com),cycle\n");
    assert_eq!(
        app.simulate(RuleTraceOperationId(3), request, None)
            .await
            .unwrap_err()
            .code,
        ErrorCode::Configuration
    );
    assert_eq!(app.execution().report, Some(report));
}
