//! Shared logical sub-rule draft reduction (DUAL-11-02).
//!
//! The visual builder on both surfaces holds one [`LogicalDraft`]: an operator
//! (`AND`/`OR`/`NOT`/`SUB-RULE`), the conditions to compose and the outbound
//! target. Everything the surfaces do with it is a pure function here — the
//! canonical `OP((cond),(cond),TARGET)` encoding, the operator/target/condition
//! mutations and the validation gate — so the Iced panel and the Bevy card
//! cannot drift from each other or from `infiltrator_domain::sub_rules`.

use infiltrator_contract::rule_edit::{LogicalDraft, RuleDraft};

use super::RuleEntry;
use super::edit;
use super::matrix;
use super::types::parse_rule_str;

/// Operator vocabulary offered by the builder, in presentation order.
pub const LOGICAL_OPERATOR_CHOICES: [&str; 4] = ["AND", "OR", "NOT", "SUB-RULE"];

/// Ready-made conditions the builder offers as one-click inserts. A condition
/// is the `TYPE,PAYLOAD` half of a rule; the target belongs to the composition.
pub const SUB_RULE_CONDITION_PRESETS: [&str; 3] =
    ["DOMAIN-SUFFIX,google.com", "NETWORK,UDP", "DST-PORT,443"];

/// Operators that accept exactly one condition.
pub const SINGLE_CONDITION_OPERATORS: [&str; 1] = ["NOT"];

/// A fresh draft for the shared default target.
pub fn default_logical_draft(target: &str) -> LogicalDraft {
    LogicalDraft {
        operator: LOGICAL_OPERATOR_CHOICES[0].to_owned(),
        conditions: vec![
            "DOMAIN-SUFFIX,company.com".to_owned(),
            "NETWORK,TCP".to_owned(),
        ],
        target: target.trim().to_owned(),
    }
}

/// Whether `operator` is one of the builder's operators (case-insensitive).
pub fn is_operator(operator: &str) -> bool {
    let candidate = operator.trim().to_ascii_uppercase();
    LOGICAL_OPERATOR_CHOICES.contains(&candidate.as_str())
}

/// Whether the operator accepts exactly one condition.
pub fn is_single_condition_operator(operator: &str) -> bool {
    let candidate = operator.trim().to_ascii_uppercase();
    SINGLE_CONDITION_OPERATORS.contains(&candidate.as_str())
}

/// Select an operator. Returns `false` (typed no-op) for an unknown operator.
pub fn select_operator(draft: &mut LogicalDraft, operator: &str) -> bool {
    let candidate = operator.trim().to_ascii_uppercase();
    if !LOGICAL_OPERATOR_CHOICES.contains(&candidate.as_str()) {
        return false;
    }
    draft.operator = candidate;
    true
}

/// Set the composition target. An empty target is rejected: a logical rule
/// without a target cannot be persisted.
pub fn set_target(draft: &mut LogicalDraft, target: &str) -> bool {
    let candidate = target.trim();
    if candidate.is_empty() {
        return false;
    }
    draft.target = candidate.to_owned();
    true
}

/// Append a condition. Rejects an empty condition, an exact duplicate and a
/// second condition on a single-condition operator (see
/// [`SINGLE_CONDITION_OPERATORS`]). An already parenthesised input (the mihomo
/// wire form `(DOMAIN,a.com)`) is stored unwrapped: the encoder owns the
/// parentheses.
pub fn add_condition(draft: &mut LogicalDraft, condition: &str) -> bool {
    let candidate = unwrap_condition(condition);
    if candidate.is_empty() || draft.condition_index(&candidate).is_some() {
        return false;
    }
    if is_single_condition_operator(&draft.operator) && !draft.conditions.is_empty() {
        return false;
    }
    draft.conditions.push(candidate);
    true
}

/// Drop one balanced outer parenthesis pair, if the condition carries one.
fn unwrap_condition(condition: &str) -> String {
    let trimmed = condition.trim();
    let bytes = trimmed.as_bytes();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return trimmed.to_owned();
    }
    let mut depth = 0_i32;
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 && index + 1 < bytes.len() {
                    // The pair closes before the end: the text is not one
                    // wrapping pair, keep it as typed.
                    return trimmed.to_owned();
                }
            }
            _ => {}
        }
    }
    trimmed[1..trimmed.len() - 1].trim().to_owned()
}

