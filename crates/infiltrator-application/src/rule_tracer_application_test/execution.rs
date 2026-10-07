//! Explicit simulation and immutable-reader behavior with real source adapters.
use super::*;
use futures_util::poll;
use infiltrator_contract::rule_trace_run::{
    RuleTraceOperation, RuleTraceOperationId, RuleTraceRequest,
};
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
    app.simulate(
        RuleTraceOperationId(1),
        request("www.google.com", Some("10.20.30.40")),
        None,
    )
    .await
    .expect("simulation");
    let execution = app.execution();
    assert_eq!(execution.operation, RuleTraceOperation::Completed);
    assert_eq!(execution.report_id, Some(RuleTraceOperationId(1)));
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
    app.simulate(
        RuleTraceOperationId(1),
        request("www.google.com", None),
        None,
    )
    .await
    .expect("baseline");
    let baseline = app.execution();
    app.set_override_port(Arc::new(PendingSource));
    {
        let mut running =
            Box::pin(app.simulate(RuleTraceOperationId(2), request("bilibili.com", None), None));
        assert!(matches!(poll!(running.as_mut()), Poll::Pending));
        let observed = app.execution();
        assert_eq!(observed.operation, RuleTraceOperation::Running);
        assert_eq!(observed.report, baseline.report);
        assert_eq!(
            app.simulate(RuleTraceOperationId(3), request("example.org", None), None)
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
        app.simulate(RuleTraceOperationId(2), request("bilibili.com", None), None)
            .await
            .expect_err("replayed identity")
            .code,
        ErrorCode::InvalidState
    );
    app.simulate(RuleTraceOperationId(3), request("bilibili.com", None), None)
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
            app.simulate(RuleTraceOperationId(1), request, None)
                .await
                .expect_err("invalid sandbox")
                .code,
            ErrorCode::InvalidInput
        );
        assert_eq!(app.execution(), RuleTraceExecution::default());
    }
    let unavailable = RuleTracerApplication::new();
    assert_eq!(
        unavailable
            .simulate(RuleTraceOperationId(1), request("example.org", None), None)
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
