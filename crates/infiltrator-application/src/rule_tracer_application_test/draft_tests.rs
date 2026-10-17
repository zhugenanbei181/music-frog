use super::*;

#[tokio::test]
async fn d12_08_override_rewrites_the_rule_and_applies_it() {
    let app = RuleTracerApplication::new();
    let port = Arc::new(FakeOverridePort::with_rules(sample_rules()));
    app.set_override_port(port.clone());

    let result = app.apply_override(&override_request(0, "DIRECT")).await;

    assert!(result.is_applied());
    assert_eq!(
        result.previous_rule_raw.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,PROXY")
    );
    assert_eq!(
        result.updated_rule_raw.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,DIRECT")
    );
    // The rewritten list was committed through the port exactly once.
    assert_eq!(port.applied.lock().expect("applied lock").len(), 1);
    let persisted = port
        .load_rule_workspace()
        .await
        .expect("persisted rules")
        .rules;
    assert_eq!(persisted[0].rule, "DOMAIN-SUFFIX,google.com,DIRECT");
    // Untouched rules keep their outbound.
    assert_eq!(persisted[1].rule, "DOMAIN-KEYWORD,bilibili,DIRECT");
}

#[tokio::test]
async fn d12_08_override_reports_unsupported_without_a_composed_port() {
    let app = RuleTracerApplication::new();
    let result = app.apply_override(&override_request(0, "DIRECT")).await;
    assert_eq!(result.status, TracerRuleOverrideStatus::Unsupported);
    assert!(result.failure_message().is_some());
}

#[tokio::test]
async fn d12_08_override_rejects_blank_and_stale_and_failed_paths() {
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(sample_rules())));

    let blank = app.apply_override(&override_request(0, "   ")).await;
    assert_eq!(blank.status, TracerRuleOverrideStatus::InvalidTarget);

    let comma = app
        .apply_override(&override_request(0, "PROXY,DIRECT"))
        .await;
    assert_eq!(comma.status, TracerRuleOverrideStatus::InvalidTarget);

    let stale = app.apply_override(&override_request(99, "DIRECT")).await;
    assert_eq!(stale.status, TracerRuleOverrideStatus::StaleRuleIndex);
}

#[tokio::test]
async fn d12_08_override_surfaces_apply_transaction_failure() {
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(FakeOverridePort::failing_apply(sample_rules())));

    let result = app.apply_override(&override_request(0, "REJECT")).await;
    assert_eq!(result.status, TracerRuleOverrideStatus::ApplyFailed);
    let message = result.failure_message().unwrap_or("<none>");
    assert!(
        message.contains("rolled back"),
        "unexpected failure message: {message}"
    );
}

#[tokio::test]
async fn d12_08_port_apply_override_delegates_to_the_shared_handler() {
    let app = RuleTracerApplication::new();
    let port = Arc::new(FakeOverridePort::with_rules(sample_rules()));
    app.set_override_port(port);
    let surface_port = app.clone();

    let result = surface_port
        .apply_override(&override_request(1, "PROXY"))
        .await;
    assert!(result.is_applied());
    assert_eq!(
        result.updated_rule_raw.as_deref(),
        Some("DOMAIN-KEYWORD,bilibili,PROXY")
    );
}

#[tokio::test]
async fn reverse_apply_rejects_changed_profile_bytes_rule_and_unknown_target_without_writes() {
    let app = RuleTracerApplication::new();
    let original = sample_rules();
    let port = Arc::new(FakeOverridePort::with_rules(original.clone()));
    app.set_override_port(port.clone());
    let mut different_profile = override_request(0, "DIRECT");
    different_profile.expected_source.profile = "other.yaml".to_owned();
    let mut different_bytes = override_request(0, "DIRECT");
    different_bytes.expected_source.document_hash = "old-document".to_owned();
    let mut changed_rule = override_request(0, "DIRECT");
    changed_rule.expected_rule = "DOMAIN-SUFFIX,google.com,REJECT".to_owned();
    for request in [different_profile, different_bytes, changed_rule] {
        let result = app.apply_override(&request).await;
        assert_eq!(result.status, TracerRuleOverrideStatus::StaleRuleIndex);
        let failure = result.into_failure().expect("source mismatch failure");
        assert_eq!(failure.code, ErrorCode::NotReady);
        assert!(failure.retryable);
    }
    let unknown = app
        .apply_override(&override_request(0, "UNCONFIGURED"))
        .await;
    assert_eq!(unknown.status, TracerRuleOverrideStatus::InvalidTarget);
    assert_eq!(
        unknown.into_failure().expect("invalid target").code,
        ErrorCode::InvalidInput
    );
    assert!(port.applied.lock().expect("writes lock").is_empty());
    assert_eq!(*port.rules.lock().expect("rules lock"), original);
}

