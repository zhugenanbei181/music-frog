//! Application seam for the interactive Live Rule Tracer sandbox (分流追踪器应用服务).

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::rule_tracer::{
    DecisionChainSnapshot, RuleDeadEntry, RuleDeadReason, RuleHitAuditSnapshot, RuleHitSummary,
    RuleTracerSnapshot, RuleTracerStatus, TrafficContextSnapshot,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rule_hit_counter::RuleHitCounter;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::analyzer::{ShadowReason, find_shadowed_rules};
use infiltrator_domain::rules::tracer::{
    RuleTraceMatch, TrafficContext, build_decision_chain, trace_rules,
};
use infiltrator_domain::rules::types::parse_rule_str;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Maximum number of per-rule hit summaries published in the audit snapshot.
const HIT_AUDIT_LIMIT: usize = 20;

#[derive(Clone, Debug, Default)]
pub struct RuleTracerApplication {
    active_query: Arc<Mutex<String>>,
    hit_counter: Arc<Mutex<RuleHitCounter>>,
}

impl RuleTracerApplication {
    pub fn new() -> Self {
        Self {
            active_query: Arc::new(Mutex::new(String::new())),
            hit_counter: Arc::new(Mutex::new(RuleHitCounter::new())),
        }
    }

    pub fn with_query(query: impl Into<String>) -> Self {
        Self {
            active_query: Arc::new(Mutex::new(query.into())),
            hit_counter: Arc::new(Mutex::new(RuleHitCounter::new())),
        }
    }

    pub fn set_query(&self, query: impl Into<String>) {
        if let Ok(mut lock) = self.active_query.lock() {
            *lock = query.into();
        }
    }

    pub fn query(&self) -> String {
        self.active_query
            .lock()
            .map(|q| q.clone())
            .unwrap_or_default()
    }

    /// Feed observed rule hits into the shared counter.
    pub fn record_hits(&self, hits: &[(&str, u64)]) {
        if let Ok(mut counter) = self.hit_counter.lock() {
            counter.record_batch(hits);
        }
    }

    /// Reset every accumulated hit counter and timestamp.
    pub fn clear_hits(&self) {
        if let Ok(mut counter) = self.hit_counter.lock() {
            counter.clear();
        }
    }

    /// Hits observed for a single rule.
    pub fn hit_count_for(&self, rule_raw: &str) -> u64 {
        self.hit_counter
            .lock()
            .map(|counter| counter.hit_count_for(rule_raw))
            .unwrap_or(0)
    }

    /// Last observed hit timestamp (epoch seconds) for a single rule.
    pub fn last_hit_for(&self, rule_raw: &str) -> Option<u64> {
        self.hit_counter
            .lock()
            .ok()
            .and_then(|counter| counter.last_hit_for(rule_raw))
    }

    /// Build the shared hit-audit read model from the accumulated counter and
    /// the static shadow analysis of the current rule list.
    pub fn audit(&self, rules: &[RuleEntry]) -> RuleHitAuditSnapshot {
        let total_hits;
        let tracked_rules;
        let top_hits: Vec<RuleHitSummary>;
        let last_hit_rule;
        let last_hit_secs;

        match self.hit_counter.lock() {
            Ok(counter) => {
                total_hits = counter.total_hits();
                top_hits = counter
                    .top_rules_by_hits(HIT_AUDIT_LIMIT)
                    .into_iter()
                    .map(|record| RuleHitSummary {
                        last_hit_secs: (record.last_hit_secs > 0).then_some(record.last_hit_secs),
                        rule_raw: record.rule_raw,
                        hit_count: record.hit_count,
                        total_payload_bytes: record.total_payload_bytes,
                    })
                    .collect();
                tracked_rules = top_hits.len();
                last_hit_secs = top_hits
                    .iter()
                    .filter_map(|entry| entry.last_hit_secs)
                    .max();
                last_hit_rule = last_hit_secs.and_then(|latest| {
                    top_hits
                        .iter()
                        .find(|entry| entry.last_hit_secs == Some(latest))
                        .map(|entry| entry.rule_raw.clone())
                });
            }
            Err(_) => {
                total_hits = 0;
                tracked_rules = 0;
                top_hits = Vec::new();
                last_hit_rule = None;
                last_hit_secs = None;
            }
        }

        let shadow_warnings = find_shadowed_rules(rules);
        let shadow_map: HashMap<usize, &_> = shadow_warnings
            .iter()
            .map(|warning| (warning.index, warning))
            .collect();

        let mut dead_rules = Vec::new();
        let mut cidr_overlaps = Vec::new();
        for (index, entry) in rules.iter().enumerate() {
            let hit_count = self.hit_count_for(&entry.rule);
            let last_hit = self.last_hit_for(&entry.rule);
            if let Some(warning) = shadow_map.get(&index) {
                let entry = RuleDeadEntry {
                    rule_raw: entry.rule.clone(),
                    hit_count,
                    reason: RuleDeadReason::Shadowed,
                    shadowed_by: Some(warning.shadowed_by_rule.clone()),
                    detail: Some(warning.reason.to_string()),
                    last_hit_secs: last_hit,
                };
                if matches!(warning.reason, ShadowReason::IpCidrShadowedByCidr) {
                    cidr_overlaps.push(entry.clone());
                }
                dead_rules.push(entry);
            } else if hit_count == 0 && entry.enabled {
                dead_rules.push(RuleDeadEntry {
                    rule_raw: entry.rule.clone(),
                    hit_count,
                    reason: RuleDeadReason::ZeroHits,
                    shadowed_by: None,
                    detail: None,
                    last_hit_secs: last_hit,
                });
            }
        }

        RuleHitAuditSnapshot {
            total_hits,
            tracked_rules,
            top_hits,
            dead_rules,
            cidr_overlaps,
            last_hit_rule,
            last_hit_secs,
            can_clear: total_hits > 0,
        }
    }

    /// Pure simulation trace over a rule set and query context.
    pub fn trace(
        &self,
        rules: &[RuleEntry],
        query: &str,
        active_exit: Option<&ActiveExitSnapshot>,
        proxies: Option<&HashMap<String, Proxy>>,
    ) -> (Option<RuleTraceMatch>, DecisionChainSnapshot) {
        let start = Instant::now();
        let context = TrafficContext::from_query(query);
        let domain_matched = trace_rules(rules, &context);

        let parsed_rule = domain_matched
            .as_ref()
            .and_then(|m| rules.get(m.index))
            .and_then(|entry| parse_rule_str(&entry.rule).ok());

        let target_group = domain_matched
            .as_ref()
            .map(|m| m.target.as_str())
            .unwrap_or("DIRECT");

        // Resolve outbound node and facts from proxy group read model or active exit.
        // Without runtime facts the outbound stage stays honestly unknown; the
        // domain chain renders a neutral node instead of fabricated node data.
        let mut resolved: (Option<String>, Option<String>, Option<u32>, Option<String>) =
            (None, None, None, None);
        if target_group.eq_ignore_ascii_case("DIRECT") {
            resolved = (Some("DIRECT".into()), Some("Direct".into()), None, None);
        } else if target_group.eq_ignore_ascii_case("REJECT") {
            resolved = (Some("REJECT".into()), Some("Reject".into()), None, None);
        } else if let Some(proxies) = proxies
            && let Some(group) = proxies.get(target_group)
            && let Some(now_node_name) = group.now()
        {
            let node_info = proxies.get(now_node_name);
            resolved = (
                Some(now_node_name.to_owned()),
                node_info.map(|n| n.proxy_type().to_string()),
                node_info.and_then(|n| n.delay()),
                active_exit.and_then(|e| e.country_code.clone()),
            );
        } else if let Some(exit) = active_exit
            && exit.is_drawable()
        {
            resolved = (
                exit.name.clone(),
                exit.protocol.clone(),
                exit.delay_ms,
                exit.country_code.clone(),
            );
        }
        let (outbound_node, protocol, delay, country) = (
            resolved.0.as_deref(),
            resolved.1.as_deref(),
            resolved.2,
            resolved.3.as_deref(),
        );

        let latency_us = start.elapsed().as_micros().max(1) as u64;

        let chain = build_decision_chain(
            rules,
            &context,
            domain_matched.as_ref(),
            parsed_rule.as_ref(),
            outbound_node,
            protocol,
            delay,
            country,
            latency_us,
        );

        (domain_matched, chain)
    }

    /// Project the Live Rule Tracer read model for the current snapshot revision.
    pub fn project(
        &self,
        core: &CoreSnapshot,
        rules: &[RuleEntry],
        active_exit: Option<&ActiveExitSnapshot>,
        proxies: Option<&HashMap<String, Proxy>>,
    ) -> RuleTracerSnapshot {
        let revision = core.revision.max(1);
        let query = self.query();

        // When query is empty, provide a clean, ready-to-test sandbox with presets
        if query.trim().is_empty() {
            let mut snapshot = RuleTracerSnapshot::empty(core.generation, revision);
            if matches!(
                core.lifecycle,
                CoreLifecycle::Running | CoreLifecycle::Ready
            ) {
                snapshot.status = RuleTracerStatus::Ready;
            }
            snapshot.hit_audit = self.audit(rules);
            return snapshot;
        }

        let context_domain = TrafficContext::from_query(&query);
        let ctx_snapshot = TrafficContextSnapshot {
            domain: context_domain.domain.clone(),
            ip: context_domain.ip.map(|ip| ip.to_string()),
            port: context_domain.port,
            process_name: context_domain.process_name.clone(),
            network: context_domain.network.clone(),
            in_type: context_domain.in_type.clone(),
            client_ip: context_domain.client_ip.map(|ip| ip.to_string()),
        };

        let (_matched, chain) = self.trace(rules, &query, active_exit, proxies);

        let mut snapshot = RuleTracerSnapshot::ready(
            core.generation,
            revision,
            query,
            ctx_snapshot,
            Some(chain),
            rules.len(),
        );

        // Offline notice if core is stopped but AST trace still completed
        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            snapshot.status = RuleTracerStatus::Ready;
            snapshot.failure =
                Some("内核离线：当前展示基于本地 AST 规则树的离线模拟推演".to_owned());
        }

        snapshot.hit_audit = self.audit(rules);
        snapshot
    }
}

/// Port seam so an inbound surface drives the exact application instance the
/// surface reader projects, keeping one query state across Iced and Bevy.
impl infiltrator_ports::rule_tracer::RuleTracerPort for RuleTracerApplication {
    fn set_query(&self, query: &str) {
        RuleTracerApplication::set_query(self, query);
    }

    fn trace(
        &self,
        rules: &[RuleEntry],
        query: &str,
        active_exit: Option<&ActiveExitSnapshot>,
    ) -> DecisionChainSnapshot {
        self.trace(rules, query, active_exit, None).1
    }

    fn record_hits(&self, hits: &[(&str, u64)]) {
        RuleTracerApplication::record_hits(self, hits);
    }

    fn clear_hits(&self) {
        RuleTracerApplication::clear_hits(self);
    }
}

#[cfg(test)]
mod tests {
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
}
