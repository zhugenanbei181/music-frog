//! One localized fold of simulation provenance and all five stage facts.
use crate::rule_condition_projection::{evaluation_text, issue_text, location_text};
use crate::rule_trace_actions::RuleTraceActions;
use infiltrator_contract::rule_trace_facts::DecisionStageFacts;
use infiltrator_contract::rule_tracer::{
    DecisionChainNode, DecisionChainSnapshot, DecisionNodeStatus,
};
use infiltrator_shared::i18n_interpolator::localize;

pub fn present_chain(chain: &DecisionChainSnapshot, code: &str) -> DecisionChainSnapshot {
    let mut display = chain.clone();
    for node in &mut display.nodes {
        present_node(node, code);
    }
    display
}
pub fn trace_status(state: &RuleTraceActions, code: &str) -> String {
    if let Some(issue) = state
        .snapshot
        .issue
        .as_ref()
        .filter(|_| state.snapshot.operation_id == state.requested)
    {
        let message = localize(
            code,
            "rule_trace_issue_at",
            &[
                ("index", (issue.index + 1).to_string()),
                ("reason", issue_text(&issue.issue, code)),
            ],
        );
        match &issue.location {
            Some(location) => format!("{} · {message}", location_text(location, code)),
            None => message,
        }
    } else if let Some(failure) = state.current_failure() {
        localize(
            code,
            "rule_trace_failure",
            &[("reason", failure.message.clone())],
        )
    } else if state.busy() {
        localize(code, "rule_trace_running", &[])
    } else if state.requested.is_some() && state.requested != state.snapshot.report_id {
        localize(code, "rule_trace_waiting", &[])
    } else {
        localize(code, "rule_trace_simulation_notice", &[])
    }
}
pub fn trace_provenance(state: &RuleTraceActions, code: &str) -> String {
    let Some(report) = &state.snapshot.report else {
        return String::new();
    };
    localize(
        code,
        if state.current_failure().is_some()
            || state.busy()
            || state.requested != state.snapshot.report_id
            || !state.draft_matches(report)
        {
            "rule_trace_previous_report"
        } else {
            "rule_trace_current_report"
        },
        &[],
    )
}
pub fn trace_headline(chain: Option<&DecisionChainSnapshot>, code: &str) -> String {
    match chain {
        Some(chain) if !chain.is_fallback => localize(
            code,
            "rule_trace_matched",
            &[
                (
                    "index",
                    chain
                        .hit_rule_index
                        .map(|index| (index + 1).to_string())
                        .unwrap_or_default(),
                ),
                ("rule", chain.matched_rule_raw.clone()),
                ("target", chain.target_proxy.clone()),
            ],
        ),
        Some(chain) => localize(
            code,
            "rule_trace_fallback",
            &[("target", chain.target_proxy.clone())],
        ),
        None => localize(code, "rule_trace_no_report", &[]),
    }
}
pub fn stage_text(node: &DecisionChainNode, code: &str) -> String {
    let mut node = node.clone();
    present_node(&mut node, code);
    let status = localize(code, status_key(node.status), &[]);
    let mut lines = vec![format!("{} · {}", node.title, status), node.detail];
    if let Some(badge) = node.badge {
        lines.push(badge);
    }
    lines.extend(node.sub_evaluations);
    lines.join("\n")
}
fn present_node(node: &mut DecisionChainNode, code: &str) {
    let unknown = || localize(code, "rule_trace_unknown", &[]);
    let optional = |value: &Option<String>| value.clone().unwrap_or_else(unknown);
    let port = |value: Option<u16>| value.map(|value| value.to_string()).unwrap_or_else(unknown);
    let Some(facts) = &node.facts else {
        node.title = localize(code, "rule_trace_legacy_stage", &[]);
        node.detail = localize(code, "rule_trace_unknown", &[]);
        node.badge = None;
        node.sub_evaluations.clear();
        node.status = DecisionNodeStatus::Neutral;
        return;
    };
    match facts {
        DecisionStageFacts::Inbound(context) => {
            node.title = localize(code, "rule_trace_inbound", &[]);
            node.detail = localize(
                code,
                "rule_trace_inbound_values",
                &[
                    ("ip", optional(&context.src_ip)),
                    ("source_port", port(context.src_port)),
                    ("inbound_port", port(context.in_port)),
                    ("network", optional(&context.network)),
                    ("kind", optional(&context.in_type)),
                ],
            );
            node.status = DecisionNodeStatus::Neutral;
            node.badge = None;
            node.sub_evaluations.clear();
        }
        DecisionStageFacts::Sniffer {
            domain,
            ip,
            port: destination,
        } => {
            node.title = localize(code, "rule_trace_sniffer", &[]);
            node.detail = localize(code, "rule_trace_sniffer_not_observed", &[]);
            node.status = DecisionNodeStatus::Neutral;
            node.badge = None;
            node.sub_evaluations = vec![localize(
                code,
                "rule_trace_query_values",
                &[
                    ("domain", optional(domain)),
                    ("ip", optional(ip)),
                    ("port", port(*destination)),
                ],
            )];
        }
        DecisionStageFacts::Rule {
            index,
            raw,
            target,
            no_resolve,
            evaluations,
            path,
        } => {
            node.title = localize(code, "rule_trace_rule", &[]);
            node.detail = match index {
                Some(index) => localize(
                    code,
                    "rule_trace_rule_values",
                    &[
                        ("index", (index + 1).to_string()),
                        ("rule", raw.clone()),
                        ("target", target.clone()),
                    ],
                ),
                None => localize(code, "rule_trace_implicit_direct", &[]),
            };
            node.badge = no_resolve.map(|value| format!("no-resolve={value}"));
            node.sub_evaluations = evaluations
                .iter()
                .map(|row| evaluation_text(row, code))
                .collect();
            if path.iter().any(|entry| entry.location.table.is_some()) {
                node.sub_evaluations.extend(path.iter().map(|entry| {
                    format!("{} · {}", location_text(&entry.location, code), entry.raw)
                }));
            }
        }
        DecisionStageFacts::Policy { target, selected } => {
            node.title = localize(code, "rule_trace_policy", &[]);
            node.detail = localize(
                code,
                "rule_trace_policy_values",
                &[("target", target.clone()), ("selected", optional(selected))],
            );
            node.badge = Some(target.clone());
            node.sub_evaluations.clear();
        }
        DecisionStageFacts::Outbound {
            name,
            protocol,
            delay_ms,
            country,
        } => {
            node.title = localize(code, "rule_trace_outbound", &[]);
            node.detail = localize(
                code,
                "rule_trace_outbound_values",
                &[
                    ("name", optional(name)),
                    ("protocol", optional(protocol)),
                    (
                        "delay",
                        delay_ms
                            .map(|value| format!("{value} ms"))
                            .unwrap_or_else(unknown),
                    ),
                    ("country", optional(country)),
                ],
            );
            node.badge = country.clone();
            node.sub_evaluations.clear();
        }
    }
}
fn status_key(status: DecisionNodeStatus) -> &'static str {
    match status {
        DecisionNodeStatus::Neutral => "rule_trace_status_unknown",
        DecisionNodeStatus::Matched => "rule_trace_status_matched",
        DecisionNodeStatus::Passed => "rule_trace_status_passed",
        DecisionNodeStatus::Failed => "rule_trace_status_failed",
        DecisionNodeStatus::Bypassed => "rule_trace_status_bypassed",
        DecisionNodeStatus::Fallback => "rule_trace_status_fallback",
    }
}
#[cfg(test)]
#[path = "rule_trace_projection_test.rs"]
mod tests;
