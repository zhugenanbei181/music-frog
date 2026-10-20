//! Engine-neutral protocol for replacing the single active Core owner.
//!
//! A switch is a strict, revision-fenced sequence: validate the target, close
//! the exact previous owner, publish the new selection, reserve a candidate
//! owner, apply it, and only then bind it. The stages below are the only legal
//! intermediate states; every transition carries the previous stage as its
//! fencing token, so a stale revision, owner generation or lease is rejected
//! instead of silently resuming.
//!
//! # Recovery modelling
//!
//! [`EngineSwitchRecovery`] is a pure tagged enum and deliberately carries no
//! `Box<dyn ...Lease>` payload. Leases are port traits, so embedding them here
//! would make `lurker-core-contract` depend on `lurker-core-ports` (or force a
//! trait object into the domain vocabulary). Instead the journal implementation
//! observes the durable stage with this enum, and the runtime obtains the
//! matching resumable lease from the journal separately
//! (`recover_old_owner_after_terminal` / `recover_candidate_after_terminal`).
//! That keeps the contract free of port traits while still making every crash
//! boundary explicit.

use crate::identity::{CoreGeneration, CoreIdentity, EngineId};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// Monotonic revision of the owner-switch journal.
///
/// Every committed transition produces a new revision. Leases and compare
/// operations are fenced by the revision they were issued against.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct EngineSwitchRevision(u64);

impl EngineSwitchRevision {
    /// Wraps a raw revision value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw revision value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// The next revision, or a typed failure when the counter is exhausted.
    pub fn next(self) -> Result<Self, EngineSwitchRevisionError> {
        match self.0.checked_add(1) {
            Some(value) => Ok(Self(value)),
            None => Err(EngineSwitchRevisionError::Exhausted),
        }
    }
}

/// Failure of a checked revision increment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineSwitchRevisionError {
    /// The revision counter reached its maximum and cannot advance.
    Exhausted,
}

impl Display for EngineSwitchRevisionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => formatter.write_str("engine switch revision exhausted"),
        }
    }
}

impl Error for EngineSwitchRevisionError {}

/// Desired owner selection. `Unselected` is explicit: a live owner can never
/// be replaced from it, and the switch protocol requires a selected target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum EngineSwitchTarget<S> {
    /// No owner is selected; a switch to an unselected target is not legal.
    Unselected,
    /// One engine and one selection become the new owner.
    Selected {
        /// The engine that must serve the selection.
        engine_id: EngineId,
        /// The engine-specific selection to publish.
        selection: S,
    },
}

/// Durable progress of one owner replacement.
///
/// `previous` is the selection the old owner was bound to. `owner_generation`
/// is the exact binding generation, never a later lifecycle generation;
/// `terminal_generation` is the exact generation that was observed Stopped and
/// may be newer, but never older.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "snake_case")]
pub enum EngineSwitchStage<S> {
    /// The old owner is still live. This is the only stage that can be
    /// cancelled back to the previous binding.
    ClosingOld {
        /// The selection the old owner was bound to.
        previous: S,
        /// Stable identity of the old owner.
        owner_identity: CoreIdentity,
        /// Exact generation the old owner is bound to.
        owner_generation: CoreGeneration,
    },
    /// The old owner has an exact terminal generation. No transition back to
    /// the previous owner is legal from here.
    OldStopped {
        /// The selection the old owner was bound to.
        previous: S,
        /// Stable identity of the old owner.
        owner_identity: CoreIdentity,
        /// Exact generation the old owner was bound to.
        owner_generation: CoreGeneration,
        /// Exact generation observed as terminal; never older than the bound one.
        terminal_generation: CoreGeneration,
    },
    /// The new selection is committed, but the candidate owner has not been
    /// reserved yet. The target engine identity is retained by the journal.
    SelectionChanged {
        /// The selection the old owner was bound to.
        previous: S,
        /// The newly committed selection.
        selection: S,
        /// Stable identity of the old owner.
        owner_identity: CoreIdentity,
        /// Exact generation observed as terminal.
        terminal_generation: CoreGeneration,
    },
    /// Candidate owner admission is reserved; the candidate has not been
    /// started yet.
    BindingNew {
        /// The committed selection the candidate will serve.
        selection: S,
        /// The engine the candidate must be.
        engine_id: EngineId,
    },
}

