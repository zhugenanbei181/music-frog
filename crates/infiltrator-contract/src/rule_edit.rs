//! Shared rule-edit vocabulary (DUAL-11-09 / 11-10 / 11-11 / 11-12 / 11-02).
//!
//! The two surfaces edit the same ordered `rules:` list but hold it in
//! different in-memory shapes. The direction of a one-step reorder, the
//! wizard draft fields and the logical sub-rule draft are therefore expressed
//! once here so the domain reduction and the command bus agree on the same
//! typed input.

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

/// Structured draft of the visual logical sub-rule builder (DUAL-11-02).
///
/// A condition is the `TYPE,PAYLOAD` half of a rule (`DOMAIN,example.com`,
/// `NETWORK,TCP`) or a complete nested logical rule; the target belongs to the
/// composed expression. Both surfaces hold this same draft and hand it to
/// `infiltrator_domain::rules::logical`, which owns the canonical
/// `OP((cond),(cond),TARGET)` encoding and its validation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalDraft {
    pub operator: String,
    pub conditions: Vec<String>,
    pub target: String,
}

impl LogicalDraft {
    /// The position of `condition` in the draft, when present.
    pub fn condition_index(&self, condition: &str) -> Option<usize> {
        let needle = condition.trim();
        self.conditions
            .iter()
            .position(|existing| existing.trim() == needle)
    }
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

    #[test]
    fn logical_draft_round_trips_and_finds_conditions() {
        let draft = LogicalDraft {
            operator: "AND".to_owned(),
            conditions: vec!["DOMAIN,a.com".to_owned(), "NETWORK,TCP".to_owned()],
            target: "AI".to_owned(),
        };
        let json = serde_json::to_string(&draft).unwrap();
        let decoded: LogicalDraft = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, draft);
        assert_eq!(draft.condition_index("  NETWORK,TCP "), Some(1));
        assert_eq!(draft.condition_index("DOMAIN,b.com"), None);
    }
}
