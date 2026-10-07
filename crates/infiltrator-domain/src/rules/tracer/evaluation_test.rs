//! Sound uncertainty, logical short-circuiting and ordered first-match behavior.
use super::*;
use crate::rules::RuleEntry;
use crate::rules::tracer::trace_rules;
use crate::rules::types::parse_rule_str;
use infiltrator_contract::rule_condition::{ConditionIssue, ConditionOutcome, TrafficField};

fn outcome(raw: &str, context: &TrafficContext) -> ConditionOutcome {
    evaluate_rule(&parse_rule_str(raw).expect("rule syntax"), context)
}
#[test]
fn external_rule_data_is_never_guessed_from_names_or_domains() {
    for kind in [
        "GEOSITE",
        "GEOIP",
        "SRC-GEOIP",
        "IP-ASN",
        "SRC-IP-ASN",
        "RULE-SET",
    ] {
        let raw = format!("{kind},cn,DIRECT");
        assert_eq!(
            outcome(&raw, &TrafficContext::from_domain("cn")),
            ConditionOutcome::Unresolved(ConditionIssue::ExternalData {
                rule_type: kind.into(),
                name: "cn".into()
            })
        );
        let rules = vec![
            RuleEntry {
                rule: raw.clone(),
                enabled: true,
            },
            RuleEntry {
                rule: "MATCH,REJECT".into(),
                enabled: true,
            },
        ];
        let issue = trace_rules(&rules, &TrafficContext::from_domain("cn"))
            .expect_err("no guessed match or fallthrough");
        assert_eq!(issue.index, 0);
        assert_eq!(issue.rule, raw);
    }
}
#[test]
fn missing_context_does_not_become_false_or_a_negated_match() {
    let context = TrafficContext::from_domain("example.org");
    assert_eq!(
        outcome("NOT,((NETWORK,udp)),DIRECT", &context),
        ConditionOutcome::Unresolved(ConditionIssue::MissingInput(TrafficField::Network))
    );
    assert_eq!(
        outcome("AND,((DOMAIN,other.org),(NETWORK,udp)),DIRECT", &context),
        ConditionOutcome::NotMatched
    );
    assert_eq!(
        outcome("OR,((DOMAIN,example.org),(NETWORK,udp)),DIRECT", &context),
        ConditionOutcome::Matched
    );
    assert_eq!(
        outcome(
            "NOT,((NETWORK,udp)),DIRECT",
            &context.clone().with_network("tcp")
        ),
        ConditionOutcome::Matched
    );
    assert_eq!(
        outcome("NOT,((NETWORK,udp)),DIRECT", &context.with_network("udp")),
        ConditionOutcome::NotMatched
    );
}
#[test]
fn unresolved_earlier_rules_block_later_matches_but_unreachable_ones_do_not() {
    let first = RuleEntry {
        rule: "DOMAIN,example.org,DIRECT".into(),
        enabled: true,
    };
    let unresolved = RuleEntry {
        rule: "SRC-IP-CIDR,10.0.0.0/8,REJECT".into(),
        enabled: true,
    };
    let context = TrafficContext::from_domain("example.org");
    let result = trace_rules(&[first.clone(), unresolved.clone()], &context)
        .unwrap()
        .unwrap();
    assert_eq!(result.index, 0);
    assert_eq!(result.target, "DIRECT");
    let issue = trace_rules(&[unresolved, first], &context).unwrap_err();
    assert_eq!(
        issue.issue,
        ConditionIssue::MissingInput(TrafficField::SourceIp)
    );
    let result = trace_rules(
        &[
            RuleEntry {
                rule: "SRC-IP-CIDR,10.0.0.0/8,REJECT".into(),
                enabled: true,
            },
            RuleEntry {
                rule: "DOMAIN,example.org,DIRECT".into(),
                enabled: true,
            },
        ],
        &context.with_src_ip("192.0.2.1".parse().unwrap()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.index, 1);
}
#[test]
fn no_resolve_and_invalid_conditions_keep_distinct_meanings() {
    let context = TrafficContext::from_domain("example.org");
    assert_eq!(
        outcome("IP-CIDR,10.0.0.0/8,DIRECT", &context),
        ConditionOutcome::Unresolved(ConditionIssue::MissingInput(TrafficField::DestinationIp))
    );
    assert_eq!(
        outcome("IP-CIDR,10.0.0.0/8,DIRECT,no-resolve", &context),
        ConditionOutcome::NotMatched
    );
    for raw in [
        "DOMAIN-REGEX,[,DIRECT",
        "IP-CIDR,10.0.0.0/90,DIRECT",
        "DST-PORT,70000,DIRECT",
        "FUTURE,candidate,DIRECT",
    ] {
        assert!(
            matches!(
                outcome(raw, &context),
                ConditionOutcome::Unresolved(ConditionIssue::InvalidRule { .. })
            ),
            "{raw}"
        );
    }
    let exact = TrafficContext::from_domain("EXAMPLE.ORG.");
    assert_eq!(
        outcome("DOMAIN,example.org,DIRECT", &exact),
        ConditionOutcome::Matched
    );
}
#[test]
fn logical_explanations_keep_structure_outcomes_and_opaque_expressions() {
    let parsed = parse_rule_str("NOT,((NETWORK,udp)),DIRECT").unwrap();
    let RuleType::Logical(rule) = parsed.rule_type else {
        panic!("logical AST");
    };
    let rows = explain_logical_ast(&rule.payload, &TrafficContext::from_domain("example.org"));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].kind, EvaluationNodeKind::Not);
    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[1].kind, EvaluationNodeKind::Leaf);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].expression.as_deref(), Some("NETWORK,udp"));
    assert_eq!(
        rows[0].outcome,
        ConditionOutcome::Unresolved(ConditionIssue::MissingInput(TrafficField::Network))
    );
    assert_eq!(rows[1].outcome, rows[0].outcome);
}