/// Remove the condition at `index`. Returns `false` for a stale index.
pub fn remove_condition(draft: &mut LogicalDraft, index: usize) -> bool {
    if index >= draft.conditions.len() {
        return false;
    }
    draft.conditions.remove(index);
    true
}

/// The normalized operator spelling of the draft (`AND`, `OR`, `NOT`,
/// `SUB-RULE`), as both the preview and the built rule carry it.
pub fn draft_operator(draft: &LogicalDraft) -> String {
    draft.operator.trim().to_ascii_uppercase()
}

/// The conditions in the parenthesised payload form the parser expects:
/// `(DOMAIN,a.com),(DST-PORT,443)`.
pub fn draft_payload(conditions: &[String]) -> String {
    conditions
        .iter()
        .map(|condition| format!("({})", condition.trim()))
        .collect::<Vec<_>>()
        .join(",")
}

/// Canonical rule expression the draft encodes. For a valid draft this is
/// exactly the expression [`build_logical_rule`] persists; for an invalid one
/// it is what the surface previews while [`draft_issue`] explains the problem.
pub fn draft_expression(draft: &LogicalDraft) -> String {
    let operator = draft_operator(draft);
    let payload = draft_payload(&draft.conditions);
    let target = draft.target.trim();
    format!("{operator}({payload},{target})")
}

/// Whether one condition is structurally usable: a known rule type with a
/// non-empty payload (`DOMAIN,example.com`, `NETWORK,TCP`) or a nested logical
/// rule. The shared parser is the only judge.
pub fn condition_issue(condition: &str) -> Option<String> {
    let candidate = condition.trim();
    if candidate.is_empty() {
        return Some("Condition cannot be empty".to_owned());
    }
    if matcher_accepts(candidate) {
        return None;
    }
    Some(format!("Condition is not a TYPE,PAYLOAD pair: {candidate}"))
}

/// The first blocking issue of the draft, or `None` when it would build.
pub fn draft_issue(draft: &LogicalDraft) -> Option<String> {
    if !is_operator(&draft.operator) {
        return Some(format!(
            "Unknown logical operator: {}",
            draft.operator.trim()
        ));
    }
    if draft.target.trim().is_empty() {
        return Some("Logical rule needs an outbound target".to_owned());
    }
    if draft.conditions.is_empty() {
        return Some("Add at least one condition".to_owned());
    }
    if is_single_condition_operator(&draft.operator) && draft.conditions.len() != 1 {
        return Some(format!(
            "{} accepts exactly one condition",
            draft.operator.trim().to_ascii_uppercase()
        ));
    }
    for condition in &draft.conditions {
        if let Some(issue) = condition_issue(condition) {
            return Some(issue);
        }
    }
    None
}

/// Build the rule entry the draft encodes, or the first [`draft_issue`].
/// Both surfaces insert through this gate, so an invalid composition can never
/// reach the profile as a malformed rule string.
pub fn build_logical_rule(draft: &LogicalDraft) -> Result<RuleEntry, String> {
    if let Some(issue) = draft_issue(draft) {
        return Err(issue);
    }
    let operator = draft_operator(draft);
    let payload = draft_payload(&draft.conditions);
    edit::build_custom_rule(&RuleDraft {
        rule_type: operator,
        payload,
        target: draft.target.trim().to_owned(),
    })
}

