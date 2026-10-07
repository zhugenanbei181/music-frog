//! Application seam for the interactive Live Rule Tracer sandbox (分流追踪器应用服务).

use self::statistics::TraceStatistics;
use futures_util::lock;
use infiltrator_contract::active_exit::ActiveExitSnapshot;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_condition::{ConditionIssue, RuleTraceIssue};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_trace_run::RuleTraceExecution;
use infiltrator_contract::rule_trace_run::RuleTraceRequest;
use infiltrator_contract::rule_tracer::{
    DecisionChainSnapshot, RuleTracerSnapshot, RuleTracerStatus, TracerRuleOverride,
    TracerRuleOverrideResult, TracerRuleOverrideStatus, TrafficContextSnapshot,
};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::rules::tracer::decision_chain::build_decision_chain;
use infiltrator_domain::rules::tracer::named::trace_named_rules;
use infiltrator_domain::rules::tracer::{RuleTraceMatch, TrafficContext, trace_rules};
use infiltrator_domain::rules::types::parse_rule_str;
use infiltrator_domain::rules::{RuleEntry, rewrite_rule_target};
use infiltrator_ports::rule_tracer::RuleOverridePort;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// DUAL-12-08: the optional host persistence capability driving the
/// reverse-apply. Wrapped so the application's `Debug` derive does not require
/// the port object itself to be `Debug`.
#[derive(Clone, Default)]
struct OverridePortSlot(Arc<Mutex<Option<Arc<dyn RuleOverridePort>>>>);

impl OverridePortSlot {
    fn get(&self) -> Option<Arc<dyn RuleOverridePort>> {
        self.0.lock().expect("rule override port lock").clone()
    }

    fn set(&self, port: Arc<dyn RuleOverridePort>) {
        *self.0.lock().expect("rule override port lock") = Some(port);
    }
}

impl Debug for OverridePortSlot {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OverridePortSlot")
            .field("configured", &self.get().is_some())
            .finish()
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuleTracerApplication {
    execution: Arc<Mutex<RuleTraceExecution>>,
    admission: Arc<lock::Mutex<()>>,
    active_query: Arc<Mutex<String>>,
    context: Arc<Mutex<TrafficContextSnapshot>>,
    statistics: Arc<Mutex<TraceStatistics>>,
    override_port: OverridePortSlot,
}

impl RuleTracerApplication {
    pub fn new() -> Self {
        Self {
            execution: Arc::new(Mutex::new(RuleTraceExecution::default())),
            admission: Arc::new(lock::Mutex::new(())),
            active_query: Arc::new(Mutex::new(String::new())),
            context: Arc::new(Mutex::new(TrafficContextSnapshot::default())),
            statistics: Arc::new(Mutex::new(TraceStatistics::default())),
            override_port: OverridePortSlot::default(),
        }
    }

    #[cfg(test)]
    fn with_query(query: impl Into<String>) -> Self {
        Self {
            execution: Arc::new(Mutex::new(RuleTraceExecution::default())),
            admission: Arc::new(lock::Mutex::new(())),
            active_query: Arc::new(Mutex::new(query.into())),
            context: Arc::new(Mutex::new(TrafficContextSnapshot::default())),
            statistics: Arc::new(Mutex::new(TraceStatistics::default())),
            override_port: OverridePortSlot::default(),
        }
    }

