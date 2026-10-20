//! Shared typed group-order draft, inventory fence, and correlated receipt reducer.
//!
//! The editor owns one explicit draft state. Every apply is fenced by the
//! inventory revision the draft was opened against, and every receipt is
//! correlated by the monotonic token of the pending order, so a late receipt
//! of a superseded order can never publish a wrong baseline.
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::identity::{OutboundId, SnapshotRevision};
use std::collections::HashSet;

/// The direction of one group move inside the draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMove {
    /// Move the group one position towards the front.
    Up,
    /// Move the group one position towards the back.
    Down,
}

/// The kind of group-order change the draft carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroupOrderOperation {
    /// The draft holds a user-defined order.
    #[default]
    Reorder,
    /// The draft asks to fall back to the engine order.
    ResetToInventory,
}

/// One in-flight group-order change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingGroupOrder {
    /// The monotonic identity of this change.
    pub token: u64,
    /// The complete new group order.
    pub groups: Vec<OutboundId>,
    /// The inventory revision the change is fenced to.
    pub inventory_revision: SnapshotRevision,
    /// The kind of change the draft carries.
    pub operation: GroupOrderOperation,
}

/// The outcome of folding one receipt into the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupOrderFinish {
    /// The receipt did not match the pending order and was ignored.
    Stale,
    /// The receipt was applied and the baseline advanced.
    Applied,
    /// The receipt reported a failure.
    Failed,
}

