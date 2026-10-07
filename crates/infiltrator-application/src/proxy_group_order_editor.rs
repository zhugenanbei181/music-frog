//! Neutral group-order draft; cancel is inert and a correlated successful command commits it.
use infiltrator_contract::error::{ErrorCode, Failure};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMove {
    Up,
    Down,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGroupOrder {
    pub token: u64,
    pub groups: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct ProxyGroupOrderEditor {
    pub draft: Vec<String>,
    pub baseline: Vec<String>,
    pub pending: Option<PendingGroupOrder>,
    pub failure: Option<Failure>,
    pub observation_failure: Option<Failure>,
    next_token: u64,
}
pub fn validate_order(groups: &[String]) -> Result<(), Failure> {
    let unique: BTreeSet<_> = groups.iter().collect();
    if unique.len() != groups.len() || groups.iter().any(|name| name.trim().is_empty()) {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "group order requires unique nonempty identities",
            false,
        ));
    }
    Ok(())
}
impl ProxyGroupOrderEditor {
    pub fn open(&mut self, groups: Vec<String>) -> Result<(), Failure> {
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "group order is already being applied",
                false,
            ));
        }
        validate_order(&groups)?;
        self.baseline = groups.clone();
        self.draft = groups;
        self.failure = None;
        self.observation_failure = None;
        Ok(())
    }
    pub fn observe(&mut self, groups: Result<Vec<String>, Failure>) {
        self.observation_failure = match groups {
            Err(failure) => Some(failure),
            Ok(groups) => {
                let expected: BTreeSet<_> = self.baseline.iter().collect();
                let actual: BTreeSet<_> = groups.iter().collect();
                if actual != expected {
                    Some(Failure::new(
                        ErrorCode::InvalidState,
                        "group identities changed; reopen the order editor",
                        true,
                    ))
                } else {
                    None
                }
            }
        };
    }
    pub fn move_group(&mut self, name: &str, direction: GroupMove) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let Some(index) = self.draft.iter().position(|item| item == name) else {
            return false;
        };
        let target = match direction {
            GroupMove::Up => index.checked_sub(1),
            GroupMove::Down => (index + 1 < self.draft.len()).then_some(index + 1),
        };
        let Some(target) = target else { return false };
        self.draft.swap(index, target);
        self.failure = None;
        true
    }
    pub fn reset_draft(&mut self) {
        if self.pending.is_none() {
            self.draft.sort();
            self.failure = None;
        }
    }
    pub fn cancel(&mut self) {
        if self.pending.is_none() {
            self.draft = self.baseline.clone();
            self.failure = None;
            self.observation_failure = None;
        }
    }
    pub fn can_apply(&self) -> bool {
        self.pending.is_none() && self.observation_failure.is_none() && self.draft != self.baseline
    }
    pub fn begin(&mut self) -> Result<PendingGroupOrder, Failure> {
        if let Some(failure) = &self.observation_failure {
            return Err(failure.clone());
        }
        if !self.can_apply() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "group order has no unapplied changes or is already pending",
                false,
            ));
        }
        validate_order(&self.draft)?;
        self.next_token = self.next_token.wrapping_add(1);
        let pending = PendingGroupOrder {
            token: self.next_token,
            groups: self.draft.clone(),
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        Ok(pending)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.token == token)
        else {
            return false;
        };
        let order = pending.groups.clone();
        self.pending = None;
        match result {
            Ok(()) => {
                self.baseline = order;
                self.failure = None;
            }
            Err(failure) => self.failure = Some(failure),
        }
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_moves_reset_cancel_and_failures_never_publish_an_uncommitted_order() {
        let original = vec!["B".into(), "A".into(), "C".into()];
        let mut editor = ProxyGroupOrderEditor::default();
        editor.open(original.clone()).unwrap();
        assert!(!editor.move_group("B", GroupMove::Up));
        assert!(editor.move_group("C", GroupMove::Up));
        assert_eq!(editor.draft, vec!["B", "C", "A"]);
        assert_eq!(editor.baseline, original);
        editor.cancel();
        assert_eq!(editor.draft, original);
        editor.reset_draft();
        assert_eq!(editor.draft, vec!["A", "B", "C"]);
        let pending = editor.begin().unwrap();
        assert!(!editor.move_group("C", GroupMove::Up));
        assert!(!editor.finish(pending.token + 1, Ok(())));
        assert!(editor.finish(
            pending.token,
            Err(Failure::new(ErrorCode::Storage, "write failed", true))
        ));
        assert_eq!(editor.baseline, original);
        assert_eq!(editor.draft, vec!["A", "B", "C"]);
        let retry = editor.begin().unwrap();
        assert!(editor.finish(retry.token, Ok(())));
        assert_eq!(editor.baseline, vec!["A", "B", "C"]);
        assert!(!editor.finish(pending.token, Ok(())));
    }
    #[test]
    fn lost_reads_and_changed_identities_block_apply_and_duplicate_payloads_are_rejected() {
        let mut editor = ProxyGroupOrderEditor::default();
        editor.open(vec!["A".into(), "B".into()]).unwrap();
        editor.move_group("B", GroupMove::Up);
        editor.observe(Err(Failure::new(ErrorCode::Network, "read failed", true)));
        assert!(!editor.can_apply());
        editor.observe(Ok(vec!["A".into(), "B".into()]));
        assert!(editor.can_apply());
        editor.observe(Ok(vec!["A".into(), "C".into()]));
        assert!(!editor.can_apply());
        assert!(editor.begin().is_err());
        assert!(validate_order(&["A".into(), "A".into()]).is_err());
        assert!(validate_order(&["".into()]).is_err());
    }
}
