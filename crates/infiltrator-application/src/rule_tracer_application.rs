//! Application seam for the interactive Live Rule Tracer sandbox (分流追踪器应用服务).

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::rule_tracer::{
    DecisionChainSnapshot, RuleTracerSnapshot, RuleTracerStatus,
    TrafficContextSnapshot,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::tracer::{TrafficContext, build_decision_chain, trace_rules, RuleTraceMatch};
use infiltrator_domain::rules::types::parse_rule_str;
use infiltrator_domain::rules::RuleEntry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Debug, Default)]
pub struct RuleTracerApplication {
    active_query: Arc<Mutex<String>>,
}

impl RuleTracerApplication {
    pub fn new() -> Self {
        Self {
            active_query: Arc::new(Mutex::new(String::new())),
        }
    }

    pub fn with_query(query: impl Into<String>) -> Self {
        Self {
            active_query: Arc::new(Mutex::new(query.into())),
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

        // Resolve outbound node and facts from proxy group read model or active exit
        let mut group_proto_holder = None;
        let mut group_country_holder = None;
        let (outbound_node, protocol, delay, country) = if target_group.eq_ignore_ascii_case("DIRECT") {
            (Some("DIRECT"), Some("Direct"), None, None)
        } else if target_group.eq_ignore_ascii_case("REJECT") {
            (Some("REJECT"), Some("Reject"), None, None)
        } else if let Some(proxies) = proxies
            && let Some(group) = proxies.get(target_group)
            && let Some(now_node_name) = group.now()
        {
            let node_info = proxies.get(now_node_name);
            group_proto_holder = node_info.map(|n| n.proxy_type().to_string());
            let delay_val = node_info.and_then(|n| n.delay());
            group_country_holder = active_exit.and_then(|e| e.country_code.clone());
            (Some(now_node_name), group_proto_holder.as_deref().or(Some("VLESS · Reality")), delay_val, group_country_holder.as_deref())
        } else if let Some(exit) = active_exit
            && exit.is_drawable()
        {
            (
                exit.name.as_deref().or(Some("香港专线 01")),
                exit.protocol.as_deref().or(Some("VLESS · Reality")),
                exit.delay_ms,
                exit.country_code.as_deref().or(Some("HK")),
            )
        } else {
            (Some("香港专线 01"), Some("VLESS · Reality"), Some(28), Some("HK"))
        };

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
            if matches!(core.lifecycle, CoreLifecycle::Running | CoreLifecycle::Ready) {
                snapshot.status = RuleTracerStatus::Ready;
            }
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
        if !matches!(core.lifecycle, CoreLifecycle::Running | CoreLifecycle::Ready) {
            snapshot.status = RuleTracerStatus::Ready;
            snapshot.failure = Some("内核离线：当前展示基于本地 AST 规则树的离线模拟推演".to_owned());
        }

        snapshot
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
        assert!(chain.nodes[2].sub_evaluations.iter().any(|e| e.contains("PASS")));
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
}
