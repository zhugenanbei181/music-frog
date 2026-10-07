//! DUAL-09-11: shared projection of the configuration apply transaction.
//!
//! The apply transaction lives in the host core (`infiltrator-core::apply`),
//! which owns the typed `RolledBack` / `RollbackFailed` outcome. This module
//! is the surface-neutral record of what the last transaction actually did;
//! the core publishes it, the application projects it, and both UI surfaces
//! render the same fact instead of inferring a rollback from an error string.
//!
//! The host owns observations per runtime instance; this contract contains immutable data only.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Terminal stage of one apply transaction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyTransactionStage {
    /// No transaction has run in this process yet.
    #[default]
    Idle,
    /// The new configuration is live.
    Committed,
    /// The new configuration was rejected and the previous one restored.
    RolledBack,
    /// Both the apply and the rollback failed; the core is not usable.
    RollbackFailed,
}

impl ApplyTransactionStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Committed => "committed",
            Self::RolledBack => "rolled_back",
            Self::RollbackFailed => "rollback_failed",
        }
    }

    /// Whether the transaction left the live config unchanged because of a
    /// failure (the honest 09-11 signal both surfaces key on).
    pub const fn is_failure(self) -> bool {
        matches!(self, Self::RolledBack | Self::RollbackFailed)
    }
}

/// One recorded apply transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyTransactionSnapshot {
    pub profile: String,
    pub stage: ApplyTransactionStage,
    /// Failure cause when the transaction did not commit; empty on success.
    pub detail: String,
    /// Rollback failure detail when the restore itself failed.
    pub rollback_error: Option<String>,
    /// How the config became live (`hot_reload` / `restart`), when it did.
    pub method: Option<String>,
    /// Record time as Unix milliseconds (0 when the host clock is unavailable).
    pub recorded_at_millis: i64,
}

impl ApplyTransactionSnapshot {
    pub fn committed(profile: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            stage: ApplyTransactionStage::Committed,
            detail: String::new(),
            rollback_error: None,
            method: Some(method.into()),
            recorded_at_millis: now_millis(),
        }
    }

    pub fn rolled_back(profile: impl Into<String>, cause: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            stage: ApplyTransactionStage::RolledBack,
            detail: cause.into(),
            rollback_error: None,
            method: None,
            recorded_at_millis: now_millis(),
        }
    }

    pub fn rollback_failed(
        profile: impl Into<String>,
        cause: impl Into<String>,
        rollback: impl Into<String>,
    ) -> Self {
        Self {
            profile: profile.into(),
            stage: ApplyTransactionStage::RollbackFailed,
            detail: cause.into(),
            rollback_error: Some(rollback.into()),
            method: None,
            recorded_at_millis: now_millis(),
        }
    }

    pub fn is_failure(&self) -> bool {
        self.stage.is_failure()
    }
}

/// Wall-clock milliseconds since the Unix epoch (0 on a pre-epoch clock).
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}
