//! Three-valued rule evaluation with typed evidence; no dataset-name heuristics.
use super::{TrafficContext, matches_cidr, matches_port, parse_cidr};
use crate::rules::types::{ParsedRule, RuleType, parse_rule_condition};
use crate::sub_rules::LogicalRuleAst;
use infiltrator_contract::rule_condition::{
    ConditionIssue, ConditionOutcome, EvaluationNodeKind, RuleEvaluationNode, TrafficField,
};
use regex::Regex;
use std::net::IpAddr;

pub fn evaluate_rule(rule: &ParsedRule, context: &TrafficContext) -> ConditionOutcome {
    evaluate_type(&rule.rule_type, context, rule.no_resolve, rule.source_ip)
}
fn missing(field: TrafficField) -> ConditionOutcome {
    ConditionOutcome::Unresolved(ConditionIssue::MissingInput(field))
}
fn result(matched: bool) -> ConditionOutcome {
    if matched {
        ConditionOutcome::Matched
    } else {
        ConditionOutcome::NotMatched
    }
}
fn invalid(reason: impl Into<String>) -> ConditionOutcome {
    ConditionOutcome::Unresolved(ConditionIssue::InvalidRule {
        reason: reason.into(),
    })
}
fn text(value: Option<&str>, expected: &str, field: TrafficField) -> ConditionOutcome {
    value.map_or_else(
        || missing(field),
        |value| result(value.eq_ignore_ascii_case(expected)),
    )
}
fn pattern(value: Option<&str>, pattern: &str, field: TrafficField) -> ConditionOutcome {
    let regex = match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(error) => return invalid(error.to_string()),
    };
    value.map_or_else(|| missing(field), |value| result(regex.is_match(value)))
}
fn port(value: Option<u16>, spec: &str, field: TrafficField) -> ConditionOutcome {
    if spec.split(['/', ',']).any(|part| {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-').or_else(|| part.split_once(':')) {
            !matches!((start.parse::<u16>(), end.parse::<u16>()), (Ok(start), Ok(end)) if start > 0 && start <= end)
        } else { !matches!(part.parse::<u16>(), Ok(port) if port > 0) }
    }) { return invalid(format!("Invalid port expression: {spec}")); }
    value.map_or_else(|| missing(field), |value| result(matches_port(spec, value)))
}
fn ip_suffix(spec: &str, ip: Option<IpAddr>, source: bool, no_resolve: bool) -> ConditionOutcome {
    let Some((network, bits)) = spec
        .split_once('/')
        .and_then(|(ip, bits)| Some((ip.parse::<IpAddr>().ok()?, bits.parse::<u32>().ok()?)))
    else {
        return invalid(format!("Invalid IP suffix: {spec}"));
    };
    let width = if network.is_ipv4() { 32 } else { 128 };
    if bits > width {
        return invalid(format!("Invalid IP suffix width: {spec}"));
    }
    let Some(ip) = ip else {
        return if !source && no_resolve {
            ConditionOutcome::NotMatched
        } else {
            missing(if source {
                TrafficField::SourceIp
            } else {
                TrafficField::DestinationIp
            })
        };
    };
    let (expected, actual) = match (network, ip) {
        (IpAddr::V4(expected), IpAddr::V4(actual)) => {
            (u32::from(expected) as u128, u32::from(actual) as u128)
        }
        (IpAddr::V6(expected), IpAddr::V6(actual)) => (u128::from(expected), u128::from(actual)),
        _ => return ConditionOutcome::NotMatched,
    };
    let mask = if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    };
    result(expected & mask == actual & mask)
}
fn evaluate_type(
    rule: &RuleType,
    context: &TrafficContext,
    no_resolve: bool,
    source_ip: bool,
) -> ConditionOutcome {
    match rule {
        RuleType::Domain(expected)
        | RuleType::DomainSuffix(expected)
        | RuleType::DomainKeyword(expected)
        | RuleType::DomainRegex(expected) => {
            let Some(domain) = context.domain.as_deref() else {
                return if context.ip.is_some() {
                    ConditionOutcome::NotMatched
                } else {
                    missing(TrafficField::Destination)
                };
            };
            let domain = domain.trim_end_matches('.');
            match rule {
                RuleType::Domain(_) => {
                    result(domain.eq_ignore_ascii_case(expected.trim_end_matches('.')))
                }
                RuleType::DomainSuffix(_) => {
                    let suffix = expected
                        .trim_start_matches('.')
                        .trim_end_matches('.')
                        .to_ascii_lowercase();
                    let domain = domain.to_ascii_lowercase();
                    result(domain == suffix || domain.ends_with(&format!(".{suffix}")))
                }
                RuleType::DomainKeyword(_) => result(
                    domain
                        .to_ascii_lowercase()
                        .contains(&expected.to_ascii_lowercase()),
                ),
                RuleType::DomainRegex(_) => {
                    pattern(Some(domain), expected, TrafficField::Destination)
                }
                _ => unreachable!(),
            }
        }
        RuleType::IpCidr(cidr) | RuleType::IpCidr6(cidr) | RuleType::SrcIpCidr(cidr) => {
            let Some((network, prefix)) = parse_cidr(cidr) else {
                return invalid(format!("Invalid CIDR: {cidr}"));
            };
            if prefix > if network.is_ipv4() { 32 } else { 128 } {
                return invalid(format!("Invalid CIDR prefix: {cidr}"));
            }
            let source = source_ip || matches!(rule, RuleType::SrcIpCidr(_));
            let ip = if source {
                context.src_ip.or(context.client_ip)
            } else {
                context.ip
            };
            match ip {
                Some(ip) => result(matches_cidr(cidr, ip)),
                None if !source && no_resolve => ConditionOutcome::NotMatched,
                None => missing(if source {
                    TrafficField::SourceIp
                } else {
                    TrafficField::DestinationIp
                }),
            }
        }
        RuleType::IpSuffix(suffix) | RuleType::SrcIpSuffix(suffix) => {
            let source = source_ip || matches!(rule, RuleType::SrcIpSuffix(_));
            ip_suffix(
                suffix,
                if source {
                    context.src_ip.or(context.client_ip)
                } else {
                    context.ip
                },
                source,
                no_resolve,
            )
        }
        RuleType::Geosite(name)
        | RuleType::IpAsn(name)
        | RuleType::SrcIpAsn(name)
        | RuleType::GeoIp(name)
        | RuleType::SrcGeoIp(name)
        | RuleType::RuleSet(name) => ConditionOutcome::Unresolved(ConditionIssue::ExternalData {
            rule_type: rule.name().to_owned(),
            name: name.clone(),
        }),
        RuleType::DstPort(spec) => port(context.port, spec, TrafficField::DestinationPort),
        RuleType::SrcPort(spec) => port(context.src_port, spec, TrafficField::SourcePort),
        RuleType::InPort(spec) => port(context.in_port, spec, TrafficField::InboundPort),
        RuleType::InType(expected) => text(
            context.in_type.as_deref(),
            expected,
            TrafficField::InboundType,
        ),
        RuleType::InName(expected) => text(
            context.in_name.as_deref(),
            expected,
            TrafficField::InboundName,
        ),
        RuleType::InUser(expected) => text(
            context.in_user.as_deref(),
            expected,
            TrafficField::InboundUser,
        ),
        RuleType::ProcessName(expected) | RuleType::ProcessPath(expected) => {
            let (value, field) = if matches!(rule, RuleType::ProcessPath(_)) {
                (context.process_path.as_deref(), TrafficField::ProcessPath)
            } else {
                (context.process_name.as_deref(), TrafficField::ProcessName)
            };
            value.map_or_else(
                || missing(field),
                |value| result(value.eq_ignore_ascii_case(expected)),
            )
        }
        RuleType::ProcessPathRegex(expected) => pattern(
            context.process_path.as_deref(),
            expected,
            TrafficField::ProcessPath,
        ),
        RuleType::ProcessNameRegex(expected) => pattern(
            context.process_name.as_deref(),
            expected,
            TrafficField::ProcessName,
        ),
        RuleType::Network(expected) => {
            text(context.network.as_deref(), expected, TrafficField::Network)
        }
        RuleType::Dscp(expected) => match expected.parse::<u8>() {
            Ok(expected) if expected <= 63 => context.dscp.map_or_else(
                || missing(TrafficField::Dscp),
                |value| result(value == expected),
            ),
            _ => invalid("DSCP must be 0 through 63"),
        },
        RuleType::Uid(expected) => match expected.parse::<u32>() {
            Ok(expected) => context.uid.map_or_else(
                || missing(TrafficField::Uid),
                |value| result(value == expected),
            ),
            _ => invalid("Invalid UID"),
        },
        RuleType::PackageName(_) => {
            invalid("PACKAGE-NAME is unsupported by the locked mihomo kernel")
        }
        RuleType::Match => ConditionOutcome::Matched,
        RuleType::Logical(rule) => {
            let condition = evaluate_ast(&rule.payload, context);
            if matches!(rule.payload, LogicalRuleAst::SubRule(_))
                && condition == ConditionOutcome::Matched
            {
                ConditionOutcome::Unresolved(ConditionIssue::ExternalData {
                    rule_type: "SUB-RULE".into(),
                    name: rule.target.clone(),
                })
            } else {
                condition
            }
        }
        RuleType::Unknown(kind, _) => invalid(format!("Unknown rule type: {kind}")),
    }
}
fn leaf(payload: &str, context: &TrafficContext) -> ConditionOutcome {
    let payload = payload.trim();
    match parse_rule_condition(payload) {
        Ok(rule) => evaluate_rule(&rule, context),
        Err(error) => invalid(error.to_string()),
    }
}
pub fn evaluate_ast(ast: &LogicalRuleAst, context: &TrafficContext) -> ConditionOutcome {
    match ast {
        LogicalRuleAst::Leaf(payload) => leaf(&payload.0, context),
        LogicalRuleAst::Not(child) => match evaluate_ast(child, context) {
            ConditionOutcome::Matched => ConditionOutcome::NotMatched,
            ConditionOutcome::NotMatched => ConditionOutcome::Matched,
            unresolved => unresolved,
        },
        LogicalRuleAst::And(children) => combine(children, context, true),
        LogicalRuleAst::Or(children) | LogicalRuleAst::SubRule(children) => {
            combine(children, context, false)
        }
    }
}
fn combine(children: &[LogicalRuleAst], context: &TrafficContext, all: bool) -> ConditionOutcome {
    if children.is_empty() {
        return invalid("Empty logical rule");
    }
    let mut unresolved = None;
    for child in children {
        match evaluate_ast(child, context) {
            ConditionOutcome::Matched if !all => return ConditionOutcome::Matched,
            ConditionOutcome::NotMatched if all => return ConditionOutcome::NotMatched,
            ConditionOutcome::Unresolved(issue) => {
                unresolved.get_or_insert(issue);
            }
            _ => {}
        }
    }
    unresolved.map_or_else(|| result(all), ConditionOutcome::Unresolved)
}
pub fn explain_logical_ast(
    ast: &LogicalRuleAst,
    context: &TrafficContext,
) -> Vec<RuleEvaluationNode> {
    let mut rows = Vec::new();
    explain(ast, context, 0, &mut rows);
    rows
}
fn explain(
    ast: &LogicalRuleAst,
    context: &TrafficContext,
    depth: usize,
    rows: &mut Vec<RuleEvaluationNode>,
) {
    let kind = match ast {
        LogicalRuleAst::Leaf(_) => EvaluationNodeKind::Leaf,
        LogicalRuleAst::And(_) => EvaluationNodeKind::And,
        LogicalRuleAst::Or(_) => EvaluationNodeKind::Or,
        LogicalRuleAst::Not(_) => EvaluationNodeKind::Not,
        LogicalRuleAst::SubRule(_) => EvaluationNodeKind::SubRule,
    };
    rows.push(RuleEvaluationNode {
        depth,
        kind,
        expression: match ast {
            LogicalRuleAst::Leaf(payload) => Some(payload.0.clone()),
            _ => None,
        },
        outcome: evaluate_ast(ast, context),
    });
    match ast {
        LogicalRuleAst::And(children)
        | LogicalRuleAst::Or(children)
        | LogicalRuleAst::SubRule(children) => {
            for child in children {
                explain(child, context, depth + 1, rows);
            }
        }
        LogicalRuleAst::Not(child) => explain(child, context, depth + 1, rows),
        LogicalRuleAst::Leaf(_) => {}
    }
}

#[cfg(test)]
#[path = "evaluation_test.rs"]
mod tests;