#[tokio::test]
async fn reverse_apply_rechecks_commit_source_and_preserves_typed_failures() {
    let original = sample_rules();
    let app = RuleTracerApplication::new();
    let race = Arc::new(FakeOverridePort {
        source_changed_at_commit: true,
        ..FakeOverridePort::with_rules(original.clone())
    });
    app.set_override_port(race.clone());
    let result = app.apply_override(&override_request(0, "DIRECT")).await;
    assert_eq!(
        result.into_failure().expect("concurrent modification").code,
        ErrorCode::NotReady
    );
    assert!(race.applied.lock().expect("writes lock").is_empty());
    assert_eq!(*race.rules.lock().expect("rules lock"), original);
    for code in [
        ErrorCode::Permission,
        ErrorCode::Authentication,
        ErrorCode::Storage,
    ] {
        let failure = Failure::new(code, "Host rejected this transaction", true);
        let host = Arc::new(FakeOverridePort {
            failure: Some(PortError::Rejected(failure.clone())),
            ..FakeOverridePort::with_rules(original.clone())
        });
        app.set_override_port(host.clone());
        let result = app.apply_override(&override_request(0, "REJECT")).await;
        assert_eq!(result.into_failure(), Some(failure));
        assert!(host.applied.lock().expect("writes lock").is_empty());
        assert_eq!(*host.rules.lock().expect("rules lock"), original);
    }
}

#[tokio::test]
async fn typed_inventory_resolves_group_selection_and_details_by_outbound_id() {
    let app = RuleTracerApplication::new();
    let rules = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(rules.clone()));
    app.set_override_port(host.clone());
    let workspace = override_workspace(&rules);
    let (scope, observed_workspace, request) = scoped_for_test(
        RuleTraceRequest {
            query: "www.google.com".to_owned(),
            context: TrafficContextSnapshot::default(),
        },
        &workspace,
    );
    let observation = typed_rule_trace_observation(&scope);
    app.simulate_scoped(
        &scope,
        &observed_workspace,
        RuleTraceOperationId(1),
        request,
        &RuleTraceOutboundFacts::observed(observation),
    )
    .await
    .unwrap();
    let execution = app.execution();
    let report = execution.report.clone().unwrap();
    assert_eq!(report.generation, scope.generation.value());
    assert_eq!(report.revision, execution.revision);
    assert_eq!(report.runtime_mode, RuleTraceRuntimeMode::Live);
    assert_eq!(
        report.outbound_resolution,
        RuleTraceOutboundResolution::Resolved {
            outbound_id: OutboundId::new(scope.identity.engine.clone(), "selected-outbound",)
                .unwrap(),
            inventory_revision: SnapshotRevision::new(6),
            details_revision: SnapshotRevision::new(9),
        }
    );
    let chain = report.decision_chain.unwrap();
    assert_eq!(chain.matched_rule_raw, rules[0].rule);
    assert_eq!(chain.final_outbound, "Selected node");
    assert_eq!(chain.final_node_protocol.as_deref(), Some("VLESS"));
    assert_eq!(chain.final_node_delay_ms, Some(26));
    assert_eq!(chain.final_node_country.as_deref(), Some("JP"));
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn typed_unknown_outbound_facts_keep_the_outbound_stage_neutral() {
    let app = RuleTracerApplication::new();
    let rules = sample_rules();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(rules.clone())));
    let workspace = override_workspace(&rules);
    simulate_for_test(
        &app,
        RuleTraceOperationId(1),
        RuleTraceRequest {
            query: "www.google.com".to_owned(),
            context: TrafficContextSnapshot::default(),
        },
        &workspace,
    )
    .await
    .unwrap();
    let report = app.execution().report.unwrap();
    assert_eq!(report.runtime_mode, RuleTraceRuntimeMode::Live);
    assert_eq!(
        report.outbound_resolution,
        RuleTraceOutboundResolution::Unknown
    );
    let chain = report.decision_chain.unwrap();
    let outbound = chain
        .nodes
        .iter()
        .find(|node| node.stage == DecisionStageKind::Outbound)
        .unwrap();
    assert_eq!(outbound.status, DecisionNodeStatus::Neutral);
    assert!(chain.final_outbound.is_empty());
    assert_eq!(chain.final_node_protocol, None);
    assert_eq!(chain.final_node_delay_ms, None);
}

