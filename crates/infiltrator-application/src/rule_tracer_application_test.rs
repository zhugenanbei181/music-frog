use infiltrator_contract::core_identity::CoreGeneration;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::rule_tracer::DecisionNodeStatus;
use infiltrator_test_support::rule_trace_fixtures::rule_trace_test_scope;
use infiltrator_test_support::rule_trace_fixtures::unknown_rule_trace_observation;
use super::*;
use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::outbound::details::{
    OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState, OutboundEgressSample,
    OutboundHealthObservation, OutboundObservation, OutboundProbeAvailability,
    OutboundProtocolObservation, OutboundSnapshot, OutboundUdpSupport,
};
use infiltrator_contract::outbound::identity::{OutboundId, OutboundProtocolId, SnapshotRevision};
use infiltrator_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState, OutboundObservationSnapshot,
    OutboundSelectionSet, OutboundSelectionState, SelectionAvailability,
};
use infiltrator_contract::rule_condition::{ConditionIssue, TrafficField};
use infiltrator_contract::rule_trace_facts::DecisionStageFacts;
use infiltrator_contract::rule_trace_run::{
    RuleTraceOperationId, RuleTraceOutboundFacts, RuleTraceOwnerBinding, RuleTraceRequest,
    ScopedRuleTraceRequest,
};
use infiltrator_contract::rule_tracer::RuleTracerStatus;
use infiltrator_contract::rule_tracer::{
    DecisionStageKind, RuleTraceOutboundResolution, RuleTraceRuntimeMode,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use infiltrator_presentation::rule_trace_projection::present_chain;
use infiltrator_test_support::rule_workspace_test_support::rule_workspace;
use std::sync::atomic::{AtomicUsize, Ordering};

fn scoped_for_test(
    request: RuleTraceRequest,
    workspace: &RuleWorkspace,
) -> (
    CoreScope,
    ProfileWorkspaceIdentity,
    ScopedRuleTraceRequest,
) {
    let scope = rule_trace_test_scope();
    let observed_workspace = workspace.profile_identity.clone();
    let owner = RuleTraceOwnerBinding::new(scope.clone(), observed_workspace.clone())
        .expect("fixture owner engine matches workspace");
    (
        scope,
        observed_workspace,
        ScopedRuleTraceRequest { owner, request },
    )
}

async fn simulate_for_test(
    app: &RuleTracerApplication,
    operation: RuleTraceOperationId,
    request: RuleTraceRequest,
    workspace: &RuleWorkspace,
) -> Result<(), Failure> {
    let (scope, observed_workspace, request) = scoped_for_test(request, workspace);
    let outbound_observation =
        unknown_rule_trace_observation(
            scope.clone(),
        );
    app.simulate_scoped(
        &scope,
        &observed_workspace,
        operation,
        request,
        &RuleTraceOutboundFacts::observed(outbound_observation.clone()),
    )
    .await
}

async fn simulate_for_sample(
    app: &RuleTracerApplication,
    operation: RuleTraceOperationId,
    request: RuleTraceRequest,
) -> Result<(), Failure> {
    simulate_for_test(
        app,
        operation,
        request,
        &override_workspace(&sample_rules()),
    )
    .await
}

fn sample_rules() -> Vec<RuleEntry> {
    vec![
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,PROXY".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-KEYWORD,bilibili,DIRECT".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,1.1.1.1/32,DNS_DIRECT,no-resolve".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "AND,((DOMAIN,api.openai.com),(DST-PORT,443)),AI_PROXY".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,FALLBACK".to_owned(),
            enabled: true,
        },
    ]
}

fn typed_rule_trace_observation(scope: &CoreScope) -> OutboundObservationSnapshot {
    let engine = scope.identity.engine.clone();
    let collection_id = OutboundId::new(engine.clone(), "PROXY").unwrap();
    let selected_id = OutboundId::new(engine.clone(), "selected-outbound").unwrap();
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(6),
        OutboundInventoryState::Ready {
            entries: vec![OutboundSnapshot {
                id: selected_id.clone(),
                display_name: Some("Selected node".to_owned()),
            }],
            collections: vec![OutboundSelectionSet {
                id: collection_id,
                member_ids: vec![selected_id.clone()],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: selected_id.clone(),
                },
            }],
        },
    )
    .unwrap();
    let details = OutboundDetailsSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(9),
        OutboundDetailsState::Ready {
            details: vec![OutboundDetail {
                id: selected_id,
                protocol: OutboundProtocolObservation::Reported(
                    OutboundProtocolId::new(engine, "vless").unwrap(),
                ),
                protocol_display_name: Some("VLESS".to_owned()),
                cipher: OutboundObservation::Unknown,
                cipher_display_name: None,
                collection_kind: OutboundObservation::Unknown,
                features: OutboundObservation::Unknown,
                endpoint: OutboundObservation::Unknown,
                udp: OutboundUdpSupport::Unknown,
                health: OutboundHealthObservation::Unknown,
                current_latency_ms: OutboundObservation::Observed(26),
                latency_history: OutboundObservation::Unknown,
                rtt: OutboundObservation::Unknown,
                historical_egress: OutboundObservation::Observed(vec![OutboundEgressSample {
                    ip: "203.0.113.9".to_owned(),
                    country: Some("JP".to_owned()),
                    observed_at_epoch_ms: Some(1_760_000_000_001),
                }]),
                probe_availability: OutboundProbeAvailability::Unknown,
            }],
        },
    )
    .unwrap();
    OutboundObservationSnapshot::new(inventory, details).unwrap()
}

