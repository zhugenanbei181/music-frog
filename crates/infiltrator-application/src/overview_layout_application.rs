//! The application owner of the Overview page's card order.
//!
//! The owner is the single source of the card order: a surface names the card
//! and the direction, the owner applies the swap against the order it holds
//! and answers with the order it now holds. A surface never derives the next
//! order and never keeps a shadow copy of it.

use infiltrator_contract::overview_layout::{
    OverviewCardKind, OverviewCardMoveDirection, OverviewLayoutSnapshot,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

/// The owner's own revisioned layout state.
#[derive(Clone, Debug, Default)]
struct OverviewLayoutState {
    revision: u64,
    snapshot: OverviewLayoutSnapshot,
}

/// The outcome of one Overview layout mutation.
///
/// The outcome is a typed transition that carries the owner's authoritative
/// readback, never a bare boolean: the caller always learns which order the
/// owner holds and which revision it reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverviewLayoutMutation {
    /// The owner applied a new order.
    Changed {
        revision: u64,
        snapshot: OverviewLayoutSnapshot,
    },
    /// The owner already held the requested order.
    Unchanged {
        revision: u64,
        snapshot: OverviewLayoutSnapshot,
    },
}

impl OverviewLayoutMutation {
    /// The order the owner holds after the mutation.
    pub fn snapshot(&self) -> &OverviewLayoutSnapshot {
        match self {
            Self::Changed { snapshot, .. } | Self::Unchanged { snapshot, .. } => snapshot,
        }
    }

    /// The owner's revision after the mutation.
    pub fn revision(&self) -> u64 {
        match self {
            Self::Changed { revision, .. } | Self::Unchanged { revision, .. } => *revision,
        }
    }
}

/// Thread-safe application owner for the Overview page's card order.
#[derive(Clone, Debug, Default)]
pub struct OverviewLayoutApplication {
    state: Arc<Mutex<OverviewLayoutState>>,
}

impl OverviewLayoutApplication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_initial(snapshot: OverviewLayoutSnapshot) -> Self {
        Self {
            state: Arc::new(Mutex::new(OverviewLayoutState {
                revision: 0,
                snapshot,
            })),
        }
    }

    /// The order the owner currently holds.
    pub fn snapshot(&self) -> OverviewLayoutSnapshot {
        self.lock().snapshot.clone()
    }

    /// The owner's current revision.
    pub fn revision(&self) -> u64 {
        self.lock().revision
    }

    /// Move one card one step in the requested direction.
    ///
    /// The move is relative to the order the owner holds right now, so a move
    /// that was planned against an older observation still applies to the
    /// current order and can never restore the older one.
    pub fn move_card(
        &self,
        kind: OverviewCardKind,
        direction: OverviewCardMoveDirection,
    ) -> OverviewLayoutMutation {
        let mut state = self.lock();
        let changed = state.snapshot.move_card(kind, direction);
        state.commit(changed)
    }

    /// Replace the whole card order.
    ///
    /// The order must be a sequence of distinct cards; an order that repeats a
    /// card or names none is refused with a typed failure instead of silently
    /// dropping a card from the page.
    pub fn reorder(&self, order: Vec<OverviewCardKind>) -> Result<OverviewLayoutMutation, Failure> {
        let mut state = self.lock();
        validate_card_order(&order)?;
        let changed = state.snapshot.order != order;
        if changed {
            state.snapshot = OverviewLayoutSnapshot::new(order);
        }
        Ok(state.commit(changed))
    }

    /// Replace the whole card order, fenced by the revision the caller read.
    ///
    /// A caller that observed an older revision gets a typed stale failure and
    /// the owner's state does not move.
    pub fn reorder_at_revision(
        &self,
        order: Vec<OverviewCardKind>,
        expected_revision: u64,
    ) -> Result<OverviewLayoutMutation, Failure> {
        let mut state = self.lock();
        state.require_revision(expected_revision)?;
        validate_card_order(&order)?;
        let changed = state.snapshot.order != order;
        if changed {
            state.snapshot = OverviewLayoutSnapshot::new(order);
        }
        Ok(state.commit(changed))
    }

    /// Restore the canonical default order.
    pub fn reset(&self) -> OverviewLayoutMutation {
        let mut state = self.lock();
        let changed = state.snapshot.order != OverviewCardKind::DEFAULT_ORDER.to_vec();
        if changed {
            state.snapshot.reset_to_default();
        }
        state.commit(changed)
    }

    fn lock(&self) -> MutexGuard<'_, OverviewLayoutState> {
        // A poisoned lock still holds a consistent state: every mutation is one
        // atomic step, so the owner recovers the guard instead of silently
        // reporting a default order it never held.
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl OverviewLayoutState {
    fn commit(&mut self, changed: bool) -> OverviewLayoutMutation {
        if changed {
            self.revision += 1;
        }
        let revision = self.revision;
        let snapshot = self.snapshot.clone();
        if changed {
            OverviewLayoutMutation::Changed { revision, snapshot }
        } else {
            OverviewLayoutMutation::Unchanged { revision, snapshot }
        }
    }

    fn require_revision(&self, expected_revision: u64) -> Result<(), Failure> {
        if self.revision == expected_revision {
            return Ok(());
        }
        Err(Failure::new(
            ErrorCode::NotReady,
            format!(
                "the Overview card order moved to revision {} while revision {expected_revision} was observed",
                self.revision
            ),
            true,
        ))
    }
}

/// Reject an order that is not a sequence of distinct cards.
fn validate_card_order(order: &[OverviewCardKind]) -> Result<(), Failure> {
    if order.is_empty() {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            "the Overview card order names no card",
            false,
        ));
    }
    let mut seen = HashSet::new();
    for kind in order {
        if !seen.insert(*kind) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("the Overview card order repeats {kind:?}"),
                false,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "overview_layout_application_tests.rs"]
mod tests;
