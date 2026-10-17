//! Explicit simulation and immutable-reader behavior with real source adapters.
use super::*;
use futures_util::poll;
use infiltrator_contract::profile_document_identity::ProfileDocumentRevision;
use infiltrator_contract::rule_trace_run::RuleTraceOutboundFacts;
use infiltrator_contract::rule_trace_run::RuleTraceOwnerBinding;
use infiltrator_contract::rule_trace_run::RuleTraceOwnerObservation;
use infiltrator_contract::rule_trace_run::ScopedRuleTraceRequest;
use infiltrator_contract::rule_trace_run::{
    RuleTraceOperation, RuleTraceOperationId, RuleTraceRequest,
};
use infiltrator_test_support::rule_trace_fixtures::rule_trace_test_scope;
use infiltrator_test_support::rule_trace_fixtures::unknown_rule_trace_observation;
use lurker_core_contract::identity::CoreGeneration;
use lurker_core_contract::identity::EngineId;
use std::future::pending;
use std::task::Poll;

fn request(query: &str, source_ip: Option<&str>) -> RuleTraceRequest {
    RuleTraceRequest {
        query: query.to_owned(),
        context: TrafficContextSnapshot {
            src_ip: source_ip.map(str::to_owned),
            ..TrafficContextSnapshot::default()
        },
    }
}
struct PendingSource;
#[async_trait::async_trait]
impl RuleOverridePort for PendingSource {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        pending().await
    }
    async fn compare_and_apply_rules(
        &self,
        _: &RuleWorkspace,
        _: &[RuleEntry],
    ) -> Result<(), PortError> {
        panic!("Simulation must never write rules")
    }
}

#[tokio::test]
async fn rule_simulation_replay_does_not_recompute_or_rebind_to_another_document() {
    let app = RuleTracerApplication::new();
    let original = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(original.clone()));
    app.set_override_port(host.clone());
    simulate_for_sample(
        &app,
        RuleTraceOperationId(1),
        request("www.google.com", Some("10.20.30.40")),
    )
    .await
    .expect("simulation");
    let execution = app.execution();
    assert_eq!(execution.operation, RuleTraceOperation::Completed);
    assert_eq!(execution.report_id, Some(RuleTraceOperationId(1)));
    let owner = RuleTraceOwnerBinding::new(
        rule_trace_test_scope(),
        override_workspace(&original).profile_identity,
    )
    .unwrap();
    assert_eq!(execution.owner, RuleTraceOwnerObservation::observed(owner));
    let report = execution.report.expect("report");
    let source = report.source.expect("exact document source");
    let chain = report.decision_chain.expect("chain");
    assert_eq!(chain.matched_rule_raw, original[0].rule);
    assert_eq!(
        report.simulated_context.src_ip.as_deref(),
        Some("10.20.30.40")
    );
    for _ in 0..20 {
        let replay = app.replay(&source);
        assert_eq!(replay.decision_chain.as_ref(), Some(&chain));
        assert_eq!(
            app.statistics_for(&source, &original)
                .unwrap()
                .audit
                .trace_count,
            1
        );
        assert!(replay.can_reverse_apply);
    }
    let mut switched = source.clone();
    switched.profile = "another.yaml".to_owned();
    let stale = app.replay(&switched);
    assert!(!stale.can_reverse_apply);
    assert_eq!(stale.source.as_ref(), Some(&source));
    assert_eq!(stale.decision_chain.as_ref(), Some(&chain));
    assert_eq!(app.execution().report_id, Some(RuleTraceOperationId(1)));
    assert!(host.applied.lock().expect("writes").is_empty());
    assert_eq!(*host.rules.lock().expect("rules"), original);
}

#[tokio::test]
async fn rule_simulation_cancel_and_duplicate_requests_preserve_last_report_and_release_admission()
{
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(sample_rules())));
    simulate_for_sample(
        &app,
        RuleTraceOperationId(1),
        request("www.google.com", None),
    )
    .await
    .expect("baseline");
    let baseline = app.execution();
    app.set_override_port(Arc::new(PendingSource));
    {
        let mut running = Box::pin(simulate_for_sample(
            &app,
            RuleTraceOperationId(2),
            request("bilibili.com", None),
        ));
        assert!(matches!(poll!(running.as_mut()), Poll::Pending));
        let observed = app.execution();
        assert_eq!(observed.operation, RuleTraceOperation::Running);
        assert_eq!(observed.report, baseline.report);
        assert_eq!(
            simulate_for_sample(&app, RuleTraceOperationId(3), request("example.org", None),)
                .await
                .expect_err("no overlap")
                .code,
            ErrorCode::NotReady
        );
        assert_eq!(app.execution().operation_id, Some(RuleTraceOperationId(2)));
    }
    let canceled = app.execution();
    assert_eq!(
        canceled.failure.expect("cancellation").code,
        ErrorCode::Canceled
    );
    assert_eq!(canceled.report, baseline.report);
    assert_eq!(canceled.report_id, baseline.report_id);
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(sample_rules())));
    assert_eq!(
        simulate_for_sample(&app, RuleTraceOperationId(2), request("bilibili.com", None),)
            .await
            .expect_err("replayed identity")
            .code,
        ErrorCode::InvalidState
    );
    simulate_for_sample(&app, RuleTraceOperationId(3), request("bilibili.com", None))
        .await
        .expect("retry");
    let retried = app.execution();
    assert_eq!(retried.operation, RuleTraceOperation::Completed);
    assert_eq!(retried.report_id, Some(RuleTraceOperationId(3)));
    assert_eq!(
        app.statistics_for(
            retried.report.as_ref().unwrap().source.as_ref().unwrap(),
            &sample_rules()
        )
        .unwrap()
        .audit
        .trace_count,
        2
    );
}

