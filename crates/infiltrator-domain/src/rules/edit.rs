//! Shared rule-list edit reductions (DUAL-11-09/10/11/12).
//!
//! The Iced editor holds a `Vec<RuleEntry>` it mutates locally before saving
//! the whole profile; the Bevy surface submits an intent the application
//! applies to the persisted profile. Both must agree on the exact list
//! arithmetic and on the wizard's rule string, so every mutation lives here as
//! a pure function over [`RuleEntry`].

use super::RuleEntry;
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};

/// Rule types offered by the add-rule wizard, in presentation order.
pub const CUSTOM_RULE_TYPE_CHOICES: [&str; 12] = [
    "DOMAIN",
    "DOMAIN-SUFFIX",
    "DOMAIN-KEYWORD",
    "IP-CIDR",
    "IP-CIDR6",
    "GEOIP",
    "MATCH",
    "RULE-SET",
    "AND",
    "OR",
    "NOT",
    "SUB-RULE",
];

/// Default outbound target the wizard and the game-preset injector start from.
pub const DEFAULT_RULE_TARGET: &str = "PROXY";

/// Whether `rule_type` composes sub-rules and therefore takes the
/// `TYPE((payload),target)` shape with shared syntax validation.
pub fn is_logical_rule_type(rule_type: &str) -> bool {
    matches!(
        rule_type.trim().to_ascii_uppercase().as_str(),
        "AND" | "OR" | "NOT" | "SUB-RULE"
    )
}

/// Build a rule entry from the wizard's structured draft. Rejects an empty
/// payload and, for logical types, runs the same
/// [`crate::sub_rules::validate_logical_rule_syntax`] check both surfaces use.
pub fn build_custom_rule(draft: &RuleDraft) -> Result<RuleEntry, String> {
    let payload = draft.payload.trim();
    if payload.is_empty() {
        return Err("Payload cannot be empty".to_owned());
    }
    let rule_type = draft.rule_type.trim();
    let target = draft.target.trim();
    let rule = if is_logical_rule_type(rule_type) {
        format!("{rule_type}({payload},{target})")
    } else {
        format!("{rule_type},{payload},{target}")
    };
    if is_logical_rule_type(rule_type) {
        crate::sub_rules::validate_logical_rule_syntax(&rule).map_err(|error| error.to_string())?;
    }
    Ok(RuleEntry {
        rule,
        enabled: true,
    })
}

/// Flip `enabled` on `rules[index]`. Returns `false` when the index is out of
/// range, so a stale UI click is a typed no-op rather than a panic.
pub fn toggle_rule_enabled(rules: &mut [RuleEntry], index: usize) -> bool {
    match rules.get_mut(index) {
        Some(entry) => {
            entry.enabled = !entry.enabled;
            true
        }
        None => false,
    }
}

/// Swap `rules[index]` one step in `direction`. Returns `false` when the move
/// is blocked by the list boundary or the index is out of range.
pub fn move_rule(rules: &mut [RuleEntry], index: usize, direction: RuleMoveDirection) -> bool {
    match direction {
        RuleMoveDirection::Up => {
            if index == 0 || index >= rules.len() {
                return false;
            }
            rules.swap(index, index - 1);
            true
        }
        RuleMoveDirection::Down => {
            if index + 1 >= rules.len() {
                return false;
            }
            rules.swap(index, index + 1);
            true
        }
    }
}

/// Prepend `incoming` at the top of the list while preserving its own order.
/// New rules and injected presets both take highest priority. Returns the
/// number of entries inserted.
pub fn prepend_rules(
    rules: &mut Vec<RuleEntry>,
    incoming: impl IntoIterator<Item = RuleEntry>,
) -> usize {
    let batch: Vec<RuleEntry> = incoming.into_iter().collect();
    let count = batch.len();
    for entry in batch.into_iter().rev() {
        rules.insert(0, entry);
    }
    count
}

