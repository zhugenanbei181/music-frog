//! DUAL-09-11: shared projection of the configuration apply transaction.
//!
//! The apply transaction lives in the host core (`infiltrator-core::apply`),
//! which owns the typed `RolledBack` / `RollbackFailed` outcome. This module
//! is the surface-neutral record of what the last transaction actually did;
//! the core publishes it, the application projects it, and both UI surfaces
//! render the same fact instead of inferring a rollback from an error string.
//!
//! The publisher is deliberately process-wide: one host process runs one core,
//! and the surfaces must not disagree about whether the live config is the
//! config the user asked for.

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

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

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Idle => "尚无应用记录",
            Self::Committed => "已提交生效",
            Self::RolledBack => "已自动回滚到上一版本",
            Self::RollbackFailed => "回滚失败：核心不可用，请立即处理",
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

    /// Honest one-line summary both surfaces can render.
    pub fn summary_zh(&self) -> String {
        match self.stage {
            ApplyTransactionStage::Idle => "尚无应用记录".to_owned(),
            ApplyTransactionStage::Committed => match &self.method {
                Some(method) => format!("{} · 已提交生效（{method}）", self.profile),
                None => format!("{} · 已提交生效", self.profile),
            },
            ApplyTransactionStage::RolledBack => {
                format!("{} · 已自动回滚到上一版本：{}", self.profile, self.detail)
            }
            ApplyTransactionStage::RollbackFailed => format!(
                "{} · 回滚失败：{}（{}）",
                self.profile,
                self.detail,
                self.rollback_error.as_deref().unwrap_or("原因未记录")
            ),
        }
    }
}

/// Wall-clock milliseconds since the Unix epoch (0 on a pre-epoch clock).
pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or(0)
}

fn transaction_cache() -> &'static Mutex<Option<ApplyTransactionSnapshot>> {
    static RECORD: OnceLock<Mutex<Option<ApplyTransactionSnapshot>>> = OnceLock::new();
    RECORD.get_or_init(|| Mutex::new(None))
}

/// Publish the outcome of the transaction that just finished.
pub fn record_apply_transaction(snapshot: ApplyTransactionSnapshot) {
    if let Ok(mut record) = transaction_cache().lock() {
        *record = Some(snapshot);
    }
}

/// The last recorded apply transaction, if any.
pub fn last_apply_transaction() -> Option<ApplyTransactionSnapshot> {
    transaction_cache()
        .lock()
        .ok()
        .and_then(|record| record.clone())
}

/// Drop the record (used by tests and by hosts that reset their state).
pub fn clear_apply_transaction() {
    if let Ok(mut record) = transaction_cache().lock() {
        *record = None;
    }
}