fn unknown_rule_trace_detail(id: OutboundId) -> OutboundDetail {
    OutboundDetail {
        id,
        protocol: OutboundProtocolObservation::Unknown,
        protocol_display_name: None,
        cipher: OutboundObservation::Unknown,
        cipher_display_name: None,
        collection_kind: OutboundObservation::Unknown,
        features: OutboundObservation::Unknown,
        endpoint: OutboundObservation::Unknown,
        udp: OutboundUdpSupport::Unknown,
        health: OutboundHealthObservation::Unknown,
        current_latency_ms: OutboundObservation::Unknown,
        latency_history: OutboundObservation::Unknown,
        rtt: OutboundObservation::Unknown,
        historical_egress: OutboundObservation::Unknown,
        probe_availability: OutboundProbeAvailability::Unknown,
    }
}

fn ambiguous_rule_trace_observation(scope: &CoreScope) -> OutboundObservationSnapshot {
    let engine = scope.identity.engine.clone();
    let first_id = OutboundId::new(engine.clone(), "first-node").unwrap();
    let second_id = OutboundId::new(engine.clone(), "second-node").unwrap();
    let inventory = OutboundInventorySnapshot::new(
        scope.clone(),
        SnapshotRevision::new(6),
        OutboundInventoryState::Ready {
            entries: vec![
                OutboundSnapshot {
                    id: first_id.clone(),
                    display_name: Some("Shared display name".to_owned()),
                },
                OutboundSnapshot {
                    id: second_id.clone(),
                    display_name: Some("Shared display name".to_owned()),
                },
            ],
            collections: Vec::new(),
        },
    )
    .unwrap();
    let details = OutboundDetailsSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(9),
        OutboundDetailsState::Ready {
            details: vec![
                unknown_rule_trace_detail(first_id),
                unknown_rule_trace_detail(second_id),
            ],
        },
    )
    .unwrap();
    OutboundObservationSnapshot::new(inventory, details).unwrap()
}

fn running_core() -> CoreSnapshot {
    CoreSnapshot {
        identity: None,
        lifecycle: CoreLifecycle::Running,
        generation: 1,
        revision: 2,
        session_token: None,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        watchdog: Default::default(),
    }
}

#[test]
fn tracer_empty_query_projects_empty_sandbox_with_presets() {
    let app = RuleTracerApplication::new();
    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    assert_eq!(snapshot.status, RuleTracerStatus::Empty);
    assert!(snapshot.active_query.is_empty());
    assert!(!snapshot.presets.is_empty());
    assert!(snapshot.decision_chain.is_none());
}

#[test]
fn tracer_simulates_domain_suffix_match() {
    let app = RuleTracerApplication::with_query("www.google.com");
    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    assert_eq!(snapshot.status, RuleTracerStatus::Ready);
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.hit_rule_index, Some(0));
    assert_eq!(chain.matched_rule_type, "DOMAIN-SUFFIX");
    assert_eq!(chain.target_proxy, "PROXY");
    assert_eq!(chain.nodes.len(), 5);
    assert_eq!(chain.nodes[0].stage, DecisionStageKind::Inbound);
    assert_eq!(chain.nodes[2].stage, DecisionStageKind::RuleSet);
    assert!(!chain.is_fallback);
}

#[test]
fn tracer_simulates_nested_logical_subrule() {
    let app = RuleTracerApplication::with_query("api.openai.com:443");
    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "AI_PROXY");
    assert_eq!(chain.matched_rule_type, "AND");
    assert!(
        present_chain(&chain, "en-US").nodes[2]
            .sub_evaluations
            .iter()
            .any(|e| e.contains("Matched"))
    );
}