/// A condition is usable when the shared parser accepts it as a `TYPE,PAYLOAD`
/// pair (or as a nested logical rule in either the parenthesised or the
/// `OP(...)` form).
fn matcher_accepts(condition: &str) -> bool {
    let head = condition
        .split([',', '('])
        .next()
        .unwrap_or_default()
        .trim();
    if matrix::matrix_is_logical(head) {
        return parse_rule_str(&format!("{condition},__logical_target__")).is_ok();
    }
    if matrix::spec_for_name(head).is_none() {
        return false;
    }
    parse_rule_str(&format!("{condition},__logical_target__")).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> LogicalDraft {
        default_logical_draft(edit::DEFAULT_RULE_TARGET)
    }

    #[test]
    fn default_draft_is_valid_and_builds_the_canonical_expression() {
        let draft = draft();
        assert_eq!(draft.operator, "AND");
        assert_eq!(draft.target, edit::DEFAULT_RULE_TARGET);
        assert_eq!(draft_issue(&draft), None);
        let built = build_logical_rule(&draft).unwrap();
        assert_eq!(
            built.rule,
            "AND((DOMAIN-SUFFIX,company.com),(NETWORK,TCP),PROXY)"
        );
        assert_eq!(draft_expression(&draft), built.rule);
        assert!(built.enabled);
    }

    #[test]
    fn mutations_reject_stale_and_duplicate_input() {
        let mut draft = draft();
        assert!(select_operator(&mut draft, "or"));
        assert_eq!(draft.operator, "OR");
        assert!(!select_operator(&mut draft, "XOR"));
        assert!(set_target(&mut draft, " Streaming "));
        assert_eq!(draft.target, "Streaming");
        assert!(!set_target(&mut draft, "   "));
        assert!(!add_condition(&mut draft, "  "));
        assert!(!add_condition(&mut draft, " NETWORK,TCP "));
        assert!(add_condition(&mut draft, "DST-PORT,443"));
        assert!(!remove_condition(&mut draft, 9));
        assert!(remove_condition(&mut draft, 0));
        assert_eq!(draft.conditions[0], "NETWORK,TCP");
    }

    #[test]
    fn single_condition_operator_refuses_a_second_condition() {
        let mut draft = draft();
        assert!(select_operator(&mut draft, "NOT"));
        assert!(!add_condition(&mut draft, "DST-PORT,443"));
        assert_eq!(
            draft_issue(&draft),
            Some("NOT accepts exactly one condition".to_owned())
        );
        assert!(remove_condition(&mut draft, 1));
        assert_eq!(draft_issue(&draft), None);
        assert_eq!(
            build_logical_rule(&draft).unwrap().rule,
            "NOT((DOMAIN-SUFFIX,company.com),PROXY)"
        );
    }

    #[test]
    fn invalid_conditions_are_rejected_before_building() {
        let mut draft = draft();
        draft.conditions.push("NOT-A-TYPE,x".to_owned());
        assert_eq!(
            draft_issue(&draft),
            Some("Condition is not a TYPE,PAYLOAD pair: NOT-A-TYPE,x".to_owned())
        );
        assert!(build_logical_rule(&draft).is_err());

        // A condition without a payload cannot build either.
        let mut bare = default_logical_draft("PROXY");
        bare.conditions = vec!["NETWORK".to_owned()];
        assert!(build_logical_rule(&bare).is_err());

        // An operator the vocabulary does not know never builds.
        let mut unknown = default_logical_draft("PROXY");
        unknown.operator = "XOR".to_owned();
        assert!(build_logical_rule(&unknown).is_err());
    }

    #[test]
    fn nested_conditions_keep_their_own_parentheses() {
        let mut draft = LogicalDraft {
            operator: "AND".to_owned(),
            conditions: Vec::new(),
            target: "AI".to_owned(),
        };
        assert!(add_condition(
            &mut draft,
            "AND((DOMAIN,a.com),(DST-PORT,443))"
        ));
        assert!(add_condition(&mut draft, "(DOMAIN-SUFFIX,b.com)"));
        assert_eq!(draft.conditions[1], "DOMAIN-SUFFIX,b.com");
        assert_eq!(draft_issue(&draft), None);
        let built = build_logical_rule(&draft).unwrap();
        assert_eq!(
            built.rule,
            "AND((AND((DOMAIN,a.com),(DST-PORT,443))),(DOMAIN-SUFFIX,b.com),AI)"
        );
        // The built expression parses back through the shared recursive AST,
        // so the visual builder and the evaluator agree on its shape.
        let parsed = parse_rule_str(&built.rule).unwrap();
        assert_eq!(parsed.rule_type.name(), "AND");
        assert_eq!(parsed.target, "AI");
        assert_eq!(draft_expression(&draft), built.rule);
    }
}
