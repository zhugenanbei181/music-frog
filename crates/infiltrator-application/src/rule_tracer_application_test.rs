use super::*;
use infiltrator_contract::rule_tracer::DecisionStageKind;

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
            rule: "IP-CIDR,1.1.1.1/32,DNS_DIRECT".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "AND((DOMAIN,api.openai.com),(DST-PORT,443),AI_PROXY)".to_owned(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,FALLBACK".to_owned(),
            enabled: true,
        },
    ]
}

fn running_core() -> CoreSnapshot {
    CoreSnapshot {
        lifecycle: CoreLifecycle::Running,
        generation: 1,
        revision: 2,
        session_token: None,
        proxy_mode: None,
        core_version: None,
        sampled_at_epoch_ms: None,
        failure: None,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        watchdog: Default::default(),
    }
}

#[test]
fn tracer_empty_query_projects_ready_sandbox_with_presets() {
    let app = RuleTracerApplication::new();
    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
    assert_eq!(snapshot.status, RuleTracerStatus::Ready);
    assert!(snapshot.active_query.is_empty());
    assert!(!snapshot.presets.is_empty());
    assert!(snapshot.decision_chain.is_none());
}

#[test]
fn tracer_simulates_domain_suffix_match() {
    let app = RuleTracerApplication::with_query("www.google.com");
    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
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
    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "AI_PROXY");
    assert_eq!(chain.matched_rule_type, "AND");
    assert!(
        chain.nodes[2]
            .sub_evaluations
            .iter()
            .any(|e| e.contains("PASS"))
    );
}

#[test]
fn tracer_simulates_ip_cidr_match() {
    let app = RuleTracerApplication::with_query("1.1.1.1:53");
    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "DNS_DIRECT");
    assert_eq!(chain.matched_rule_type, "IP-CIDR");
}

#[test]
fn tracer_fallback_when_unmatched() {
    let app = RuleTracerApplication::with_query("unknown-domain.xyz");
    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
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
    let snapshot = app.project(&core, &sample_rules(), None, None);
    assert_eq!(snapshot.status, RuleTracerStatus::Ready);
    assert!(snapshot.failure.as_deref().unwrap_or("").contains("离线"));
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.target_proxy, "DIRECT");
}

#[test]
fn hit_audit_reports_real_counts_dead_rules_and_last_hit() {
    use infiltrator_contract::rule_tracer::RuleDeadReason;

    let app = RuleTracerApplication::new();
    app.record_hits(&[
        ("DOMAIN-SUFFIX,google.com,PROXY", 2048),
        ("DOMAIN-SUFFIX,google.com,PROXY", 1024),
        ("DOMAIN-KEYWORD,bilibili,DIRECT", 512),
    ]);

    let audit = app.audit(&sample_rules());

    assert_eq!(audit.total_hits, 3);
    assert!(audit.can_clear);
    // Highest-count rule is first; payload bytes accumulate across hits.
    assert_eq!(audit.top_hits[0].rule_raw, "DOMAIN-SUFFIX,google.com,PROXY");
    assert_eq!(audit.top_hits[0].hit_count, 2);
    assert_eq!(audit.top_hits[0].total_payload_bytes, 3072);
    assert!(audit.top_hits[0].last_hit_secs.is_some());
    // The most recent timestamp drives the hit-flash highlight; ties are
    // broken toward the higher-hit rule so the projection is deterministic.
    assert_eq!(
        audit.last_hit_rule.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,PROXY")
    );
    // Untouched enabled rules are honestly reported as zero-hit.
    assert!(audit.dead_rules.iter().any(|entry| {
        entry.rule_raw == "IP-CIDR,1.1.1.1/32,DNS_DIRECT"
            && entry.reason == RuleDeadReason::ZeroHits
            && entry.hit_count == 0
    }));
}

#[test]
fn hit_audit_surfaces_cidr_overlap_and_shadow_reason() {
    use infiltrator_contract::rule_tracer::RuleDeadReason;

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
    let _ = app.trace(&rules, "www.google.com", None, None);
    let _ = app.trace(&rules, "bilibili.com", None, None);

    let audit = app.audit(&rules);
    assert_eq!(audit.trace_count, 2);
    assert!(audit.avg_match_latency_us.is_some_and(|avg| avg >= 1.0));
    assert!(audit.last_match_latency_us.is_some_and(|last| last >= 1));
}