#[test]
fn tracer_simulates_ip_cidr_match() {
    let app = RuleTracerApplication::with_query("1.1.1.1:53");
    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "DNS_DIRECT");
    assert_eq!(chain.matched_rule_type, "IP-CIDR");
}

#[test]
fn tracer_fallback_when_unmatched() {
    let app = RuleTracerApplication::with_query("unknown-domain.xyz");
    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "FALLBACK");
    assert_eq!(chain.matched_rule_type, "MATCH");
    assert!(chain.is_fallback);
}

#[test]
fn tracer_supports_offline_ast_trace_when_core_stopped() {
    let mut core = running_core();
    core.lifecycle = CoreLifecycle::Stopped;
    let app = RuleTracerApplication::with_query("bilibili.com");
    let snapshot = app
        .simulate_snapshot(&core, &sample_rules())
        .expect("complete sandbox");
    assert_eq!(snapshot.status, RuleTracerStatus::Ready);
    assert_eq!(snapshot.runtime_mode, RuleTraceRuntimeMode::Offline);
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "DIRECT");
}

#[test]
fn hit_audit_reports_real_counts_dead_rules_and_last_hit() {
    use infiltrator_contract::rule_hit_audit::RuleDeadReason;

    let app = RuleTracerApplication::new();
    app.record_hits(
        &sample_rules(),
        &[
            ("DOMAIN-SUFFIX,google.com,PROXY", 2048),
            ("DOMAIN-SUFFIX,google.com,PROXY", 1024),
            ("DOMAIN-KEYWORD,bilibili,DIRECT", 512),
        ],
    );

    let audit = app.audit(&sample_rules());

    assert_eq!(audit.total_hits, 3);
    assert!(audit.can_clear);
    // Highest-count rule is first; payload bytes accumulate across hits.
    assert_eq!(audit.top_hits[0].rule_raw, "DOMAIN-SUFFIX,google.com,PROXY");
    assert_eq!(audit.top_hits[0].hit_count, 2);
    assert_eq!(audit.top_hits[0].total_payload_bytes, Some(3072));
    assert!(audit.top_hits[0].last_hit_secs.is_some());
    // The most recent timestamp drives the hit-flash highlight; ties are
    // broken toward the higher-hit rule so the projection is deterministic.
    assert_eq!(
        audit.last_hit_rule.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,PROXY")
    );
    // Untouched enabled rules are honestly reported as zero-hit.
    assert!(audit.dead_rules.iter().any(|entry| {
        entry.rule_raw == "IP-CIDR,1.1.1.1/32,DNS_DIRECT,no-resolve"
            && entry.reason == RuleDeadReason::ZeroHits
            && entry.hit_count == 0
    }));
}

#[test]
fn hit_audit_surfaces_cidr_overlap_and_shadow_reason() {
    use infiltrator_contract::rule_hit_audit::RuleDeadReason;

    let app = RuleTracerApplication::new();
    let rules = vec![
        RuleEntry {
            rule: "IP-CIDR,10.0.0.0/8,DIRECT".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,10.1.2.0/24,PROXY".to_owned(),
            enabled: true,
        },
    ];

    let audit = app.audit(&rules);
    assert_eq!(audit.cidr_overlaps.len(), 1);
    let overlap = &audit.cidr_overlaps[0];
    assert_eq!(overlap.rule_raw, "IP-CIDR,10.1.2.0/24,PROXY");
    assert_eq!(overlap.reason, RuleDeadReason::Shadowed);
    assert_eq!(
        overlap.shadowed_by.as_deref(),
        Some("IP-CIDR,10.0.0.0/8,DIRECT")
    );
    assert!(
        audit
            .dead_rules
            .iter()
            .any(|entry| entry.rule_raw == overlap.rule_raw)
    );
}

#[test]
fn hit_audit_publishes_trace_latency_contribution() {
    let app = RuleTracerApplication::new();
    let rules = sample_rules();
    let first = app
        .trace(&rules, "www.google.com")
        .expect("first simulation")
        .1;
    let second = app
        .trace(&rules, "bilibili.com")
        .expect("second simulation")
        .1;

    let audit = app.audit(&rules);
    assert_eq!(audit.trace_count, 2);
    assert_eq!(
        audit.avg_match_latency_us,
        Some((first.match_latency_us + second.match_latency_us) as f64 / 2.0)
    );
    assert_eq!(audit.last_match_latency_us, Some(second.match_latency_us));
}

