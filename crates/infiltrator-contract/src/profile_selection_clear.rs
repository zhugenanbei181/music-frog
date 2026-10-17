//! Typed request and readback for clearing one engine's active profile choice.

use lurker_core_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceRevision, ActiveProfileReferenceState,
    ActiveProfileReferenceWriteAuthorization,
};

/// Compare-and-clear request bound to the exact selection observed by the
/// caller and the owner's authorization for the write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClearActiveProfileSelectionRequest {
    pub expected: ActiveProfileReferenceState,
    pub authorization: ActiveProfileReferenceWriteAuthorization,
}

impl ClearActiveProfileSelectionRequest {
    pub fn new(
        expected: ActiveProfileReferenceState,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Self {
        Self {
            expected,
            authorization,
        }
    }
}

/// Exact readback proving there is no selected profile at this revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnselectedProfileSelectionReadback {
    pub revision: ActiveProfileReferenceRevision,
}

/// Result of a typed active-profile clear operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClearActiveProfileSelectionOutcome {
    Cleared {
        previous: ActiveProfileReference,
        current: UnselectedProfileSelectionReadback,
    },
    AlreadyUnselected {
        current: UnselectedProfileSelectionReadback,
    },
    Stale {
        expected: ActiveProfileReferenceState,
        observed: ActiveProfileReferenceState,
    },
}
