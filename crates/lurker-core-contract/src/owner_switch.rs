//! Engine-neutral active-profile switch recovery vocabulary.
//!
//! These types describe what happened when an activation apply failed and the
//! runtime recovered; they carry no product-private field.

use crate::identity::EngineId;
use crate::lifecycle::CoreInactiveLifecycle;
use serde::{Deserialize, Serialize};

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
