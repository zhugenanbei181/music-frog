//! Localize typed logical evaluations once; raw expressions stay opaque.
use infiltrator_contract::rule_condition::{
    ConditionIssue, ConditionOutcome, EvaluationNodeKind, RuleEvaluationNode, TrafficField,
};
use infiltrator_contract::rule_location::RuleLocation;
use infiltrator_shared::i18n_interpolator::localize;
pub fn issue_text(issue: &ConditionIssue, code: &str) -> String {
    match issue {
        ConditionIssue::MissingInput(field) => localize(
            code,
            "rule_trace_missing_input",
            &[("field", localize(code, field_key(*field), &[]))],
        ),
        ConditionIssue::ExternalData { rule_type, name } => localize(
            code,
            "rule_trace_external_missing",
            &[("type", rule_type.clone()), ("name", name.clone())],
        ),
        ConditionIssue::InvalidRule { reason } => localize(
            code,
            "rule_trace_invalid_rule",
            &[("reason", reason.clone())],
        ),
    }
}
pub fn evaluation_text(row: &RuleEvaluationNode, code: &str) -> String {
    let kind = match row.kind {
        EvaluationNodeKind::Leaf => "rule_trace_eval_leaf",
        EvaluationNodeKind::And => "rule_trace_eval_and",
        EvaluationNodeKind::Or => "rule_trace_eval_or",
        EvaluationNodeKind::Not => "rule_trace_eval_not",
        EvaluationNodeKind::SubRule => "rule_trace_eval_subrule",
    };
    let outcome = match &row.outcome {
        ConditionOutcome::Matched => localize(code, "rule_trace_eval_match", &[]),
        ConditionOutcome::NotMatched => localize(code, "rule_trace_eval_no_match", &[]),
        ConditionOutcome::Unresolved(issue) => issue_text(issue, code),
    };
    format!(
        "{}{}",
        "  ".repeat(row.depth),
        localize(
            code,
            "rule_trace_eval_row",
            &[
                ("kind", localize(code, kind, &[])),
                ("outcome", outcome),
                ("expression", row.expression.clone().unwrap_or_default())
            ]
        )
    )
}
pub const fn field_key(field: TrafficField) -> &'static str {
    match field {
        TrafficField::Destination => "rule_trace_field_destination",
        TrafficField::DestinationIp => "rule_trace_field_destination_ip",
        TrafficField::DestinationPort => "rule_trace_field_destination_port",
        TrafficField::SourceIp => "rule_trace_field_source_ip",
        TrafficField::SourcePort => "rule_trace_field_source_port",
        TrafficField::InboundPort => "rule_trace_field_inbound_port",
        TrafficField::InboundType => "rule_trace_field_inbound_type",
        TrafficField::InboundName => "rule_trace_field_inbound_name",
        TrafficField::InboundUser => "rule_trace_field_inbound_user",
        TrafficField::ProcessName => "rule_trace_field_process_name",
        TrafficField::ProcessPath => "rule_trace_field_process_path",
        TrafficField::Network => "rule_trace_field_network",
        TrafficField::Dscp => "rule_trace_field_dscp",
        TrafficField::Uid => "rule_trace_field_uid",
        TrafficField::PackageName => "rule_trace_field_package_name",
    }
}

pub fn location_text(location: &RuleLocation, code: &str) -> String {
    let mut arguments = vec![("index", (location.index + 1).to_string())];
    let key = match &location.table {
        Some(table) => {
            arguments.push(("table", table.clone()));
            "rule_trace_location_named"
        }
        None => "rule_trace_location_root",
    };
    localize(code, key, &arguments)
}
