//! Pure simulation facts for five stages; formatting belongs to the shared application fold.
use super::evaluation::explain_logical_ast;
use crate::rules::RuleEntry;
use crate::rules::tracer::{RuleTraceMatch, TrafficContext};
use crate::rules::types::{ParsedRule, RuleType};
use infiltrator_contract::rule_condition::{
    ConditionOutcome, EvaluationNodeKind, RuleEvaluationNode,
};
use infiltrator_contract::rule_trace_facts::DecisionStageFacts;
use infiltrator_contract::rule_tracer::{
    DecisionChainNode, DecisionChainSnapshot, DecisionNodeStatus, DecisionStageKind,
};

#[allow(clippy::too_many_arguments)]
pub fn build_decision_chain(
    _rules: &[RuleEntry],
    context: &TrafficContext,
    matched: Option<&RuleTraceMatch>,
    parsed_rule: Option<&ParsedRule>,
    final_node: Option<&str>,
    final_node_protocol: Option<&str>,
    final_node_delay_ms: Option<u32>,
    final_node_country: Option<&str>,
    match_latency_us: u64,
) -> DecisionChainSnapshot {
    let inbound = context.snapshot();
    let (index, raw, kind, payload, target, fallback, no_resolve, evaluations) =
        match (matched, parsed_rule) {
            (Some(matched), Some(parsed)) => (
                Some(matched.index),
                matched.raw.clone(),
                parsed.rule_type.name().to_owned(),
                parsed.rule_type.payload().unwrap_or("").to_owned(),
                matched.target.clone(),
                matches!(parsed.rule_type, RuleType::Match) && matched.location.table.is_none(),
                (!matches!(parsed.rule_type, RuleType::Logical(_))).then_some(parsed.no_resolve),
                match &parsed.rule_type {
                    RuleType::Logical(logical) => explain_logical_ast(&logical.payload, context),
                    _ => vec![RuleEvaluationNode {
                        depth: 0,
                        kind: EvaluationNodeKind::Leaf,
                        expression: Some(matched.rule.clone()),
                        outcome: ConditionOutcome::Matched,
                    }],
                },
            ),
            _ => (
                None,
                String::new(),
                "MATCH".to_owned(),
                String::new(),
                "DIRECT".to_owned(),
                true,
                None,
                Vec::new(),
            ),
        };
    let builtin = target == "DIRECT" || target == "REJECT";
    let outbound = final_node
        .map(str::to_owned)
        .or_else(|| builtin.then(|| target.clone()));
    let protocol = final_node_protocol
        .map(str::to_owned)
        .or_else(|| builtin.then(|| target.clone()));
    let nodes = vec![
        node(
            DecisionStageKind::Inbound,
            DecisionNodeStatus::Neutral,
            DecisionStageFacts::Inbound(inbound),
        ),
        node(
            DecisionStageKind::Sniffer,
            DecisionNodeStatus::Neutral,
            DecisionStageFacts::Sniffer {
                domain: context.domain.clone(),
                ip: context.ip.map(|ip| ip.to_string()),
                port: context.port,
            },
        ),
        node(
            DecisionStageKind::RuleSet,
            if fallback {
                DecisionNodeStatus::Fallback
            } else {
                DecisionNodeStatus::Matched
            },
            DecisionStageFacts::Rule {
                index,
                raw: raw.clone(),
                target: target.clone(),
                no_resolve,
                evaluations: evaluations.clone(),
                path: matched
                    .map(|matched| matched.path.clone())
                    .unwrap_or_default(),
            },
        ),
        node(
            DecisionStageKind::ProxyGroup,
            if outbound.is_some() {
                DecisionNodeStatus::Matched
            } else {
                DecisionNodeStatus::Neutral
            },
            DecisionStageFacts::Policy {
                target: target.clone(),
                selected: outbound.clone(),
            },
        ),
        node(
            DecisionStageKind::Outbound,
            if outbound.is_some() {
                DecisionNodeStatus::Matched
            } else {
                DecisionNodeStatus::Neutral
            },
            DecisionStageFacts::Outbound {
                name: outbound.clone(),
                protocol: protocol.clone(),
                delay_ms: final_node_delay_ms,
                country: final_node_country.map(str::to_owned),
            },
        ),
    ];
    DecisionChainSnapshot {
        nodes,
        hit_rule_index: index,
        hit_rule_table: matched.and_then(|matched| matched.location.table.clone()),
        rule_path: matched
            .map(|matched| matched.path.clone())
            .unwrap_or_default(),
        matched_rule_raw: raw,
        matched_rule_type: kind,
        matched_payload: payload,
        target_proxy: target,
        final_outbound: outbound.unwrap_or_default(),
        final_node_protocol: protocol,
        final_node_delay_ms,
        final_node_country: final_node_country.map(str::to_owned),
        match_latency_us,
        is_fallback: fallback,
    }
}
fn node(
    stage: DecisionStageKind,
    status: DecisionNodeStatus,
    facts: DecisionStageFacts,
) -> DecisionChainNode {
    DecisionChainNode {
        stage,
        facts: Some(facts),
        title: String::new(),
        detail: String::new(),
        badge: None,
        status,
        sub_evaluations: Vec::new(),
    }
}
