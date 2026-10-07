//! Draft validation, observed operation high-water marks and terminal identity fences.
use super::*;
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_tracer::TracerRuleOverride;

#[test]
fn confirmation_freezes_drafts_and_operation_until_cancel() {
    let confirmation = TracerRuleOverride {
        rule_index: 1,
        rule_table: None,
        new_target: "REJECT".into(),
        expected_source: RuleSourceIdentity {
            profile: "fixture.yaml".into(),
            document_hash: "confirmed".into(),
        },
        expected_rule: "DOMAIN,example.org,DIRECT".into(),
    };
    let mut state = RuleTraceActions {
        query: "example.org".into(),
        source_ip: "192.0.2.1".into(),
        confirmation: Some(confirmation.clone()),
        ..RuleTraceActions::default()
    };
    state.set_query("blocked.test".into());
    state.set_source_ip("192.0.2.2".into());
    state.set_sandbox(TrafficField::Network, "udp".into());
    assert_eq!(state.query, "example.org");
    assert_eq!(state.source_ip, "192.0.2.1");
    assert!(state.sandbox.is_empty());
    assert_eq!(state.begin().unwrap_err().code, ErrorCode::NotReady);
    assert!(state.pending.is_none());
    let (operation, request) = state.begin_override().unwrap();
    assert_eq!(request, confirmation);
    assert!(!state.cancel_override());
    assert!(state.finish_override(
        operation,
        Err(Failure::new(ErrorCode::Permission, "denied", true))
    ));
    assert!(state.confirmation.is_some());
    assert!(state.cancel_override());
    state.set_query("next.test".into());
    assert_eq!(state.begin().unwrap().1.query, "next.test");
}
#[test]
fn allocation_advances_past_observed_identity_and_never_wraps() {
    let counter = AtomicU64::new(2);
    assert_eq!(
        allocate(&counter, 1000).unwrap(),
        RuleTraceOperationId(1000)
    );
    assert_eq!(allocate(&counter, 20).unwrap(), RuleTraceOperationId(1001));
    let exhausted_counter = AtomicU64::new(u64::MAX);
    assert_eq!(
        allocate(&exhausted_counter, 1).unwrap_err().code,
        ErrorCode::InvalidState
    );
    assert_eq!(exhausted_counter.load(Ordering::Relaxed), u64::MAX);
    let mut state = RuleTraceActions {
        query: "example.org".into(),
        snapshot: RuleTraceExecution {
            operation_id: Some(RuleTraceOperationId(3000)),
            ..RuleTraceExecution::default()
        },
        ..RuleTraceActions::default()
    };
    assert!(state.begin().unwrap().0.0 > 3000);
}
#[test]
fn all_sandbox_values_reach_one_request_and_invalid_values_do_not_submit() {
    let mut state = RuleTraceActions {
        query: "example.org".into(),
        source_ip: "192.0.2.1".into(),
        ..RuleTraceActions::default()
    };
    for (field, value) in [
        (TrafficField::DestinationIp, "192.0.2.2"),
        (TrafficField::DestinationPort, "443"),
        (TrafficField::SourcePort, "1234"),
        (TrafficField::InboundPort, "7890"),
        (TrafficField::InboundType, "mixed"),
        (TrafficField::InboundName, "listener"),
        (TrafficField::InboundUser, "alice"),
        (TrafficField::ProcessName, "curl"),
        (TrafficField::ProcessPath, "/usr/bin/curl"),
        (TrafficField::Network, "tcp"),
        (TrafficField::Dscp, "0"),
        (TrafficField::Uid, "0"),
        (TrafficField::PackageName, "org.test"),
    ] {
        state.set_sandbox(field, value.into());
    }
    let (operation, request) = state.begin().unwrap();
    assert_eq!(request.context.ip.as_deref(), Some("192.0.2.2"));
    assert_eq!(request.context.port, Some(443));
    assert_eq!(request.context.src_ip.as_deref(), Some("192.0.2.1"));
    assert_eq!(request.context.src_port, Some(1234));
    assert_eq!(request.context.in_port, Some(7890));
    assert_eq!(request.context.in_type.as_deref(), Some("mixed"));
    assert_eq!(request.context.in_name.as_deref(), Some("listener"));
    assert_eq!(request.context.in_user.as_deref(), Some("alice"));
    assert_eq!(request.context.process_name.as_deref(), Some("curl"));
    assert_eq!(
        request.context.process_path.as_deref(),
        Some("/usr/bin/curl")
    );
    assert_eq!(request.context.network.as_deref(), Some("tcp"));
    assert_eq!(request.context.dscp, Some(0));
    assert_eq!(request.context.uid, Some(0));
    assert_eq!(request.context.package_name.as_deref(), Some("org.test"));
    assert!(!state.finish(RuleTraceOperationId(operation.0 + 1), Ok(())));
    assert_eq!(state.pending, Some(operation));
    assert!(state.finish(operation, Ok(())));
    for (field, value) in [
        (TrafficField::DestinationPort, "0"),
        (TrafficField::DestinationPort, "70000"),
        (TrafficField::DestinationIp, "invalid"),
        (TrafficField::Dscp, "64"),
        (TrafficField::Network, "invented"),
    ] {
        let old = state.sandbox.clone();
        state.set_sandbox(field, value.into());
        assert_eq!(state.begin().unwrap_err().code, ErrorCode::InvalidInput);
        assert!(state.pending.is_none());
        state.sandbox = old;
    }
}
