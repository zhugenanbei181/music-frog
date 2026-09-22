//! DUAL-09-06/07: shared snapshot-history read model and prune policy.
//!
//! The history itself lives behind the [`crate::snapshot`] storage identity of
//! each host; this module is the *read model* both surfaces render and the
//! vocabulary for the one shared prune decision. A surface may never invent
//! its own retention rule: `pending_prune` is computed by the same
//! dedupe-then-LRU policy the adapter executes.

use serde::{Deserialize, Serialize};

/// Default number of snapshots retained per profile (matches the apply
/// transaction's automatic prune).
pub const SNAPSHOT_DEFAULT_KEEP: usize = 20;

/// Smallest retention the manual prune control may request.
pub const SNAPSHOT_KEEP_MIN: usize = 1;

/// Largest retention the manual prune control may request.
pub const SNAPSHOT_KEEP_MAX: usize = 100;

/// One stored snapshot as the surfaces see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEntry {
    /// Opaque storage identity (the path string) used to diff/restore/prune.
    pub id: String,
    pub file_name: String,
    /// Unix milliseconds from the snapshot name.
    pub timestamp_millis: i64,
    /// Lowercase hex SHA-256 of the stored content.
    pub sha256: String,
    /// Whether this is the newest entry of the profile.
    pub is_newest: bool,
    /// Older entry whose content hash already appeared newer in the history.
    pub is_duplicate: bool,
}

impl SnapshotEntry {
    /// Short hash pill text (first 8 hex characters).
    pub fn short_hash(&self) -> &str {
        let end = self.sha256.len().min(8);
        &self.sha256[..end]
    }

    /// UTC `MM-DD HH:MM` stamp of the snapshot name, without a date library.
    pub fn stamp_label(&self) -> String {
        let total_seconds = self.timestamp_millis.div_euclid(1000);
        let seconds_in_day = total_seconds.rem_euclid(86_400);
        let days = total_seconds.div_euclid(86_400);
        let (month, day) = civil_month_day(days);
        format!(
            "{month:02}-{day:02} {:02}:{:02}",
            seconds_in_day / 3600,
            (seconds_in_day % 3600) / 60
        )
    }
}

/// Civil (year ignored) month/day of a Unix day number, UTC.
fn civil_month_day(days: i64) -> (u32, u32) {
    // Howard Hinnant's `civil_from_days`, reduced to the month/day part.
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let doe = (shifted - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (month, day)
}

/// Who asked for the last prune.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotPruneSource {
    /// The apply transaction pruned after storing a new snapshot.
    Apply,
    /// The user ran the manual prune control.
    Manual,
}

impl SnapshotPruneSource {
    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Apply => "应用成功后自动修剪",
            Self::Manual => "手动修剪",
        }
    }
}

/// Outcome of one prune pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotPruneReport {
    pub removed: usize,
    pub keep_limit: usize,
    pub source: SnapshotPruneSource,
}

/// The current profile's snapshot history plus the shared prune view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotHistorySnapshot {
    pub profile: String,
    /// Newest first.
    pub entries: Vec<SnapshotEntry>,
    pub keep_limit: usize,
    /// Entries the shared policy would delete right now.
    pub pending_prune: usize,
    /// Of those, how many are older duplicates of a newer snapshot.
    pub duplicate_entries: usize,
    /// Last prune executed in this process, if any.
    pub last_prune: Option<SnapshotPruneReport>,
}

impl SnapshotHistorySnapshot {
    /// Empty history for a profile with no snapshots yet.
    pub fn empty(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            entries: Vec::new(),
            keep_limit: SNAPSHOT_DEFAULT_KEEP,
            pending_prune: 0,
            duplicate_entries: 0,
            last_prune: None,
        }
    }

    pub fn newest(&self) -> Option<&SnapshotEntry> {
        self.entries.first()
    }

    /// Honest one-line summary both surfaces can render.
    pub fn summary_zh(&self) -> String {
        if self.entries.is_empty() {
            return format!("{} 暂无历史快照", self.profile);
        }
        format!(
            "{} · {} 份快照（上限 {}）· 待修剪 {} 份",
            self.profile,
            self.entries.len(),
            self.keep_limit,
            self.pending_prune
        )
    }

    /// Clamp a requested retention into the supported range.
    pub fn clamp_keep(keep: usize) -> usize {
        keep.clamp(SNAPSHOT_KEEP_MIN, SNAPSHOT_KEEP_MAX)
    }
}
