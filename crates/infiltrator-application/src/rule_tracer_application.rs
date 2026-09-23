//! Application seam for the interactive Live Rule Tracer sandbox (分流追踪器应用服务).

use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::rule_tracer::{
    DecisionChainSnapshot, RuleDeadEntry, RuleDeadReason, RuleHitAuditSnapshot, RuleHitSummary,
    RuleTracerSnapshot, RuleTracerStatus, TracerRuleOverride, TracerRuleOverrideResult,
    TracerRuleOverrideStatus, TrafficContextSnapshot,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rule_hit_counter::RuleHitCounter;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::analyzer::{ShadowReason, find_shadowed_rules};
use infiltrator_domain::rules::tracer::decision_chain::build_decision_chain;
use infiltrator_domain::rules::tracer::{RuleTraceMatch, TrafficContext, trace_rules};
use infiltrator_domain::rules::types::parse_rule_str;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Maximum number of per-rule hit summaries published in the audit snapshot.
const HIT_AUDIT_LIMIT: usize = 20;

/// Running AST match-latency statistics for the routing-contribution audit.
#[derive(Clone, Debug, Default)]
struct TraceLatencyStats {
    count: u64,
    total_us: u64,
    last_us: Option<u64>,
}

/// DUAL-12-08: the optional host persistence capability driving the
/// reverse-apply. Wrapped so the application's `Debug` derive does not require
/// the port object itself to be `Debug`.
#[derive(Clone, Default)]
struct OverridePortSlot(
    Arc<Mutex<Option<Arc<dyn infiltrator_ports::rule_tracer::RuleOverridePort>>>>,
);

impl OverridePortSlot {
    fn get(&self) -> Option<Arc<dyn infiltrator_ports::rule_tracer::RuleOverridePort>> {
        self.0.lock().ok().and_then(|slot| slot.clone())
    }

    fn set(&self, port: Arc<dyn infiltrator_ports::rule_tracer::RuleOverridePort>) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(port);
        }
    }
}

impl std::fmt::Debug for OverridePortSlot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OverridePortSlot")
            .field("configured", &self.get().is_some())
            .finish()
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuleTracerApplication {
    active_query: Arc<Mutex<String>>,
    context: Arc<Mutex<TrafficContextSnapshot>>,
    hit_counter: Arc<Mutex<RuleHitCounter>>,
    trace_stats: Arc<Mutex<TraceLatencyStats>>,
    override_port: OverridePortSlot,
}

impl RuleTracerApplication {
    pub fn new() -> Self {
        Self {
            active_query: Arc::new(Mutex::new(String::new())),
            context: Arc::new(Mutex::new(TrafficContextSnapshot::default())),
            hit_counter: Arc::new(Mutex::new(RuleHitCounter::new())),
            trace_stats: Arc::new(Mutex::new(TraceLatencyStats::default())),
            override_port: OverridePortSlot::default(),
        }
    }

    pub fn with_query(query: impl Into<String>) -> Self {
        Self {
            active_query: Arc::new(Mutex::new(query.into())),
            context: Arc::new(Mutex::new(TrafficContextSnapshot::default())),
            hit_counter: Arc::new(Mutex::new(RuleHitCounter::new())),
            trace_stats: Arc::new(Mutex::new(TraceLatencyStats::default())),
            override_port: OverridePortSlot::default(),
        }
    }

    /// DUAL-12-08: inject the host persistence capability. Composed after the
    /// core application exists (the adapter needs the live lifecycle session),
    /// so every surface holding a clone observes the same port.
    pub fn set_override_port(
        &self,
        port: Arc<dyn infiltrator_ports::rule_tracer::RuleOverridePort>,
    ) {
        self.override_port.set(port);
    }

