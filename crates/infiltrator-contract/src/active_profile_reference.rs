//! Cross-engine active-profile selection identity and revision fence.

use crate::core_identity::{CoreGeneration, CoreIdentity, EngineId};
use crate::profile_document_identity::ProfileId;
use crate::snapshot::CoreInactiveLifecycle;
use serde::{Deserialize, Serialize};

/// Monotonic revision for the one product-wide active-profile reference.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ActiveProfileReferenceRevision(u64);

impl ActiveProfileReferenceRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

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
    pub engine_id: EngineId,
    pub profile_id: ProfileId,
    pub revision: ActiveProfileReferenceRevision,
}

impl ActiveProfileReference {
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
    Unselected {
        revision: ActiveProfileReferenceRevision,
    },
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
    pub const fn revision(&self) -> ActiveProfileReferenceRevision {
        match self {
            Self::Unselected { revision } => *revision,
            Self::Selected(reference) => reference.revision,
        }
    }
}

/// Desired state for a revision-fenced active-profile reference write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceTarget {
    Unselected,
    Selected {
        engine_id: EngineId,
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
    Unowned,
    BoundOwner {
        identity: CoreIdentity,
        generation: CoreGeneration,
    },
}

/// Result of one revision-fenced selection write. A stale compare is a
/// normal, explicit outcome so callers can refuse dispatch or conditional
/// recovery without guessing from adapter error text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActiveProfileReferenceCasOutcome {
    Committed {
        previous: ActiveProfileReferenceState,
        current: ActiveProfileReferenceState,
    },
    Stale {
        expected: ActiveProfileReferenceState,
        observed: ActiveProfileReferenceState,
    },
}

/// Stable category of the native apply failure that triggered activation
/// recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreApplyFailureKind {
    Validation,
    Write,
    Permission,
    SourceChanged,
    ActiveProfileReferenceStale,
    InactiveCore,
    Busy,
    RolledBack,
    RollbackFailed,
    Lifecycle,
    Adapter,
}

/// Exhaustive outcome of the application-level recovery after an activation
/// apply failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActiveProfileRecoveryOutcome {
    RestoredAndReapplied {
        restored_revision: u64,
        apply_failure: CoreApplyFailureKind,
    },
    RestoredButRuntimeUnavailable {
        restored_revision: u64,
        engine_id: EngineId,
        lifecycle: CoreInactiveLifecycle,
        apply_failure: CoreApplyFailureKind,
    },
    RestoredButRuntimeMismatch {
        restored_revision: u64,
        engine_id: EngineId,
        apply_failure: CoreApplyFailureKind,
    },
    Superseded {
        expected_revision: u64,
        observed_revision: u64,
        apply_failure: CoreApplyFailureKind,
    },
    CompareFailed {
        expected_revision: u64,
        message: String,
        apply_failure: CoreApplyFailureKind,
    },
    ReapplyFailed {
        restored_revision: u64,
        message: String,
        apply_failure: CoreApplyFailureKind,
    },
}

impl ActiveProfileReferenceTarget {
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
    use crate::core_identity::EngineId;
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
}