#[test]
fn clear_hits_resets_the_audit() {
    let app = RuleTracerApplication::new();
    app.record_hits(&[("DOMAIN-SUFFIX,google.com,PROXY", 64)]);
    assert_eq!(app.audit(&sample_rules()).total_hits, 1);

    app.clear_hits();
    let audit = app.audit(&sample_rules());
    assert_eq!(audit.total_hits, 0);
    assert!(!audit.can_clear);
    assert!(audit.last_hit_rule.is_none());
}

#[test]
fn project_publishes_the_hit_audit_for_the_surface() {
    let app = RuleTracerApplication::with_query("www.google.com");
    app.record_hits(&[("DOMAIN-SUFFIX,google.com,PROXY", 128)]);
    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
    assert_eq!(snapshot.hit_audit.total_hits, 1);
    assert_eq!(
        snapshot.hit_audit.last_hit_rule.as_deref(),
        Some("DOMAIN-SUFFIX,google.com,PROXY")
    );
}

#[test]
fn tracer_port_drives_the_shared_query_state() {
    let app = RuleTracerApplication::new();
    let port: &dyn infiltrator_ports::rule_tracer::RuleTracerPort = &app;
    port.set_query("www.google.com");
    assert_eq!(app.query(), "www.google.com");

    let chain = port.trace(&sample_rules(), "www.google.com", None);
    assert_eq!(chain.hit_rule_index, Some(0));
    assert_eq!(chain.matched_rule_type, "DOMAIN-SUFFIX");
    // No runtime exit facts were supplied, so the outbound stage must not
    // fabricate a node name or latency.
    assert_eq!(chain.final_outbound, "未知出口");
    assert_eq!(chain.final_node_delay_ms, None);

    // Hit feeding and clearing flow through the same shared instance.
    port.record_hits(&[("MATCH,FALLBACK", 32)]);
    assert_eq!(app.hit_count_for("MATCH,FALLBACK"), 1);
    port.clear_hits();
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

    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
    assert_eq!(
        snapshot.simulated_context.src_ip.as_deref(),
        Some("10.20.30.40")
    );
    assert_eq!(snapshot.simulated_context.src_port, Some(54321));
    assert_eq!(snapshot.simulated_context.in_port, Some(7891));

    let chain = snapshot.decision_chain.expect("decision chain");
    let inbound = &chain.nodes[0];
    assert_eq!(inbound.stage, DecisionStageKind::Inbound);
    assert!(inbound.detail.contains("10.20.30.40"));
    assert!(inbound.detail.contains("7891"));
    assert!(
        inbound
            .sub_evaluations
            .iter()
            .any(|line| line.contains("10.20.30.40"))
    );
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
    let baseline = app.project(&running_core(), &rules, None, None);
    assert_eq!(
        baseline
            .decision_chain
            .expect("baseline chain")
            .target_proxy,
        "FALLBACK"
    );

    app.set_context(&TrafficContextSnapshot {
        src_ip: Some("10.1.2.3".to_owned()),
        ..TrafficContextSnapshot::default()
    });
    let snapshot = app.project(&running_core(), &rules, None, None);
    let chain = snapshot.decision_chain.expect("decision chain");
    assert_eq!(chain.matched_rule_type, "SRC-IP-CIDR");
    assert_eq!(chain.target_proxy, "DIRECT");
}

#[test]
fn tracer_port_set_context_updates_shared_environment() {
    let app = RuleTracerApplication::new();
    let port: &dyn infiltrator_ports::rule_tracer::RuleTracerPort = &app;
    port.set_query("www.google.com");
    port.set_context(&TrafficContextSnapshot {
        src_ip: Some("192.168.1.77".to_owned()),
        ..TrafficContextSnapshot::default()
    });
    assert_eq!(app.context().src_ip.as_deref(), Some("192.168.1.77"));

    let chain = port.trace(&sample_rules(), "www.google.com", None);
    assert!(chain.nodes[0].detail.contains("192.168.1.77"));

    let snapshot = app.project(&running_core(), &sample_rules(), None, None);
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
    fail_apply: bool,
}