    /// DUAL-12-08: reverse-apply a matched rule's outbound. Loads the live rule
    /// list, rewrites the matched rule's target, and commits the whole list
    /// through the host's atomic apply transaction. Returns one typed result;
    /// no capability, a stale index, an invalid target and an apply failure are
    /// all reported honestly.
    pub async fn apply_override(&self, request: &TracerRuleOverride) -> TracerRuleOverrideResult {
        let new_target = request.new_target.trim().to_owned();
        if let Err(reason) = validate_override_target(&new_target) {
            return TracerRuleOverrideResult::rejected(
                TracerRuleOverrideStatus::InvalidTarget,
                request.rule_index,
                new_target,
                reason,
            );
        }

        let Some(port) = self.override_port.get() else {
            return TracerRuleOverrideResult::rejected(
                TracerRuleOverrideStatus::Unsupported,
                request.rule_index,
                new_target,
                "此主机未组合规则反向应用能力 (no rule apply capability composed)".to_owned(),
            );
        };

        let mut rules = match port.load_rule_entries().await {
            Ok(rules) => rules,
            Err(error) => {
                return TracerRuleOverrideResult::rejected(
                    TracerRuleOverrideStatus::ApplyFailed,
                    request.rule_index,
                    new_target,
                    format!("读取当前规则失败: {error}"),
                );
            }
        };

        let Some(entry) = rules.get_mut(request.rule_index) else {
            return TracerRuleOverrideResult::rejected(
                TracerRuleOverrideStatus::StaleRuleIndex,
                request.rule_index,
                new_target,
                format!(
                    "命中规则 #{} 已不存在，请重新执行追踪",
                    request.rule_index + 1
                ),
            );
        };

        let previous_rule_raw = entry.rule.clone();
        let Some(updated_rule_raw) =
            infiltrator_domain::rules::rewrite_rule_target(&previous_rule_raw, &new_target)
        else {
            return TracerRuleOverrideResult::rejected(
                TracerRuleOverrideStatus::InvalidTarget,
                request.rule_index,
                new_target,
                "该规则没有可重写的出站目标".to_owned(),
            );
        };
        entry.rule = updated_rule_raw.clone();

        if let Err(error) = port.apply_rule_entries(&rules).await {
            return TracerRuleOverrideResult::rejected(
                TracerRuleOverrideStatus::ApplyFailed,
                request.rule_index,
                new_target,
                format!("应用配置事务失败: {error}"),
            );
        }

        TracerRuleOverrideResult::applied(
            request.rule_index,
            new_target,
            previous_rule_raw,
            updated_rule_raw,
        )
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

    /// DUAL-12-10: store the simulated sandbox environment (source IP and
    /// inbound ports). The stored context is merged onto every trace and
    /// projection so the Iced port path and the Bevy surface reader agree.
    pub fn set_context(&self, context: &TrafficContextSnapshot) {
        if let Ok(mut lock) = self.context.lock() {
            *lock = context.clone();
        }
    }

    /// The stored simulated sandbox environment.
    pub fn context(&self) -> TrafficContextSnapshot {
        self.context
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Merge the stored sandbox environment onto a query-derived context.
    fn merged_context(&self, query: &str) -> TrafficContext {
        let mut context = TrafficContext::from_query(query);
        let stored = self.context();
        if let Some(src_ip) = stored
            .src_ip
            .as_deref()
            .and_then(|raw| raw.trim().parse::<std::net::IpAddr>().ok())
        {
            context.src_ip = Some(src_ip);
        }
        if let Some(src_port) = stored.src_port {
            context.src_port = Some(src_port);
        }
        if let Some(in_port) = stored.in_port {
            context.in_port = Some(in_port);
        }
        if let Some(client_ip) = stored
            .client_ip
            .as_deref()
            .and_then(|raw| raw.trim().parse::<std::net::IpAddr>().ok())
        {
            context.client_ip = Some(client_ip);
        }
        context
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

        let (trace_count, avg_match_latency_us, last_match_latency_us) =
            match self.trace_stats.lock() {
                Ok(stats) => (
                    stats.count,
                    (stats.count > 0).then(|| stats.total_us as f64 / stats.count as f64),
                    stats.last_us,
                ),
                Err(_) => (0, None, None),
            };

        RuleHitAuditSnapshot {
            total_hits,
            tracked_rules,
            top_hits,
            dead_rules,
            cidr_overlaps,
            last_hit_rule,
            last_hit_secs,
            can_clear: total_hits > 0,
            trace_count,
            avg_match_latency_us,
            last_match_latency_us,
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
        let context = self.merged_context(query);
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
        if let Ok(mut stats) = self.trace_stats.lock() {
            stats.count += 1;
            stats.total_us += latency_us;
            stats.last_us = Some(latency_us);
        }

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

        // The simulated sandbox environment (source IP / ports) the shared
        // engine merged onto the query; published as `simulated_context`.
        let simulated_context = self.merged_context(&query);
        let ctx_snapshot = TrafficContextSnapshot {
            domain: simulated_context.domain.clone(),
            ip: simulated_context.ip.map(|ip| ip.to_string()),
            port: simulated_context.port,
            src_ip: simulated_context.src_ip.map(|ip| ip.to_string()),
            src_port: simulated_context.src_port,
            in_port: simulated_context.in_port,
            process_name: simulated_context.process_name.clone(),
            network: simulated_context.network.clone(),
            in_type: simulated_context.in_type.clone(),
            client_ip: simulated_context.client_ip.map(|ip| ip.to_string()),
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
#[async_trait::async_trait]
impl infiltrator_ports::rule_tracer::RuleTracerPort for RuleTracerApplication {
    fn set_query(&self, query: &str) {
        RuleTracerApplication::set_query(self, query);
    }

    fn set_context(&self, context: &TrafficContextSnapshot) {
        RuleTracerApplication::set_context(self, context);
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

    async fn apply_override(&self, request: &TracerRuleOverride) -> TracerRuleOverrideResult {
        RuleTracerApplication::apply_override(self, request).await
    }
}

/// DUAL-12-08: reject an outbound target that cannot be written into a rule
/// expression. PROXY / DIRECT / REJECT and any typed group name are accepted;
/// empty, comma-bearing and control-character targets are not.
fn validate_override_target(target: &str) -> Result<(), String> {
    if target.is_empty() {
        return Err("出站目标不能为空".to_owned());
    }
    if target.chars().count() > 64 {
        return Err("出站目标过长 (最多 64 字符)".to_owned());
    }
    if target.contains(',') || target.contains('\n') || target.contains('\r') {
        return Err("出站目标不能包含逗号或换行".to_owned());
    }
    if target.chars().any(char::is_control) {
        return Err("出站目标包含非法控制字符".to_owned());
    }
    Ok(())
}

#[cfg(test)]
#[path = "rule_tracer_application_test.rs"]
mod tests;