    /// DUAL-12-08: inject the host persistence capability. Composed after the
    /// core application exists (the adapter needs the live lifecycle session),
    /// so every surface holding a clone observes the same port.
    pub fn set_override_port(&self, port: Arc<dyn RuleOverridePort>) {
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

        let workspace = match port.load_rule_workspace().await {
            Ok(workspace) => workspace,
            Err(error) => {
                return TracerRuleOverrideResult::rejected_failure(
                    TracerRuleOverrideStatus::ApplyFailed,
                    request,
                    Failure::from(error),
                );
            }
        };
        if workspace.source != request.expected_source {
            return TracerRuleOverrideResult::rejected_failure(
                TracerRuleOverrideStatus::StaleRuleIndex,
                request,
                Failure::new(
                    ErrorCode::NotReady,
                    "The source profile changed; run the simulation again",
                    true,
                ),
            );
        }
        if !workspace.targets.contains(&new_target) {
            return TracerRuleOverrideResult::rejected_failure(
                TracerRuleOverrideStatus::InvalidTarget,
                request,
                Failure::new(
                    ErrorCode::InvalidInput,
                    "The requested outbound is absent from the source profile",
                    false,
                ),
            );
        }
        let mut rules = match &request.rule_table {
            Some(table) => workspace.sub_rules.get(table).cloned().unwrap_or_default(),
            None => workspace.rules.clone(),
        };
        let Some(entry) = rules
            .get_mut(request.rule_index)
            .filter(|entry| entry.enabled && entry.rule == request.expected_rule)
        else {
            return TracerRuleOverrideResult::rejected_failure(
                TracerRuleOverrideStatus::StaleRuleIndex,
                request,
                Failure::new(
                    ErrorCode::NotReady,
                    "The traced rule changed or no longer exists; run the simulation again",
                    true,
                ),
            );
        };
        let previous_rule_raw = entry.rule.clone();
        let Some(updated_rule_raw) = rewrite_rule_target(&previous_rule_raw, &new_target) else {
            return TracerRuleOverrideResult::rejected_failure(
                TracerRuleOverrideStatus::InvalidTarget,
                request,
                Failure::new(
                    ErrorCode::InvalidInput,
                    "The rule has no rewritable outbound",
                    false,
                ),
            );
        };
        entry.rule = updated_rule_raw.clone();
        let committed = match &request.rule_table {
            Some(table) => {
                port.compare_and_apply_named_rule(
                    &workspace,
                    &RuleLocation::named(table.clone(), request.rule_index),
                    &previous_rule_raw,
                    &updated_rule_raw,
                )
                .await
            }
            None => port.compare_and_apply_rules(&workspace, &rules).await,
        };
        if let Err(error) = committed {
            return TracerRuleOverrideResult::rejected_failure(
                TracerRuleOverrideStatus::ApplyFailed,
                request,
                Failure::from(error),
            );
        }

        TracerRuleOverrideResult::applied(
            request.rule_index,
            new_target,
            previous_rule_raw,
            updated_rule_raw,
        )
    }

    fn set_query(&self, query: impl Into<String>) {
        *self.active_query.lock().expect("rule trace query lock") = query.into();
    }

    fn query(&self) -> String {
        self.active_query
            .lock()
            .expect("rule trace query lock")
            .clone()
    }

    /// DUAL-12-10: store the simulated sandbox environment (source IP and
    /// inbound ports). The stored context is merged onto every trace and
    /// projection so the Iced port path and the Bevy surface reader agree.
    fn set_context(&self, context: &TrafficContextSnapshot) {
        *self.context.lock().expect("rule trace context lock") = context.clone();
    }

    /// The stored simulated sandbox environment.
    fn context(&self) -> TrafficContextSnapshot {
        self.context
            .lock()
            .expect("rule trace context lock")
            .clone()
    }

    /// Merge the stored sandbox environment onto a query-derived context.
    fn merged_context(&self, query: &str) -> Result<TrafficContext, RuleTraceIssue> {
        TrafficContext::from_request(&RuleTraceRequest {
            query: query.into(),
            context: self.context(),
        })
        .map_err(|failure| RuleTraceIssue {
            index: 0,
            location: None,
            rule: String::new(),
            issue: ConditionIssue::InvalidRule {
                reason: failure.message,
            },
        })
    }

    /// Pure simulation trace over a rule set and query context.
    #[cfg(test)]
    pub(crate) fn trace(
        &self,
        rules: &[RuleEntry],
        query: &str,
        active_exit: Option<&ActiveExitSnapshot>,
        proxies: Option<&HashMap<String, Proxy>>,
    ) -> Result<(Option<RuleTraceMatch>, DecisionChainSnapshot), RuleTraceIssue> {
        self.trace_in_scope(rules, None, query, active_exit, proxies)
    }