impl FakeOverridePort {
    fn with_rules(rules: Vec<RuleEntry>) -> Self {
        Self {
            rules: Arc::new(Mutex::new(rules)),
            applied: Arc::new(Mutex::new(Vec::new())),
            fail_apply: false,
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
impl infiltrator_ports::rule_tracer::RuleOverridePort for FakeOverridePort {
    async fn load_rule_entries(
        &self,
    ) -> Result<Vec<RuleEntry>, infiltrator_ports::error::PortError> {
        Ok(self.rules.lock().expect("rules lock").clone())
    }

    async fn apply_rule_entries(
        &self,
        entries: &[RuleEntry],
    ) -> Result<(), infiltrator_ports::error::PortError> {
        if self.fail_apply {
            return Err(infiltrator_ports::error::PortError::Failed(
                "apply transaction rolled back".to_owned(),
            ));
        }
        self.applied
            .lock()
            .expect("applied lock")
            .push(entries.to_vec());
        *self.rules.lock().expect("rules lock") = entries.to_vec();
        Ok(())
    }
}

#[tokio::test]
async fn d12_08_override_rewrites_the_rule_and_applies_it() {
    let app = RuleTracerApplication::new();
    let port = Arc::new(FakeOverridePort::with_rules(sample_rules()));
    app.set_override_port(port.clone());

    let result = app
        .apply_override(&TracerRuleOverride {
            rule_index: 0,
            new_target: "DIRECT".to_owned(),
        })
        .await;

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
    let persisted =
        infiltrator_ports::rule_tracer::RuleOverridePort::load_rule_entries(port.as_ref())
            .await
            .expect("persisted rules");
    assert_eq!(persisted[0].rule, "DOMAIN-SUFFIX,google.com,DIRECT");
    // Untouched rules keep their outbound.
    assert_eq!(persisted[1].rule, "DOMAIN-KEYWORD,bilibili,DIRECT");
}

#[tokio::test]
async fn d12_08_override_reports_unsupported_without_a_composed_port() {
    let app = RuleTracerApplication::new();
    let result = app
        .apply_override(&TracerRuleOverride {
            rule_index: 0,
            new_target: "DIRECT".to_owned(),
        })
        .await;
    assert_eq!(result.status, TracerRuleOverrideStatus::Unsupported);
    assert!(result.failure_message().is_some());
}

#[tokio::test]
async fn d12_08_override_rejects_blank_and_stale_and_failed_paths() {
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(FakeOverridePort::with_rules(sample_rules())));

    let blank = app
        .apply_override(&TracerRuleOverride {
            rule_index: 0,
            new_target: "   ".to_owned(),
        })
        .await;
    assert_eq!(blank.status, TracerRuleOverrideStatus::InvalidTarget);

    let comma = app
        .apply_override(&TracerRuleOverride {
            rule_index: 0,
            new_target: "PROXY,DIRECT".to_owned(),
        })
        .await;
    assert_eq!(comma.status, TracerRuleOverrideStatus::InvalidTarget);

    let stale = app
        .apply_override(&TracerRuleOverride {
            rule_index: 99,
            new_target: "DIRECT".to_owned(),
        })
        .await;
    assert_eq!(stale.status, TracerRuleOverrideStatus::StaleRuleIndex);
}

#[tokio::test]
async fn d12_08_override_surfaces_apply_transaction_failure() {
    let app = RuleTracerApplication::new();
    app.set_override_port(Arc::new(FakeOverridePort::failing_apply(sample_rules())));

    let result = app
        .apply_override(&TracerRuleOverride {
            rule_index: 0,
            new_target: "REJECT".to_owned(),
        })
        .await;
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
    let surface_port: &dyn infiltrator_ports::rule_tracer::RuleTracerPort = &app;

    let result = surface_port
        .apply_override(&TracerRuleOverride {
            rule_index: 1,
            new_target: "PROXY".to_owned(),
        })
        .await;
    assert!(result.is_applied());
    assert_eq!(
        result.updated_rule_raw.as_deref(),
        Some("DOMAIN-KEYWORD,bilibili,PROXY")
    );
}