/// Prepend the built-in game-routing presets for `target` (DUAL-11-12).
pub fn inject_game_presets(rules: &mut Vec<RuleEntry>, target: &str) -> usize {
    prepend_rules(rules, super::game_routing_presets(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(rule: &str, enabled: bool) -> RuleEntry {
        RuleEntry {
            rule: rule.to_owned(),
            enabled,
        }
    }

    #[test]
    fn toggle_flips_in_range_and_rejects_stale_index() {
        let mut rules = vec![entry("DOMAIN,a.com,DIRECT", true)];
        assert!(toggle_rule_enabled(&mut rules, 0));
        assert!(!rules[0].enabled);
        assert!(toggle_rule_enabled(&mut rules, 0));
        assert!(rules[0].enabled);
        assert!(!toggle_rule_enabled(&mut rules, 4));
    }

    #[test]
    fn move_swaps_within_bounds_and_stops_at_edges() {
        let mut rules = vec![
            entry("A,1,DIRECT", true),
            entry("B,2,DIRECT", true),
            entry("C,3,DIRECT", true),
        ];
        assert!(move_rule(&mut rules, 2, RuleMoveDirection::Up));
        assert_eq!(rules[1].rule, "C,3,DIRECT");
        assert!(!move_rule(&mut rules, 0, RuleMoveDirection::Up));
        assert!(!move_rule(&mut rules, 2, RuleMoveDirection::Down));
        assert!(move_rule(&mut rules, 0, RuleMoveDirection::Down));
        assert_eq!(rules[0].rule, "C,3,DIRECT");
        assert_eq!(rules[1].rule, "A,1,DIRECT");
    }

    #[test]
    fn wizard_builds_flat_and_logical_rules() {
        let flat = build_custom_rule(&RuleDraft {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "github.com".to_owned(),
            target: "PROXY".to_owned(),
        })
        .unwrap();
        assert_eq!(flat.rule, "DOMAIN-SUFFIX,github.com,PROXY");
        assert!(flat.enabled);

        let logical = build_custom_rule(&RuleDraft {
            rule_type: "AND".to_owned(),
            payload: "(DOMAIN,a.com),(DST-PORT,443)".to_owned(),
            target: "AI".to_owned(),
        })
        .unwrap();
        assert_eq!(logical.rule, "AND((DOMAIN,a.com),(DST-PORT,443),AI)");

        assert!(
            build_custom_rule(&RuleDraft {
                rule_type: "DOMAIN".to_owned(),
                payload: "   ".to_owned(),
                target: "PROXY".to_owned(),
            })
            .is_err()
        );
        assert!(
            build_custom_rule(&RuleDraft {
                rule_type: "AND".to_owned(),
                payload: "(DOMAIN,a.com".to_owned(),
                target: "AI".to_owned(),
            })
            .is_err()
        );
    }

    #[test]
    fn prepend_keeps_batch_order_and_counts() {
        let mut rules = vec![entry("MATCH,DIRECT", true)];
        let inserted = prepend_rules(
            &mut rules,
            vec![entry("A,1,DIRECT", true), entry("B,2,DIRECT", true)],
        );
        assert_eq!(inserted, 2);
        assert_eq!(rules[0].rule, "A,1,DIRECT");
        assert_eq!(rules[1].rule, "B,2,DIRECT");
        assert_eq!(rules[2].rule, "MATCH,DIRECT");
    }

    #[test]
    fn game_presets_use_shared_list_and_target() {
        let mut rules = vec![entry("MATCH,DIRECT", true)];
        let inserted = inject_game_presets(&mut rules, "Game-Proxy");
        assert_eq!(
            inserted,
            super::super::game_routing_presets("Game-Proxy").len()
        );
        assert!(rules[0].rule.contains("Game-Proxy"));
        assert!(rules[0].rule.starts_with("PROCESS-NAME"));
        assert_eq!(rules.last().unwrap().rule, "MATCH,DIRECT");
    }
}