#[tokio::test]
async fn clear_hits_resets_the_audit() {
    let app = RuleTracerApplication::new();
    let rules = sample_rules();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(rules.clone())));
    simulate_for_test(
        &app,
        RuleTraceOperationId(1),
        RuleTraceRequest {
            query: "www.google.com".into(),
            context: TrafficContextSnapshot::default(),
        },
        &override_workspace(&rules),
    )
    .await
    .unwrap();
    let source = app.execution().report.unwrap().source.unwrap();
    assert_eq!(app.audit(&sample_rules()).total_hits, 1);
    app.clear_hits(&source).await.unwrap();
    let audit = app.audit(&sample_rules());
    assert_eq!(audit.total_hits, 0);
    assert!(!audit.can_clear);
    assert!(audit.last_hit_rule.is_none());
}

#[tokio::test]
async fn project_publishes_the_hit_audit_for_the_surface() {
    use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
    let app = RuleTracerApplication::new();
    let rules = sample_rules();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(rules.clone())));
    simulate_for_test(
        &app,
        RuleTraceOperationId(1),
        RuleTraceRequest {
            query: "www.google.com".into(),
            context: TrafficContextSnapshot::default(),
        },
        &override_workspace(&rules),
    )
    .await
    .unwrap();
    let report = app.execution().report.unwrap();
    let readout = app
        .statistics_for(report.source.as_ref().unwrap(), &rules)
        .unwrap();
    assert_eq!(readout.audit.total_hits, 1);
    assert_eq!(
        readout.audit.last_hit_rule.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,PROXY")
    );
    assert_eq!(readout.row_count(0), 1);
    assert_eq!(readout.audit.top_hits[0].total_payload_bytes, None);
}

#[tokio::test]
async fn tracer_port_drives_the_shared_query_state() {
    let app = RuleTracerApplication::new();
    let port = app.clone();
    port.set_query("www.google.com");
    assert_eq!(app.query(), "www.google.com");

    let chain = port
        .trace(&sample_rules(), "www.google.com")
        .expect("complete sandbox")
        .1;
    assert_eq!(chain.hit_rule_index, Some(0));
    assert_eq!(chain.matched_rule_type, "DOMAIN-SUFFIX");
    // No runtime exit facts were supplied, so the outbound stage must not
    // fabricate a node name or latency.
    assert!(chain.final_outbound.is_empty());
    assert_eq!(chain.final_node_protocol, None);
    assert_eq!(chain.final_node_delay_ms, None);

    app.set_override_port(Arc::new(FakeOverridePort::with_rules(sample_rules())));
    simulate_for_test(
        &port,
        RuleTraceOperationId(1),
        RuleTraceRequest {
            query: "example.org".into(),
            context: TrafficContextSnapshot::default(),
        },
        &override_workspace(&sample_rules()),
    )
    .await
    .unwrap();
    let source = app.execution().report.unwrap().source.unwrap();
    assert_eq!(app.hit_count_for("MATCH,FALLBACK"), 1);
    port.clear_hits(&source).await.unwrap();
    assert_eq!(app.hit_count_for("MATCH,FALLBACK"), 0);
}

#[test]
fn tracer_stored_source_ip_drives_inbound_stage_and_context() {
    let app = RuleTracerApplication::with_query("www.google.com");
    app.set_context(&TrafficContextSnapshot {
        src_ip: Some("10.20.30.40".to_owned()),
        src_port: Some(54321),
        in_port: Some(7891),
        ..TrafficContextSnapshot::default()
    });

    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    assert_eq!(
        snapshot.simulated_context.src_ip.as_deref(),
        Some("10.20.30.40")
    );
    assert_eq!(snapshot.simulated_context.src_port, Some(54321));
    assert_eq!(snapshot.simulated_context.in_port, Some(7891));

    let chain = snapshot.decision_chain.expect("decision chain");
    let displayed = present_chain(&chain, "zh-CN");
    let inbound = &displayed.nodes[0];
    assert_eq!(inbound.stage, DecisionStageKind::Inbound);
    assert!(inbound.detail.contains("10.20.30.40"));
    assert!(inbound.detail.contains("7891"));
    let Some(DecisionStageFacts::Inbound(context)) = &inbound.facts else {
        panic!("typed inbound facts");
    };
    assert_eq!(context.src_ip.as_deref(), Some("10.20.30.40"));
    assert_eq!(context.src_port, Some(54321));
    assert_eq!(context.in_port, Some(7891));
    assert_eq!(context.network, None);
    assert_eq!(context.in_type, None);
}