    fn trace_in_scope(
        &self,
        rules: &[RuleEntry],
        sub_rules: Option<&BTreeMap<String, Vec<RuleEntry>>>,
        query: &str,
        active_exit: Option<&ActiveExitSnapshot>,
        proxies: Option<&HashMap<String, Proxy>>,
    ) -> Result<(Option<RuleTraceMatch>, DecisionChainSnapshot), RuleTraceIssue> {
        let start = Instant::now();
        let context = self.merged_context(query)?;
        let domain_matched = match sub_rules {
            Some(tables) => trace_named_rules(rules, tables, &context),
            None => trace_rules(rules, &context),
        }?;

        let parsed_rule = domain_matched
            .as_ref()
            .and_then(|matched| parse_rule_str(&matched.raw).ok());

        let target_group = domain_matched
            .as_ref()
            .map(|m| m.target.as_str())
            .unwrap_or("DIRECT");

        // Resolve outbound node and facts from proxy group read model or active exit.
        // Without runtime facts the outbound stage stays honestly unknown; the
        // domain chain renders a neutral node instead of fabricated node data.
        let mut resolved: (Option<String>, Option<String>, Option<u32>, Option<String>) =
            (None, None, None, None);
        if target_group == "DIRECT" {
            resolved = (Some("DIRECT".into()), Some("Direct".into()), None, None);
        } else if target_group == "REJECT" {
            resolved = (Some("REJECT".into()), Some("Reject".into()), None, None);
        } else if let Some(proxies) = proxies
            && let Some(group) = proxies.get(target_group)
            && let Some(now_node_name) = group.now()
        {
            let node_info = proxies.get(now_node_name);
            resolved = (
                Some(now_node_name.to_owned()),
                node_info.map(|n| n.proxy_type().to_string()),
                node_info.and_then(|n| n.delay()).filter(|delay| *delay > 0),
                active_exit
                    .filter(|exit| {
                        exit.is_drawable() && exit.name.as_deref() == Some(now_node_name)
                    })
                    .and_then(|exit| exit.country_code.clone()),
            );
        } else if let Some(node) = proxies.and_then(|proxies| proxies.get(target_group))
            && !node.is_group()
        {
            resolved = (
                Some(target_group.to_owned()),
                Some(node.proxy_type().to_owned()),
                node.delay().filter(|delay| *delay > 0),
                active_exit
                    .filter(|exit| exit.is_drawable() && exit.name.as_deref() == Some(target_group))
                    .and_then(|exit| exit.country_code.clone()),
            );
        } else if let Some(exit) = active_exit
            && exit.is_drawable()
            && (exit.group.as_deref() == Some(target_group)
                || exit.name.as_deref() == Some(target_group))
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

        let latency_us = start.elapsed().as_micros() as u64;
        self.statistics
            .lock()
            .expect("rule statistics lock")
            .record_trace(domain_matched.as_ref(), latency_us);

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

        Ok((domain_matched, chain))
    }

    /// Project the Live Rule Tracer read model for the current snapshot revision.
    #[cfg(test)]
    pub(crate) fn simulate_snapshot(
        &self,
        core: Option<&CoreSnapshot>,
        rules: &[RuleEntry],
        active_exit: Option<&ActiveExitSnapshot>,
        proxies: Option<&HashMap<String, Proxy>>,
    ) -> Result<RuleTracerSnapshot, RuleTraceIssue> {
        self.simulate_workspace_snapshot(core, rules, None, active_exit, proxies)
    }

    pub(crate) fn simulate_workspace_snapshot(
        &self,
        core: Option<&CoreSnapshot>,
        rules: &[RuleEntry],
        sub_rules: Option<&BTreeMap<String, Vec<RuleEntry>>>,
        active_exit: Option<&ActiveExitSnapshot>,
        proxies: Option<&HashMap<String, Proxy>>,
    ) -> Result<RuleTracerSnapshot, RuleTraceIssue> {
        let generation = core.map_or(0, |core| core.generation);
        let revision = core.map_or(1, |core| core.revision.max(1));
        let query = self.query();

        // When query is empty, provide a clean, ready-to-test sandbox with presets
        if query.trim().is_empty() {
            let mut snapshot = RuleTracerSnapshot::empty(generation, revision);
            if core.is_some_and(|core| {
                matches!(
                    core.lifecycle,
                    CoreLifecycle::Running | CoreLifecycle::Ready
                )
            }) {
                snapshot.status = RuleTracerStatus::Ready;
            }
            return Ok(snapshot);
        }

        // The simulated sandbox environment (source IP / ports) the shared
        // engine merged onto the query; published as `simulated_context`.
        let simulated_context = self.merged_context(&query)?;
        let ctx_snapshot = simulated_context.snapshot();

        let (_matched, chain) =
            self.trace_in_scope(rules, sub_rules, &query, active_exit, proxies)?;

        let mut snapshot = RuleTracerSnapshot::ready(
            generation,
            revision,
            query,
            ctx_snapshot,
            Some(chain),
            rules.len(),
        );

        // Offline notice if core is stopped but AST trace still completed
        if core.is_some_and(|core| {
            !matches!(
                core.lifecycle,
                CoreLifecycle::Running | CoreLifecycle::Ready
            )
        }) {
            snapshot.status = RuleTracerStatus::Ready;
            snapshot.failure =
                Some("内核离线：当前展示基于本地 AST 规则树的离线模拟推演".to_owned());
        }

        Ok(snapshot)
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

mod execution;

#[cfg(test)]
#[path = "rule_tracer_application_test.rs"]
mod tests;

mod statistics;

#[cfg(test)]
#[path = "rule_trace_statistics_test.rs"]
mod statistics_test;