/// Owner state of the journal snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum EngineOwnerState<S> {
    /// No owner is bound; only an offline selection may exist.
    Unowned,
    /// One owner is bound to the current selection.
    Bound {
        /// Stable identity of the bound owner.
        identity: CoreIdentity,
        /// Exact generation the owner is bound to.
        generation: CoreGeneration,
    },
    /// A replacement is in progress.
    Switching {
        /// The durable stage of the running switch.
        stage: EngineSwitchStage<S>,
    },
}

/// One atomic readback of selection plus owner state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineSwitchSnapshot<S> {
    /// The revision the snapshot was read at.
    pub revision: EngineSwitchRevision,
    /// The currently committed selection.
    pub selection: S,
    /// The owner state that belongs to the selection.
    pub owner: EngineOwnerState<S>,
}

impl<S> EngineSwitchSnapshot<S> {
    /// The exact bound owner fence, or `None` while unowned or switching.
    pub fn bound_owner(&self) -> Option<(&CoreIdentity, CoreGeneration)> {
        match &self.owner {
            EngineOwnerState::Bound {
                identity,
                generation,
            } => Some((identity, *generation)),
            EngineOwnerState::Unowned | EngineOwnerState::Switching { .. } => None,
        }
    }
}

/// Restart-visible stage of an unfinished switch.
///
/// Recovery is observation only: it never restores a closed owner and never
/// infers process state. The runtime turns the tag into an action by asking
/// the journal for the corresponding lease after the relevant terminal
/// evidence exists.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "recovery", rename_all = "snake_case")]
pub enum EngineSwitchRecovery<S> {
    /// No switch was pending when the journal was read.
    NoPendingSwitch,
    /// The switch stopped while the old owner was still live.
    ResumeClosingOld {
        /// The selection the old owner was bound to.
        previous: S,
        /// Stable identity of the old owner.
        owner_identity: CoreIdentity,
        /// Exact generation the old owner is bound to.
        owner_generation: CoreGeneration,
    },
    /// The switch stopped after the old owner reached a terminal generation.
    ResumeOldStopped {
        /// The selection the old owner was bound to.
        previous: S,
        /// Stable identity of the old owner.
        owner_identity: CoreIdentity,
        /// Exact generation the old owner was bound to.
        owner_generation: CoreGeneration,
        /// Exact generation observed as terminal.
        terminal_generation: CoreGeneration,
    },
    /// The switch stopped after the new selection was committed.
    ResumeSelectionChanged {
        /// The selection the old owner was bound to.
        previous: S,
        /// The newly committed selection.
        selection: S,
        /// Stable identity of the old owner.
        owner_identity: CoreIdentity,
        /// Exact generation observed as terminal.
        terminal_generation: CoreGeneration,
    },
    /// The switch stopped after the candidate admission was reserved.
    ResumeBindingNew {
        /// The committed selection the candidate will serve.
        selection: S,
        /// The engine the candidate must be.
        engine_id: EngineId,
    },
}

impl<S: Clone> From<&EngineSwitchStage<S>> for EngineSwitchRecovery<S> {
    fn from(stage: &EngineSwitchStage<S>) -> Self {
        match stage {
            EngineSwitchStage::ClosingOld {
                previous,
                owner_identity,
                owner_generation,
            } => Self::ResumeClosingOld {
                previous: previous.clone(),
                owner_identity: owner_identity.clone(),
                owner_generation: *owner_generation,
            },
            EngineSwitchStage::OldStopped {
                previous,
                owner_identity,
                owner_generation,
                terminal_generation,
            } => Self::ResumeOldStopped {
                previous: previous.clone(),
                owner_identity: owner_identity.clone(),
                owner_generation: *owner_generation,
                terminal_generation: *terminal_generation,
            },
            EngineSwitchStage::SelectionChanged {
                previous,
                selection,
                owner_identity,
                terminal_generation,
            } => Self::ResumeSelectionChanged {
                previous: previous.clone(),
                selection: selection.clone(),
                owner_identity: owner_identity.clone(),
                terminal_generation: *terminal_generation,
            },
            EngineSwitchStage::BindingNew {
                selection,
                engine_id,
            } => Self::ResumeBindingNew {
                selection: selection.clone(),
                engine_id: engine_id.clone(),
            },
        }
    }
}

/// Where a switch attempt failed. Only the first phase allows a clean
/// rollback; every later phase requires recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineSwitchFailurePhase {
    /// Nothing was reserved: the previous owner is still bound and unchanged.
    BeforeReservation,
    /// The durable reservation may have committed; its state is unknown.
    ReservationPending,
    /// The old owner may or may not have stopped. Terminal evidence is
    /// required before any further transition.
    OwnerTerminationUnconfirmed,
    /// The old owner has an exact terminal. The previous owner can never be
    /// restored.
    OldOwnerClosed,
}

