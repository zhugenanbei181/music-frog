//! Ordered named-table execution; a table name is never an outbound policy.
use super::evaluation::{evaluate_ast, evaluate_rule};
use super::{RuleTraceMatch, TrafficContext, format_matched_rule_desc};
use crate::rules::RuleEntry;
use crate::rules::types::{RuleType, parse_rule_str};
use crate::sub_rules::LogicalRuleAst;
use infiltrator_contract::rule_condition::{ConditionIssue, ConditionOutcome, RuleTraceIssue};
use infiltrator_contract::rule_location::{RuleLocation, RulePathEntry};
use std::collections::BTreeMap;

pub fn trace_named_rules(
    rules: &[RuleEntry],
    tables: &BTreeMap<String, Vec<RuleEntry>>,
    context: &TrafficContext,
) -> Result<Option<RuleTraceMatch>, RuleTraceIssue> {
    trace_in_tables(rules, Some(tables), context)
}
pub(super) fn trace_in_tables(
    rules: &[RuleEntry],
    tables: Option<&BTreeMap<String, Vec<RuleEntry>>>,
    context: &TrafficContext,
) -> Result<Option<RuleTraceMatch>, RuleTraceIssue> {
    let mut trace = TableTrace {
        tables,
        context,
        active: Vec::new(),
    };
    trace.run(rules, None, &[])
}
struct TableTrace<'a> {
    tables: Option<&'a BTreeMap<String, Vec<RuleEntry>>>,
    context: &'a TrafficContext,
    active: Vec<String>,
}
impl TableTrace<'_> {
    fn run(
        &mut self,
        rules: &[RuleEntry],
        table: Option<&str>,
        path: &[RulePathEntry],
    ) -> Result<Option<RuleTraceMatch>, RuleTraceIssue> {
        for (index, entry) in rules.iter().enumerate() {
            if !entry.enabled {
                continue;
            }
            let parsed = parse_rule_str(&entry.rule)
                .map_err(|error| issue(table, index, entry, error.to_string()))?;
            let location = RuleLocation {
                table: table.map(str::to_owned),
                index,
            };
            let mut next_path = path.to_vec();
            next_path.push(RulePathEntry {
                location: location.clone(),
                raw: entry.rule.clone(),
            });
            if let RuleType::Logical(logical) = &parsed.rule_type
                && matches!(logical.payload, LogicalRuleAst::SubRule(_))
                && let Some(tables) = self.tables
            {
                match evaluate_ast(&logical.payload, self.context) {
                    ConditionOutcome::NotMatched => continue,
                    ConditionOutcome::Unresolved(reason) => {
                        return Err(RuleTraceIssue {
                            index,
                            location: Some(RuleLocation {
                                table: table.map(str::to_owned),
                                index,
                            }),
                            rule: entry.rule.clone(),
                            issue: reason,
                        });
                    }
                    ConditionOutcome::Matched => {}
                }
                if self.active.contains(&logical.target) || self.active.len() >= 64 {
                    return Err(issue(
                        table,
                        index,
                        entry,
                        format!(
                            "Cyclic or excessively deep sub-rule reference: {}",
                            logical.target
                        ),
                    ));
                }
                let Some(children) = tables.get(&logical.target) else {
                    return Err(issue(
                        table,
                        index,
                        entry,
                        format!("Missing sub-rule table: {}", logical.target),
                    ));
                };
                self.active.push(logical.target.clone());
                let result = self.run(children, Some(&logical.target), &next_path);
                self.active.pop();
                if let Some(matched) = result? {
                    return Ok(Some(matched));
                }
                continue;
            }
            match evaluate_rule(&parsed, self.context) {
                ConditionOutcome::NotMatched => continue,
                ConditionOutcome::Unresolved(reason) => {
                    return Err(RuleTraceIssue {
                        index,
                        location: Some(RuleLocation {
                            table: table.map(str::to_owned),
                            index,
                        }),
                        rule: entry.rule.clone(),
                        issue: reason,
                    });
                }
                ConditionOutcome::Matched => {}
            }
            return Ok(Some(RuleTraceMatch {
                index,
                rule: format_matched_rule_desc(&parsed),
                target: parsed.target,
                location,
                raw: entry.rule.clone(),
                path: next_path,
            }));
        }
        Ok(None)
    }
}
fn issue(table: Option<&str>, index: usize, entry: &RuleEntry, reason: String) -> RuleTraceIssue {
    RuleTraceIssue {
        index,
        location: Some(RuleLocation {
            table: table.map(str::to_owned),
            index,
        }),
        rule: entry.rule.clone(),
        issue: ConditionIssue::InvalidRule { reason },
    }
}

#[cfg(test)]
#[path = "named_test.rs"]
mod tests;