#[tokio::test]
async fn rule_simulation_invalid_sandbox_is_rejected_before_source_read_and_missing_host_is_typed()
{
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(PendingSource));
    for request in [
        request("", None),
        request("host:70000", None),
        request("example.org", Some("invalid-ip")),
    ] {
        assert_eq!(
            simulate_for_sample(&app, RuleTraceOperationId(1), request)
                .await
                .expect_err("invalid sandbox")
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(app.execution(), RuleTraceExecution::default());
    }
    let unavailable = RuleTracerApplication::new();
    assert_eq!(
        simulate_for_sample(
            &unavailable,
            RuleTraceOperationId(1),
            request("example.org", None),
        )
        .await
        .expect_err("host missing")
        .code,
        ErrorCode::Unsupported
    );
    assert_eq!(
        unavailable.execution().operation,
        RuleTraceOperation::Unsupported
    );
    assert!(unavailable.execution().report.is_none());
}

#[tokio::test]
async fn stale_or_foreign_trace_owner_is_rejected_before_rule_adapter_read() {
    let rules = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(rules.clone()));
    let app = RuleTracerApplication::new();
    app.set_override_port(host.clone());
    let active_workspace = override_workspace(&rules);
    let active_scope = rule_trace_test_scope();
    let outbound_observation = unknown_rule_trace_observation(active_scope.clone());
    let input = request("example.org", None);

    let mut stale_scope = active_scope.clone();
    stale_scope.generation = CoreGeneration::new(2);
    let stale_scope_request = ScopedRuleTraceRequest {
        owner: RuleTraceOwnerBinding {
            scope: stale_scope,
            workspace: active_workspace.profile_identity.clone(),
        },
        request: input.clone(),
    };
    assert_eq!(
        app.simulate_scoped(
            &active_scope,
            &active_workspace.profile_identity,
            RuleTraceOperationId(1),
            stale_scope_request,
            &RuleTraceOutboundFacts::observed(outbound_observation.clone(),),
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::NotReady
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 0);
    assert_eq!(app.execution(), RuleTraceExecution::default());

    let mut stale_profile = active_workspace.profile_identity.clone();
    stale_profile.document.revision = ProfileDocumentRevision::new("stale-revision").unwrap();
    let stale_profile_request = ScopedRuleTraceRequest {
        owner: RuleTraceOwnerBinding {
            scope: active_scope.clone(),
            workspace: stale_profile,
        },
        request: input.clone(),
    };
    assert_eq!(
        app.simulate_scoped(
            &active_scope,
            &active_workspace.profile_identity,
            RuleTraceOperationId(1),
            stale_profile_request,
            &RuleTraceOutboundFacts::observed(outbound_observation.clone(),),
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::NotReady
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 0);

    let mut foreign_profile = active_workspace.profile_identity.clone();
    foreign_profile.document.engine_id = EngineId::new("sing-box").unwrap();
    let foreign_request = ScopedRuleTraceRequest {
        owner: RuleTraceOwnerBinding {
            scope: active_scope.clone(),
            workspace: foreign_profile,
        },
        request: input,
    };
    assert_eq!(
        app.simulate_scoped(
            &active_scope,
            &active_workspace.profile_identity,
            RuleTraceOperationId(1),
            foreign_request,
            &RuleTraceOutboundFacts::observed(outbound_observation.clone(),),
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::Unsupported
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 0);
    assert_eq!(app.execution(), RuleTraceExecution::default());
}

#[tokio::test]
async fn foreign_outbound_observation_is_rejected_before_rule_adapter_read() {
    let rules = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(rules.clone()));
    let app = RuleTracerApplication::new();
    app.set_override_port(host.clone());
    let active_workspace = override_workspace(&rules);
    let active_scope = rule_trace_test_scope();
    let mut foreign_scope = active_scope.clone();
    foreign_scope.generation = CoreGeneration::new(2);
    let outbound_observation = super::typed_rule_trace_observation(&foreign_scope);
    let (scope, observed_workspace, request) =
        super::scoped_for_test(request("example.org", None), &active_workspace);

    assert_eq!(
        app.simulate_scoped(
            &scope,
            &observed_workspace,
            RuleTraceOperationId(1),
            request,
            &RuleTraceOutboundFacts::observed(outbound_observation.clone(),),
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::InvalidState
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 0);
    assert_eq!(app.execution(), RuleTraceExecution::default());
}
