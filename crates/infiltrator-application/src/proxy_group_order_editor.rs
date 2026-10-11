//! Neutral group-order draft; cancel is inert and a correlated successful command commits it.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::outbound::identity::{OutboundId, SnapshotRevision};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMove {
    Up,
    Down,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupOrderOperation {
    #[default]
    Reorder,
    ResetToInventory,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGroupOrder {
    pub token: u64,
    pub groups: Vec<OutboundId>,
    pub inventory_revision: SnapshotRevision,
    pub operation: GroupOrderOperation,
}
#[derive(Clone, Debug, Default)]
pub struct ProxyGroupOrderEditor {
    pub draft: Vec<OutboundId>,
    pub baseline: Vec<OutboundId>,
    pub default_order: Vec<OutboundId>,
    pub inventory_revision: Option<SnapshotRevision>,
    pub operation: GroupOrderOperation,
    pub pending: Option<PendingGroupOrder>,
    pub failure: Option<Failure>,
    pub observation_failure: Option<Failure>,
    next_token: u64,
}
pub fn validate_order(groups: &[OutboundId]) -> Result<(), Failure> {
    let unique: HashSet<_> = groups.iter().collect();
    if unique.len() != groups.len() {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "group order requires unique nonempty identities",
            false,
        ));
    }
    Ok(())
}
impl ProxyGroupOrderEditor {
    pub fn open(
        &mut self,
        groups: Vec<OutboundId>,
        default_order: Vec<OutboundId>,
        inventory_revision: SnapshotRevision,
    ) -> Result<(), Failure> {
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "group order is already being applied",
                false,
            ));
        }
        validate_order(&groups)?;
        validate_order(&default_order)?;
        if groups.iter().any(|group| !default_order.contains(group))
            || default_order.iter().any(|group| !groups.contains(group))
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "group order does not match the accepted outbound inventory",
                false,
            ));
        }
        self.baseline = groups.clone();
        self.draft = groups;
        self.default_order = default_order;
        self.inventory_revision = Some(inventory_revision);
        self.operation = GroupOrderOperation::Reorder;
        self.failure = None;
        self.observation_failure = None;
        Ok(())
    }
    pub fn observe(&mut self, observation: Result<(Vec<OutboundId>, SnapshotRevision), Failure>) {
        self.observation_failure = match observation {
            Err(failure) => Some(failure),
            Ok((groups, inventory_revision)) => {
                if groups != self.baseline || Some(inventory_revision) != self.inventory_revision {
                    Some(Failure::new(
                        ErrorCode::InvalidState,
                        "outbound inventory changed; reopen the order editor",
                        true,
                    ))
                } else {
                    None
                }
            }
        };
    }
    pub fn move_group(&mut self, id: &OutboundId, direction: GroupMove) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let Some(index) = self.draft.iter().position(|item| item == id) else {
            return false;
        };
        let target = match direction {
            GroupMove::Up => index.checked_sub(1),
            GroupMove::Down => (index + 1 < self.draft.len()).then_some(index + 1),
        };
        let Some(target) = target else { return false };
        self.draft.swap(index, target);
        self.operation = GroupOrderOperation::Reorder;
        self.failure = None;
        true
    }
    pub fn reset_draft(&mut self) {
        if self.pending.is_none() {
            self.draft.clone_from(&self.default_order);
            self.operation = GroupOrderOperation::ResetToInventory;
            self.failure = None;
        }
    }
    pub fn cancel(&mut self) {
        if self.pending.is_none() {
            self.draft = self.baseline.clone();
            self.operation = GroupOrderOperation::Reorder;
            self.failure = None;
            self.observation_failure = None;
        }
    }
    pub fn can_apply(&self) -> bool {
        self.pending.is_none()
            && self.observation_failure.is_none()
            && self.inventory_revision.is_some()
            && self.draft != self.baseline
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
            inventory_revision: self.inventory_revision.ok_or_else(|| {
                Failure::new(
                    ErrorCode::NotReady,
                    "outbound inventory revision is unavailable",
                    true,
                )
            })?,
            operation: self.operation,
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
                self.operation = GroupOrderOperation::Reorder;
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
        let id = |value: &str| {
            OutboundId::new(
                infiltrator_contract::core_identity::EngineId::new("mihomo").unwrap(),
                value,
            )
            .unwrap()
        };
        let original = vec![id("B"), id("A"), id("C")];
        let default_order = vec![id("A"), id("B"), id("C")];
        let inventory_revision = SnapshotRevision::new(12);
        let mut editor = ProxyGroupOrderEditor::default();
        editor
            .open(original.clone(), default_order.clone(), inventory_revision)
            .unwrap();
        assert!(!editor.move_group(&id("B"), GroupMove::Up));
        assert!(editor.move_group(&id("C"), GroupMove::Up));
        assert_eq!(editor.draft, vec![id("B"), id("C"), id("A")]);
        assert_eq!(editor.baseline, original);
        editor.cancel();
        assert_eq!(editor.draft, original);
        editor.reset_draft();
        assert_eq!(editor.draft, default_order);
        assert_eq!(editor.operation, GroupOrderOperation::ResetToInventory);
        let pending = editor.begin().unwrap();
        assert_eq!(pending.operation, GroupOrderOperation::ResetToInventory);
        assert!(!editor.move_group(&id("C"), GroupMove::Up));
        assert!(!editor.finish(pending.token + 1, Ok(())));
        assert!(editor.finish(
            pending.token,
            Err(Failure::new(ErrorCode::Storage, "write failed", true))
        ));
        assert_eq!(editor.baseline, original);
        assert_eq!(editor.draft, vec![id("A"), id("B"), id("C")]);
        let retry = editor.begin().unwrap();
        assert!(editor.finish(retry.token, Ok(())));
        assert_eq!(editor.baseline, vec![id("A"), id("B"), id("C")]);
        assert!(!editor.finish(pending.token, Ok(())));
    }
    #[test]
    fn lost_reads_and_changed_identities_block_apply_and_duplicate_payloads_are_rejected() {
        let id = |value: &str| {
            OutboundId::new(
                infiltrator_contract::core_identity::EngineId::new("mihomo").unwrap(),
                value,
            )
            .unwrap()
        };
        let groups = vec![id("A"), id("B")];
        let revision = SnapshotRevision::new(3);
        let mut editor = ProxyGroupOrderEditor::default();
        editor
            .open(groups.clone(), groups.clone(), revision)
            .unwrap();
        editor.move_group(&id("B"), GroupMove::Up);
        editor.observe(Err(Failure::new(ErrorCode::Network, "read failed", true)));
        assert!(!editor.can_apply());
        editor.observe(Ok((groups.clone(), revision)));
        assert!(editor.can_apply());
        editor.observe(Ok((vec![id("A"), id("C")], revision)));
        assert!(!editor.can_apply());
        assert!(editor.begin().is_err());
        assert!(validate_order(&[id("A"), id("A")]).is_err());
    }
}
