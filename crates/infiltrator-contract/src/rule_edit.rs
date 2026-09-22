//! Shared rule-edit vocabulary (DUAL-11-09 / 11-10 / 11-11 / 11-12).
//!
//! The two surfaces edit the same ordered `rules:` list but hold it in
//! different in-memory shapes. The direction of a one-step reorder and the
//! wizard draft fields are therefore expressed once here so the domain
//! reduction and the command bus agree on the same typed input.

use serde::{Deserialize, Serialize};

/// Shared direction for a one-step rule reorder (DUAL-11-10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleMoveDirection {
    /// Swap the rule with the entry directly above it.
    Up,
    /// Swap the rule with the entry directly below it.
    Down,
}

/// Structured fields captured by the add-rule wizard (DUAL-11-11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDraft {
    pub rule_type: String,
    pub payload: String,
    pub target: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_move_direction_round_trips() {
        let up = serde_json::to_string(&RuleMoveDirection::Up).unwrap();
        assert_eq!(up, "\"Up\"");
        let down: RuleMoveDirection = serde_json::from_str("\"Down\"").unwrap();
        assert_eq!(down, RuleMoveDirection::Down);
    }

    #[test]
    fn rule_draft_carries_structured_fields() {
        let draft = RuleDraft {
            rule_type: "DOMAIN-SUFFIX".to_owned(),
            payload: "github.com".to_owned(),
            target: "PROXY".to_owned(),
        };
        assert_eq!(draft.rule_type, "DOMAIN-SUFFIX");
        assert_eq!(draft.payload, "github.com");
        assert_eq!(draft.target, "PROXY");
    }
}