#[test]
fn tracer_stored_source_ip_drives_src_ip_cidr_rule_match() {
    let rules = vec![
        RuleEntry {
            rule: "SRC-IP-CIDR,10.0.0.0/8,DIRECT".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,FALLBACK".to_owned(),
            enabled: true,
        },
    ];

    let app = RuleTracerApplication::with_query("example.org");
    let baseline = app
        .simulate_snapshot(&running_core(), &rules)
        .expect_err("missing source IP");
    assert_eq!(
        baseline.issue,
        ConditionIssue::MissingInput(TrafficField::SourceIp)
    );

    app.set_context(&TrafficContextSnapshot {
        src_ip: Some("10.1.2.3".to_owned()),
        ..TrafficContextSnapshot::default()
    });
    let snapshot = app
        .simulate_snapshot(&running_core(), &rules)
        .expect("complete sandbox");
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.matched_rule_type, "SRC-IP-CIDR");
    assert_eq!(chain.target_proxy, "DIRECT");
}

#[test]
fn tracer_port_set_context_updates_shared_environment() {
    let app = RuleTracerApplication::new();
    let port = app.clone();
    port.set_query("www.google.com");
    port.set_context(&TrafficContextSnapshot {
        src_ip: Some("192.168.1.77".to_owned()),
        ..TrafficContextSnapshot::default()
    });
    assert_eq!(app.context().src_ip.as_deref(), Some("192.168.1.77"));

    let chain = port
        .trace(&sample_rules(), "www.google.com")
        .expect("complete sandbox")
        .1;
    assert!(
        present_chain(&chain, "zh-CN").nodes[0]
            .detail
            .contains("192.168.1.77")
    );

    let snapshot = app
        .simulate_snapshot(&running_core(), &sample_rules())
        .expect("complete sandbox");
    assert_eq!(
        snapshot.simulated_context.src_ip.as_deref(),
        Some("192.168.1.77")
    );
}

// ---- DUAL-12-08 reverse-apply -----------------------------------------

#[derive(Default)]
struct FakeOverridePort {
    rules: Arc<Mutex<Vec<RuleEntry>>>,
    applied: Arc<Mutex<Vec<Vec<RuleEntry>>>>,
    load_calls: AtomicUsize,
    fail_apply: bool,
    source_changed_at_commit: bool,
    failure: Option<PortError>,
}

impl FakeOverridePort {
    fn with_rules(rules: Vec<RuleEntry>) -> Self {
        Self {
            rules: Arc::new(Mutex::new(rules)),
            applied: Arc::new(Mutex::new(Vec::new())),
            load_calls: AtomicUsize::new(0),
            fail_apply: false,
            source_changed_at_commit: false,
            failure: None,
        }
    }

    fn failing_apply(rules: Vec<RuleEntry>) -> Self {
        Self {
            fail_apply: true,
            ..Self::with_rules(rules)
        }
    }
}

#[async_trait::async_trait]
impl RuleOverridePort for FakeOverridePort {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        self.load_calls.fetch_add(1, Ordering::SeqCst);
        Ok(override_workspace(&self.rules.lock().expect("rules lock")))
    }

    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        let mut rules = self.rules.lock().expect("rules lock");
        if self.source_changed_at_commit
            || override_workspace(&rules).source != expected.source
            || *rules != expected.rules
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Source changed",
                true,
            )));
        }
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        if self.fail_apply {
            return Err(PortError::Failed(
                "apply transaction rolled back".to_owned(),
            ));
        }
        self.applied
            .lock()
            .expect("applied lock")
            .push(entries.to_vec());
        *rules = entries.to_vec();
        Ok(())
    }
}

fn override_workspace(rules: &[RuleEntry]) -> RuleWorkspace {
    let raw: Vec<&str> = rules.iter().map(|entry| entry.rule.as_str()).collect();
    let yaml = format!(
        "proxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\nrules:\n{}",
        serde_yaml_ng::to_string(&raw).expect("rules YAML")
    );
    let mut workspace = rule_workspace("test.yaml".to_owned(), &yaml).expect("test workspace");
    workspace.rules = rules.to_vec();
    workspace
}

fn override_request(rule_index: usize, target: &str) -> TracerRuleOverride {
    let workspace = override_workspace(&sample_rules());
    TracerRuleOverride {
        rule_index,
        rule_table: None,
        new_target: target.to_owned(),
        expected_source: workspace.source,
        expected_rule: workspace
            .rules
            .get(rule_index)
            .map(|entry| entry.rule.clone())
            .unwrap_or_else(|| "DOMAIN,removed.example,DIRECT".to_owned()),
    }
}


#[path = "rule_tracer_application_test/draft_tests.rs"]
mod draft_tests;

#[path = "rule_tracer_application_test/execution.rs"]
mod execution;

#[path = "rule_tracer_application_test/named.rs"]
mod named;
