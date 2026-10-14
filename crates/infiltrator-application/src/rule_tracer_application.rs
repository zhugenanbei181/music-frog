//! Application seam for the interactive Live Rule Tracer sandbox (分流追踪器应用服务).

use self::statistics::TraceStatistics;
use futures_util::lock;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::details::{OutboundObservation, OutboundProtocolObservation};
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::outbound::inventory::{
    OutboundInventoryState, OutboundObservationSnapshot, OutboundSelectionState,
};
use infiltrator_contract::rule_condition::{ConditionIssue, RuleTraceIssue};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_contract::rule_trace_run::RuleTraceRequest;
use infiltrator_contract::rule_trace_run::{RuleTraceExecution, RuleTraceOutboundFacts};
use infiltrator_contract::rule_tracer::{
    DecisionChainSnapshot, RuleTraceOutboundResolution, RuleTraceRuntimeMode, RuleTracerSnapshot,
    TracerRuleOverride, TracerRuleOverrideResult, TracerRuleOverrideStatus, TrafficContextSnapshot,
};
#[cfg(test)]
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_domain::rules::tracer::decision_chain::build_decision_chain;
use infiltrator_domain::rules::tracer::named::trace_named_rules;
use infiltrator_domain::rules::tracer::{RuleTraceMatch, TrafficContext, trace_rules};
use infiltrator_domain::rules::types::parse_rule_str;
use infiltrator_domain::rules::{RuleEntry, rewrite_rule_target};
use infiltrator_ports::rule_tracer::RuleOverridePort;
#[cfg(test)]
use infiltrator_test_support::rule_trace_fixtures::{
    rule_trace_test_scope, unknown_rule_trace_observation,
};
use std::collections::BTreeMap;
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

#[derive(Clone, Copy)]
enum RuleTraceSessionMode {
    Live,
    Offline,
}