/// Exhaustive outcome of a failed switch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum EngineSwitchDisposition<S> {
    /// The journal is back at the previous binding; the caller keeps using it.
    RolledBackToPrevious {
        /// The selection that is bound again.
        selection: S,
    },
    /// The journal retained an unfinished stage; the caller must resume or
    /// surface a recovery-required state.
    RecoveryRequired,
}

#[cfg(test)]
mod tests {
    use super::{
        EngineOwnerState, EngineSwitchDisposition, EngineSwitchFailurePhase, EngineSwitchRecovery,
        EngineSwitchRevision, EngineSwitchRevisionError, EngineSwitchSnapshot, EngineSwitchStage,
        EngineSwitchTarget,
    };
    use crate::identity::{CoreGeneration, CoreIdentity, CoreInstanceId, EngineId};

    fn engine(value: &str) -> EngineId {
        EngineId::new(value).expect("test engine ID is valid")
    }

    fn identity(engine_id: EngineId, instance: &str) -> CoreIdentity {
        CoreIdentity::new(
            engine_id,
            CoreInstanceId::new(instance).expect("test instance ID is valid"),
            None,
        )
    }

    #[test]
    fn revision_increment_is_checked() {
        assert_eq!(
            EngineSwitchRevision::new(4).next(),
            Ok(EngineSwitchRevision::new(5))
        );
        assert_eq!(
            EngineSwitchRevision::new(u64::MAX).next(),
            Err(EngineSwitchRevisionError::Exhausted)
        );
    }

    #[test]
    fn snapshot_round_trip_keeps_every_stage_field() {
        let previous = engine("mihomo");
        let selection = engine("sing-box");
        let stage = EngineSwitchStage::SelectionChanged {
            previous: previous.clone(),
            selection: selection.clone(),
            owner_identity: identity(previous, "old-owner"),
            terminal_generation: CoreGeneration::new(7),
        };
        let snapshot = EngineSwitchSnapshot {
            revision: EngineSwitchRevision::new(12),
            selection: selection.clone(),
            owner: EngineOwnerState::Switching {
                stage: stage.clone(),
            },
        };

        let encoded = serde_json::to_string(&snapshot).expect("serialize switch snapshot");
        let decoded: EngineSwitchSnapshot<EngineId> =
            serde_json::from_str(&encoded).expect("deserialize switch snapshot");
        assert_eq!(decoded, snapshot);
        assert_eq!(decoded.bound_owner(), None);

        let value = serde_json::to_value(&stage).expect("serialize stage");
        assert_eq!(value["stage"], "selection_changed");
        assert_eq!(value["terminal_generation"], 7);
        assert_eq!(value["selection"], "sing-box");
    }

    #[test]
    fn recovery_is_a_tagged_enum_without_lease_payloads() {
        let previous = engine("mihomo");
        let stage = EngineSwitchStage::ClosingOld {
            previous: previous.clone(),
            owner_identity: identity(previous.clone(), "old-owner"),
            owner_generation: CoreGeneration::new(3),
        };
        let recovery = EngineSwitchRecovery::from(&stage);
        assert!(matches!(
            recovery,
            EngineSwitchRecovery::ResumeClosingOld {
                owner_generation,
                ..
            } if owner_generation == CoreGeneration::new(3)
        ));
        let value = serde_json::to_value(&recovery).expect("serialize recovery");
        assert_eq!(value["recovery"], "resume_closing_old");
        assert!(value.get("lease").is_none());
    }

    #[test]
    fn target_and_disposition_wire_tags_stay_explicit() {
        let target = EngineSwitchTarget::Selected {
            engine_id: engine("sing-box"),
            selection: engine("sing-box"),
        };
        let value = serde_json::to_value(&target).expect("serialize target");
        assert_eq!(value["target"], "selected");
        assert_eq!(value["engine_id"], "sing-box");

        let disposition = EngineSwitchDisposition::RolledBackToPrevious {
            selection: engine("mihomo"),
        };
        let value = serde_json::to_value(&disposition).expect("serialize disposition");
        assert_eq!(value["disposition"], "rolled_back_to_previous");
        assert_eq!(value["selection"], "mihomo");

        let phase = EngineSwitchFailurePhase::OldOwnerClosed;
        assert_eq!(
            serde_json::to_value(phase).expect("serialize phase"),
            serde_json::json!("old_owner_closed")
        );
    }
}
