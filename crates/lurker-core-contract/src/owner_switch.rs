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
    /// The engine rejected the configuration as invalid.
    Validation,
    /// The configuration could not be written to disk.
    Write,
    /// The process lacked the permission the apply needed.
    Permission,
    /// The source document changed while the apply was running.
    SourceChanged,
    /// The active-profile reference moved on before the apply committed.
    ActiveProfileReferenceStale,
    /// The core was not running when the apply started.
    InactiveCore,
    /// Another apply was already in flight.
    Busy,
    /// The apply failed and was rolled back successfully.
    RolledBack,
    /// The apply failed and the rollback failed too.
    RollbackFailed,
    /// The core lifecycle did not reach the expected state.
    Lifecycle,
    /// The engine adapter reported an opaque failure.
    Adapter,
}

/// Exhaustive outcome of the application-level recovery after an activation
/// apply failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ActiveProfileRecoveryOutcome {
    /// The previous selection was restored and applied successfully.
    RestoredAndReapplied {
        /// Revision of the restored selection.
        restored_revision: u64,
        /// The apply failure that triggered the recovery.
        apply_failure: CoreApplyFailureKind,
    },
    /// The previous selection was restored, but the runtime cannot serve it.
    RestoredButRuntimeUnavailable {
        /// Revision of the restored selection.
        restored_revision: u64,
        /// The engine the restored selection belongs to.
        engine_id: EngineId,
        /// The inactive lifecycle fact observed after the restore.
        lifecycle: CoreInactiveLifecycle,
        /// The apply failure that triggered the recovery.
        apply_failure: CoreApplyFailureKind,
    },
    /// The previous selection was restored, but the runtime no longer matches
    /// it.
    RestoredButRuntimeMismatch {
        /// Revision of the restored selection.
        restored_revision: u64,
        /// The engine the restored selection belongs to.
        engine_id: EngineId,
        /// The apply failure that triggered the recovery.
        apply_failure: CoreApplyFailureKind,
    },
    /// Another writer advanced the active-profile reference first.
    Superseded {
        /// Revision the writer expected to observe.
        expected_revision: u64,
        /// Revision the writer actually observed.
        observed_revision: u64,
        /// The apply failure that triggered the recovery.
        apply_failure: CoreApplyFailureKind,
    },
    /// The compare-and-set of the active-profile reference failed.
    CompareFailed {
        /// Revision the writer expected to observe.
        expected_revision: u64,
        /// Human-readable detail of the failure.
        message: String,
        /// The apply failure that triggered the recovery.
        apply_failure: CoreApplyFailureKind,
    },
    /// The restore succeeded, but re-applying the restored selection failed.
    ReapplyFailed {
        /// Revision of the restored selection.
        restored_revision: u64,
        /// Human-readable detail of the failure.
        message: String,
        /// The apply failure that triggered the recovery.
        apply_failure: CoreApplyFailureKind,
    },
}