#[derive(Clone, Copy)]
enum RuleTraceSnapshotOwner {
    Live { generation: u64, revision: u64 },
    Offline { generation: u64, revision: u64 },
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
    ) -> Result<(Option<RuleTraceMatch>, DecisionChainSnapshot), RuleTraceIssue> {
        let observation = unknown_rule_trace_observation(rule_trace_test_scope());
        self.trace_with_outbounds(
            rules,
            None,
            query,
            &RuleTraceOutboundFacts::observed(observation),
        )
        .map(|(matched, chain, _)| (matched, chain))
    }

    fn trace_with_outbounds(
        &self,
        rules: &[RuleEntry],
        sub_rules: Option<&BTreeMap<String, Vec<RuleEntry>>>,
        query: &str,
        outbound_facts: &RuleTraceOutboundFacts,
    ) -> Result<
        (
            Option<RuleTraceMatch>,
            DecisionChainSnapshot,
            RuleTraceOutboundResolution,
        ),
        RuleTraceIssue,
    > {
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

        // Resolve through typed collection membership and selection, then use
        // details authored for that same engine observation. Names in a rule
        // remain document syntax; they never key a production map.
        let mut resolved: (Option<String>, Option<String>, Option<u32>, Option<String>) =
            (None, None, None, None);
        let mut outbound_resolution = match outbound_facts {
            RuleTraceOutboundFacts::Observed(_) => RuleTraceOutboundResolution::Unknown,
            RuleTraceOutboundFacts::Unavailable(failure) => {
                RuleTraceOutboundResolution::Unavailable {
                    failure: failure.clone(),
                }
            }
        };
        if target_group == "DIRECT" {
            resolved = (Some("DIRECT".into()), Some("Direct".into()), None, None);
            outbound_resolution = RuleTraceOutboundResolution::NotRequired;
        } else if target_group == "REJECT" {
            resolved = (Some("REJECT".into()), Some("Reject".into()), None, None);
            outbound_resolution = RuleTraceOutboundResolution::NotRequired;
        } else if let RuleTraceOutboundFacts::Observed(outbound_observation) = outbound_facts {
            if let Some(outbound_id) = resolve_rule_target(target_group, outbound_observation) {
                let outbound_name = match outbound_observation.inventory().inventory() {
                    OutboundInventoryState::Ready { entries, .. } => entries
                        .iter()
                        .find(|entry| &entry.id == outbound_id)
                        .and_then(|entry| entry.display_name.clone())
                        .unwrap_or_else(|| outbound_id.value().to_owned()),
                    OutboundInventoryState::Unknown
                    | OutboundInventoryState::Empty
                    | OutboundInventoryState::Unsupported
                    | OutboundInventoryState::Failed { .. } => outbound_id.value().to_owned(),
                };
                let detail = outbound_observation.details().detail(outbound_id);
                let protocol = detail.and_then(|detail| match &detail.protocol {
                    OutboundProtocolObservation::Reported(protocol) => Some(
                        detail
                            .protocol_display_name
                            .clone()
                            .unwrap_or_else(|| protocol.value().to_owned()),
                    ),
                    OutboundProtocolObservation::Unknown
                    | OutboundProtocolObservation::Unsupported => None,
                });
                let delay = detail.and_then(|detail| match &detail.current_latency_ms {
                    OutboundObservation::Observed(0) => None,
                    OutboundObservation::Observed(delay) => Some(*delay),
                    OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
                });
                let country = detail.and_then(|detail| match &detail.historical_egress {
                    OutboundObservation::Observed(history) => {
                        history.last().and_then(|sample| sample.country.clone())
                    }
                    OutboundObservation::Unknown | OutboundObservation::Unsupported => None,
                });
                resolved = (Some(outbound_name), protocol, delay, country);
                outbound_resolution = RuleTraceOutboundResolution::Resolved {
                    outbound_id: outbound_id.clone(),
                    inventory_revision: outbound_observation.inventory().revision(),
                    details_revision: outbound_observation.details().detail_revision(),
                };
            }
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

        Ok((domain_matched, chain, outbound_resolution))
    }

    /// Project the Live Rule Tracer read model for the current snapshot revision.
    #[cfg(test)]
    pub(crate) fn simulate_snapshot(
        &self,
        core: &CoreSnapshot,
        rules: &[RuleEntry],
    ) -> Result<RuleTracerSnapshot, RuleTraceIssue> {
        let outbound_facts = match core.scope() {
            Some(scope) => match scope.session_token.as_ref() {
                Some(_) => RuleTraceOutboundFacts::observed(unknown_rule_trace_observation(scope)),
                None => RuleTraceOutboundFacts::Unavailable(Failure::new(
                    ErrorCode::NotReady,
                    "Core session is unavailable; local AST simulation only",
                    true,
                )),
            },
            None => RuleTraceOutboundFacts::Unavailable(Failure::new(
                ErrorCode::NotReady,
                "Core owner is unavailable; local AST simulation only",
                true,
            )),
        };
        let owner = match (&core.lifecycle, core.session_token.as_ref()) {
            (CoreLifecycle::Running | CoreLifecycle::Ready, Some(_)) => {
                RuleTraceSnapshotOwner::Live {
                    generation: core.generation,
                    revision: core.revision.max(1),
                }
            }
            _ => RuleTraceSnapshotOwner::Offline {
                generation: core.generation,
                revision: core.revision.max(1),
            },
        };
        self.simulate_workspace_snapshot(owner, rules, None, &outbound_facts)
    }

    fn simulate_workspace_snapshot(
        &self,
        owner: RuleTraceSnapshotOwner,
        rules: &[RuleEntry],
        sub_rules: Option<&BTreeMap<String, Vec<RuleEntry>>>,
        outbound_facts: &RuleTraceOutboundFacts,
    ) -> Result<RuleTracerSnapshot, RuleTraceIssue> {
        let (generation, revision, runtime_mode) = match owner {
            RuleTraceSnapshotOwner::Live {
                generation,
                revision,
            } => (generation, revision, RuleTraceRuntimeMode::Live),
            RuleTraceSnapshotOwner::Offline {
                generation,
                revision,
            } => (generation, revision, RuleTraceRuntimeMode::Offline),
        };
        let query = self.query();

        // When query is empty, provide a clean, ready-to-test sandbox with presets
        if query.trim().is_empty() {
            let mut snapshot = RuleTracerSnapshot::empty(generation, revision);
            snapshot.runtime_mode = runtime_mode;
            snapshot.outbound_resolution = RuleTraceOutboundResolution::NotEvaluated;
            return Ok(snapshot);
        }

        // The simulated sandbox environment (source IP / ports) the shared
        // engine merged onto the query; published as `simulated_context`.
        let simulated_context = self.merged_context(&query)?;
        let ctx_snapshot = simulated_context.snapshot();

        let (_matched, chain, outbound_resolution) =
            self.trace_with_outbounds(rules, sub_rules, &query, outbound_facts)?;

        let mut snapshot = RuleTracerSnapshot::ready(
            generation,
            revision,
            query,
            ctx_snapshot,
            Some(chain),
            rules.len(),
        );
        snapshot.runtime_mode = runtime_mode;
        snapshot.outbound_resolution = outbound_resolution;

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

fn resolve_rule_target<'a>(
    target: &str,
    observation: &'a OutboundObservationSnapshot,
) -> Option<&'a OutboundId> {
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = observation.inventory().inventory()
    else {
        return None;
    };

    let matching_collections: Vec<_> = collections
        .iter()
        .filter(|collection| collection.id.value() == target)
        .collect();
    match matching_collections.as_slice() {
        [collection] => {
            return match &collection.selection {
                OutboundSelectionState::Ready { selected } => Some(selected),
                OutboundSelectionState::Unknown
                | OutboundSelectionState::Empty
                | OutboundSelectionState::Unsupported
                | OutboundSelectionState::Failed { .. } => None,
            };
        }
        [] => {}
        _ => return None,
    }

    let mut matching_entries = entries.iter().filter(|entry| {
        entry.id.value() == target || entry.display_name.as_deref() == Some(target)
    });
    let first = matching_entries.next()?;
    if matching_entries.any(|entry| entry.id != first.id) {
        return None;
    }
    Some(&first.id)
}