/// The presentation reducer of the proxy-group order editor.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupOrderEditor {
    draft: Vec<OutboundId>,
    inventory: GroupOrderInventoryState,
    operation: GroupOrderOperation,
    action: GroupOrderActionState,
    next_token: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum GroupOrderInventoryState {
    #[default]
    Unopened,
    Current(AcceptedGroupInventory),
    ReadFailed {
        accepted: AcceptedGroupInventory,
        failure: Failure,
    },
    Stale {
        accepted: AcceptedGroupInventory,
        failure: Failure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AcceptedGroupInventory {
    baseline: Vec<OutboundId>,
    default_order: Vec<OutboundId>,
    revision: SnapshotRevision,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum GroupOrderActionState {
    #[default]
    Ready,
    Pending(PendingGroupOrder),
    Rejected(Failure),
    Failed(Failure),
}

/// Validates that a group order contains unique identities.
///
/// Fails with an `InvalidInput` failure when the order repeats a group.
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
    /// Opens the editor against one accepted outbound inventory.
    ///
    /// Fails with an `InvalidState` failure when a change is already pending,
    /// with an `InvalidInput` failure when the order is not unique, and with an
    /// `InvalidInput` failure when the order does not match the inventory.
    pub fn open(
        &mut self,
        groups: Vec<OutboundId>,
        default_order: Vec<OutboundId>,
        inventory_revision: SnapshotRevision,
    ) -> Result<(), Failure> {
        if self.is_pending() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "group order is already being applied",
                false,
            ));
        }
        validate_order(&groups)?;
        validate_order(&default_order)?;
        if groups.len() != default_order.len()
            || groups.iter().any(|group| !default_order.contains(group))
            || default_order.iter().any(|group| !groups.contains(group))
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "group order does not match the accepted outbound inventory",
                false,
            ));
        }
        self.inventory = GroupOrderInventoryState::Current(AcceptedGroupInventory {
            baseline: groups.clone(),
            default_order,
            revision: inventory_revision,
        });
        self.draft = groups;
        self.operation = GroupOrderOperation::Reorder;
        self.action = GroupOrderActionState::Ready;
        Ok(())
    }

    /// Folds one inventory observation into the editor.
    ///
    /// An observation that differs from the accepted inventory marks the
    /// editor stale and requires the caller to reopen it.
    pub fn observe(&mut self, observation: Result<(Vec<OutboundId>, SnapshotRevision), Failure>) {
        let Some(accepted) = self.inventory.accepted().cloned() else {
            return;
        };
        self.inventory = match observation {
            Err(failure) => GroupOrderInventoryState::ReadFailed { accepted, failure },
            Ok((groups, revision))
                if groups == accepted.baseline && revision == accepted.revision =>
            {
                GroupOrderInventoryState::Current(accepted)
            }
            Ok(_) => GroupOrderInventoryState::Stale {
                accepted,
                failure: Failure::new(
                    ErrorCode::InvalidState,
                    "outbound inventory changed; reopen the order editor",
                    true,
                ),
            },
        };
    }

    /// Moves one group inside the draft.
    ///
    /// Returns `false` when the editor is pending, when the group is unknown
    /// or when the move would leave the draft.
    pub fn move_group(&mut self, id: &OutboundId, direction: GroupMove) -> bool {
        if self.is_pending() {
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
        self.action = GroupOrderActionState::Ready;
        true
    }

    /// Resets the draft to the engine order of the accepted inventory.
    pub fn reset_draft(&mut self) {
        if self.is_pending() {
            return;
        }
        let Some(accepted) = self.inventory.accepted() else {
            return;
        };
        self.draft.clone_from(&accepted.default_order);
        self.operation = GroupOrderOperation::ResetToInventory;
        self.action = GroupOrderActionState::Ready;
    }

    /// Cancels the draft and restores the accepted baseline.
    pub fn cancel(&mut self) {
        if self.is_pending() {
            return;
        }
        if let Some(accepted) = self.inventory.accepted() {
            self.draft.clone_from(&accepted.baseline);
        } else {
            self.draft.clear();
        }
        self.operation = GroupOrderOperation::Reorder;
        self.action = GroupOrderActionState::Ready;
    }

    /// Whether the draft can be applied right now.
    pub fn can_apply(&self) -> bool {
        !self.is_pending()
            && matches!(&self.inventory, GroupOrderInventoryState::Current(_))
            && self
                .inventory
                .accepted()
                .is_some_and(|accepted| self.draft != accepted.baseline)
    }

    /// Starts one group-order change.
    ///
    /// Fails with an `InvalidState` failure when a change is already pending or
    /// when the draft has no unapplied changes, and with a `NotReady` failure
    /// when the inventory is not open.
    pub fn begin(&mut self) -> Result<PendingGroupOrder, Failure> {
        if self.is_pending() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "group order is already being applied",
                false,
            ));
        }
        if let Some(failure) = self.observation_failure().cloned() {
            return self.reject_with(failure);
        }
        let Some(accepted) = self.inventory.accepted().cloned() else {
            return self.reject_with(Failure::new(
                ErrorCode::NotReady,
                "outbound inventory is not open",
                true,
            ));
        };
        if !self.can_apply() {
            return self.reject_with(Failure::new(
                ErrorCode::InvalidState,
                "group order has no unapplied changes",
                false,
            ));
        }
        if let Err(failure) = validate_order(&self.draft) {
            return self.reject_with(failure);
        }
        self.next_token = match self.next_token.checked_add(1) {
            Some(token) => token,
            None => {
                return self.reject_with(Failure::new(
                    ErrorCode::InvalidState,
                    "group order token exhausted",
                    false,
                ));
            }
        };
        let pending = PendingGroupOrder {
            token: self.next_token,
            groups: self.draft.clone(),
            inventory_revision: accepted.revision,
            operation: self.operation,
        };
        self.action = GroupOrderActionState::Pending(pending.clone());
        Ok(pending)
    }

    /// Folds the receipt of one group-order change into the editor.
    ///
    /// A receipt whose token does not match the pending order is stale and is
    /// ignored.
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> GroupOrderFinish {
        let pending = match &self.action {
            GroupOrderActionState::Pending(pending) if pending.token == token => pending.clone(),
            GroupOrderActionState::Ready
            | GroupOrderActionState::Pending(_)
            | GroupOrderActionState::Rejected(_)
            | GroupOrderActionState::Failed(_) => return GroupOrderFinish::Stale,
        };
        match result {
            Ok(()) => {
                if let Some(accepted) = self.inventory.accepted_mut() {
                    accepted.baseline.clone_from(&pending.groups);
                }
                self.draft.clone_from(&pending.groups);
                self.operation = GroupOrderOperation::Reorder;
                self.action = GroupOrderActionState::Ready;
                GroupOrderFinish::Applied
            }
            Err(failure) => {
                self.action = GroupOrderActionState::Failed(failure);
                GroupOrderFinish::Failed
            }
        }
    }

    /// Records a rejection that happened before the change was started.
    pub fn reject(&mut self, failure: Failure) {
        if !self.is_pending() {
            self.action = GroupOrderActionState::Rejected(failure);
        }
    }

    /// Returns the current draft order.
    pub fn draft(&self) -> &[OutboundId] {
        &self.draft
    }

    /// Returns the accepted baseline order, when the inventory is open.
    pub fn baseline(&self) -> Option<&[OutboundId]> {
        self.inventory
            .accepted()
            .map(|accepted| accepted.baseline.as_slice())
    }

    /// Returns the engine order, when the inventory is open.
    pub fn default_order(&self) -> Option<&[OutboundId]> {
        self.inventory
            .accepted()
            .map(|accepted| accepted.default_order.as_slice())
    }

    /// Returns the inventory revision the draft is fenced to.
    pub fn inventory_revision(&self) -> Option<SnapshotRevision> {
        self.inventory.accepted().map(|accepted| accepted.revision)
    }

    /// Returns the kind of change the draft carries.
    pub fn operation(&self) -> GroupOrderOperation {
        self.operation
    }

    /// Returns the in-flight change, when one is pending.
    pub fn pending(&self) -> Option<&PendingGroupOrder> {
        match &self.action {
            GroupOrderActionState::Pending(pending) => Some(pending),
            GroupOrderActionState::Ready
            | GroupOrderActionState::Rejected(_)
            | GroupOrderActionState::Failed(_) => None,
        }
    }

    /// Whether one group-order change is in flight.
    pub fn is_pending(&self) -> bool {
        matches!(&self.action, GroupOrderActionState::Pending(_))
    }

    /// Returns the recorded failure, when the editor carries one.
    pub fn failure(&self) -> Option<&Failure> {
        match &self.action {
            GroupOrderActionState::Rejected(failure) | GroupOrderActionState::Failed(failure) => {
                Some(failure)
            }
            GroupOrderActionState::Ready | GroupOrderActionState::Pending(_) => None,
        }
    }

    /// Returns the inventory observation failure, when there is one.
    pub fn observation_failure(&self) -> Option<&Failure> {
        match &self.inventory {
            GroupOrderInventoryState::ReadFailed { failure, .. }
            | GroupOrderInventoryState::Stale { failure, .. } => Some(failure),
            GroupOrderInventoryState::Unopened | GroupOrderInventoryState::Current(_) => None,
        }
    }

    fn reject_with<T>(&mut self, failure: Failure) -> Result<T, Failure> {
        self.action = GroupOrderActionState::Rejected(failure.clone());
        Err(failure)
    }
}

impl GroupOrderInventoryState {
    fn accepted(&self) -> Option<&AcceptedGroupInventory> {
        match self {
            Self::Unopened => None,
            Self::Current(accepted)
            | Self::ReadFailed { accepted, .. }
            | Self::Stale { accepted, .. } => Some(accepted),
        }
    }

    fn accepted_mut(&mut self) -> Option<&mut AcceptedGroupInventory> {
        match self {
            Self::Unopened => None,
            Self::Current(accepted)
            | Self::ReadFailed { accepted, .. }
            | Self::Stale { accepted, .. } => Some(accepted),
        }
    }
}

#[cfg(test)]
#[path = "proxy_group_order_editor_tests.rs"]
mod tests;