#[tokio::test]
async fn no_session_keeps_local_trace_explicitly_offline_with_unavailable_outbounds() {
    let rules = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(rules.clone()));
    let app = RuleTracerApplication::new();
    app.set_override_port(host.clone());
    let workspace = override_workspace(&rules);
    let mut scope = rule_trace_test_scope();
    scope.session_token = None;
    scope.generation = CoreGeneration::new(7);
    let observed_workspace = workspace.profile_identity.clone();
    let request = ScopedRuleTraceRequest {
        owner: RuleTraceOwnerBinding::new(scope.clone(), observed_workspace.clone()).unwrap(),
        request: RuleTraceRequest {
            query: "www.google.com".to_owned(),
            context: TrafficContextSnapshot::default(),
        },
    };
    let offline_failure = Failure::new(
        ErrorCode::NotReady,
        "Core session is unavailable; local AST simulation only",
        true,
    );

    app.simulate_scoped(
        &scope,
        &observed_workspace,
        RuleTraceOperationId(1),
        request,
        &RuleTraceOutboundFacts::Unavailable(offline_failure.clone()),
    )
    .await
    .unwrap();

    let execution = app.execution();
    let report = execution.report.unwrap();
    assert_eq!(report.generation, 7);
    assert_eq!(report.revision, execution.revision);
    assert_eq!(report.status, RuleTracerStatus::Ready);
    assert_eq!(report.runtime_mode, RuleTraceRuntimeMode::Offline);
    assert!(report.failure.is_none());
    assert!(execution.failure.is_none());
    assert_eq!(
        report.outbound_resolution,
        RuleTraceOutboundResolution::Unavailable {
            failure: offline_failure,
        }
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn live_session_rejects_unavailable_outbound_read_before_profile_adapter() {
    let rules = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(rules.clone()));
    let app = RuleTracerApplication::new();
    app.set_override_port(host.clone());
    let workspace = override_workspace(&rules);
    let (scope, observed_workspace, request) = scoped_for_test(
        RuleTraceRequest {
            query: "www.google.com".to_owned(),
            context: TrafficContextSnapshot::default(),
        },
        &workspace,
    );
    let observation_failure = Failure::new(
        ErrorCode::Permission,
        "Outbound observation was denied",
        true,
    );

    assert_eq!(
        app.simulate_scoped(
            &scope,
            &observed_workspace,
            RuleTraceOperationId(1),
            request,
            &RuleTraceOutboundFacts::Unavailable(observation_failure.clone()),
        )
        .await
        .unwrap_err(),
        observation_failure
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 0);
    assert_eq!(app.execution(), RuleTraceExecution::default());
}

#[tokio::test]
async fn no_session_rejects_live_outbound_observation_before_profile_adapter() {
    let rules = sample_rules();
    let host = Arc::new(FakeOverridePort::with_rules(rules.clone()));
    let app = RuleTracerApplication::new();
    app.set_override_port(host.clone());
    let workspace = override_workspace(&rules);
    let mut scope = rule_trace_test_scope();
    scope.session_token = None;
    let observed_workspace = workspace.profile_identity.clone();
    let request = ScopedRuleTraceRequest {
        owner: RuleTraceOwnerBinding::new(scope.clone(), observed_workspace.clone()).unwrap(),
        request: RuleTraceRequest {
            query: "www.google.com".to_owned(),
            context: TrafficContextSnapshot::default(),
        },
    };
    let observation = unknown_rule_trace_observation(scope.clone());

    assert_eq!(
        app.simulate_scoped(
            &scope,
            &observed_workspace,
            RuleTraceOperationId(1),
            request,
            &RuleTraceOutboundFacts::observed(observation),
        )
        .await
        .unwrap_err()
        .code,
        ErrorCode::NotReady
    );
    assert_eq!(host.load_calls.load(Ordering::SeqCst), 0);
    assert_eq!(app.execution(), RuleTraceExecution::default());
}

#[tokio::test]
async fn builtin_direct_and_reject_targets_do_not_require_outbound_observation() {
    for rule in ["DOMAIN,example.org,DIRECT", "DOMAIN,example.org,REJECT"] {
        let rules = vec![RuleEntry {
            rule: rule.to_owned(),
            enabled: true,
        }];
        let workspace = override_workspace(&rules);
        let app = RuleTracerApplication::new();
        app.set_override_port(Arc::new(FakeOverridePort::with_rules(rules.clone())));
        simulate_for_test(
            &app,
            RuleTraceOperationId(1),
            RuleTraceRequest {
                query: "example.org".to_owned(),
                context: TrafficContextSnapshot::default(),
            },
            &workspace,
        )
        .await
        .unwrap();
        let report = app.execution().report.unwrap();
        assert_eq!(report.runtime_mode, RuleTraceRuntimeMode::Live);
        assert_eq!(
            report.outbound_resolution,
            RuleTraceOutboundResolution::NotRequired
        );
    }
}

#[tokio::test]
async fn ambiguous_typed_display_name_keeps_rule_trace_outbound_unknown() {
    let scope = rule_trace_test_scope();
    let observation = ambiguous_rule_trace_observation(&scope);
    assert!(super::resolve_rule_target("Shared display name", &observation).is_none());
    let rules = vec![RuleEntry {
        rule: "DOMAIN,example.org,Shared display name".to_owned(),
        enabled: true,
    }];
    let workspace = override_workspace(&rules);
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(rules)));
    let observed_workspace = workspace.profile_identity.clone();
    let request = ScopedRuleTraceRequest {
        owner: RuleTraceOwnerBinding::new(scope.clone(), observed_workspace.clone()).unwrap(),
        request: RuleTraceRequest {
            query: "example.org".to_owned(),
            context: TrafficContextSnapshot::default(),
        },
    };
    app.simulate_scoped(
        &scope,
        &observed_workspace,
        RuleTraceOperationId(1),
        request,
        &RuleTraceOutboundFacts::observed(observation),
    )
    .await
    .unwrap();
    let report = app.execution().report.unwrap();
    assert_eq!(
        report.outbound_resolution,
        RuleTraceOutboundResolution::Unknown
    );
    assert!(report.decision_chain.unwrap().final_outbound.is_empty());
}
