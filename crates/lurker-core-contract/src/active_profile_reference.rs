//! Engine-neutral active-profile selection identity and revision fence.
//!
//! The product has exactly one active profile at a time. Every write of that
//! selection is fenced by a monotonic revision so two racing writers cannot
//! both believe they won: the loser observes a `Stale` outcome instead of
//! guessing from an adapter error message.

use crate::identity::{CoreGeneration, CoreIdentity, EngineId};
use crate::profile_document_identity::ProfileId;
use serde::{Deserialize, Serialize};

/// Monotonic revision for the one product-wide active-profile reference.
///
/// The revision is the compare-and-set token of the selection. It advances on
/// every committed write — including a write that clears the selection — so a
/// stale writer can always be detected.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ActiveProfileReferenceRevision(u64);

impl ActiveProfileReferenceRevision {
    /// Wraps a raw revision value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw revision value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns the next revision, or `None` when the counter is exhausted.
    ///
    /// `None` is an explicit "no further revision exists" answer; it never
    /// wraps back to an older revision.
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

/// Exact profile identity used to choose the sole active Core provider.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveProfileReference {
    /// Engine adapter that must serve the selected profile.
    pub engine_id: EngineId,
    /// Identity of the selected profile document.
    pub profile_id: ProfileId,
    /// Revision this reference was committed under.
    pub revision: ActiveProfileReferenceRevision,
}

impl ActiveProfileReference {
    /// Assembles a reference to one profile of one engine at one revision.
    pub fn new(
        engine_id: EngineId,
        profile_id: ProfileId,
        revision: ActiveProfileReferenceRevision,
    ) -> Self {
        Self {
            engine_id,
            profile_id,
            revision,
        }
    }
}

/// Product-wide active selection. `Unselected` is explicit and retains its
/// revision so even clearing the selection participates in compare-and-set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "reference", rename_all = "snake_case")]
pub enum ActiveProfileReferenceState {
    /// No profile is selected; the revision still participates in fencing.
    Unselected {
        /// Revision the cleared selection was committed under.
        revision: ActiveProfileReferenceRevision,
    },
    /// One profile of one engine is selected.
    Selected(ActiveProfileReference),
}

impl Default for ActiveProfileReferenceState {
    fn default() -> Self {
        Self::Unselected {
            revision: ActiveProfileReferenceRevision::default(),
        }
    }
}

impl ActiveProfileReferenceState {
    /// Returns the revision this state was committed under.
    ///
    /// Both variants carry a revision, so a caller can always tell which
    /// compare-and-set generation it observed.
    pub const fn revision(&self) -> ActiveProfileReferenceRevision {
        match self {
            Self::Unselected { revision } => *revision,
            Self::Selected(reference) => reference.revision,
        }
    }
}

/// Desired state for a revision-fenced active-profile reference write.
///
/// The target carries the *desired* selection only; the expected revision is
/// supplied separately by the writer, so a target can never smuggle in the
/// revision it claims to be based on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceTarget {
    /// Clear the active selection.
    Unselected,
    /// Select one profile of one engine.
    Selected {
        /// Engine adapter that must serve the selected profile.
        engine_id: EngineId,
        /// Identity of the profile to select.
        profile_id: ProfileId,
    },
}

/// Proof required to publish an active-profile reference change. Offline
/// selection is accepted only while no Core owner is bound. A live owner may
/// change the reference only when its stable engine+instance identity and
/// exact bound generation are supplied by the runtime applier. Runtime version
/// evidence does not participate in owner identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceWriteAuthorization {
    /// No Core owner is bound, so the write may proceed offline.
    Unowned,
    /// A Core owner is bound; the write must match its fence exactly.
    BoundOwner {
        /// Stable identity of the bound Core owner.
        identity: CoreIdentity,
        /// Exact generation the owner is bound to.
        generation: CoreGeneration,
    },
}

/// Result of one revision-fenced selection write. A stale compare is a
/// normal, explicit outcome so callers can refuse dispatch or conditional
/// recovery without guessing from adapter error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceCasOutcome {
    /// The write was committed and the selection advanced.
    Committed {
        /// Selection state before the write.
        previous: ActiveProfileReferenceState,
        /// Selection state after the write.
        current: ActiveProfileReferenceState,
    },
    /// The write was refused because the observed revision had moved on.
    Stale {
        /// State the writer expected to observe.
        expected: ActiveProfileReferenceState,
        /// State the writer actually observed.
        observed: ActiveProfileReferenceState,
    },
}

impl ActiveProfileReferenceTarget {
    /// Builds the target that selects one profile of one engine.
    pub fn selected(engine_id: EngineId, profile_id: ProfileId) -> Self {
        Self::Selected {
            engine_id,
            profile_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    };
    use crate::identity::EngineId;
    use crate::profile_document_identity::ProfileId;

    #[test]
    fn reference_round_trip_preserves_engine_profile_and_revision() {
        let state = ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
            EngineId::new("sing-box").unwrap(),
            ProfileId::new("primary").unwrap(),
            ActiveProfileReferenceRevision::new(8),
        ));

        let encoded = serde_json::to_string(&state).unwrap();
        let decoded: ActiveProfileReferenceState = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, state);
        assert_eq!(decoded.revision(), ActiveProfileReferenceRevision::new(8));
    }

    #[test]
    fn unselected_state_is_typed_and_revisioned() {
        let state = ActiveProfileReferenceState::Unselected {
            revision: ActiveProfileReferenceRevision::new(3),
        };

        let encoded = serde_json::to_string(&state).unwrap();
        let decoded: ActiveProfileReferenceState = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded, state);
        assert_eq!(decoded.revision(), ActiveProfileReferenceRevision::new(3));
    }

    #[test]
    fn serialized_state_keeps_persisted_tag_and_field_names() {
        let encoded = serde_json::to_value(ActiveProfileReferenceState::Selected(
            ActiveProfileReference::new(
                EngineId::new("mihomo").unwrap(),
                ProfileId::new("main").unwrap(),
                ActiveProfileReferenceRevision::new(4),
            ),
        ))
        .unwrap();

        assert_eq!(encoded["state"], "selected");
        assert_eq!(encoded["reference"]["engine_id"], "mihomo");
        assert_eq!(encoded["reference"]["profile_id"], "main");
        assert_eq!(encoded["reference"]["revision"], 4);
    }
}
