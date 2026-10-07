//! Frontend-neutral synchronization page observations.
use crate::surface_snapshot::PageStatus;
use crate::sync::SyncStatus;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConflictSnapshot {
    pub remote_device: String,
    pub conflict_time: String,
    pub conflicting_keys: Vec<(String, String, String)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotItemSnapshot {
    pub profile: String,
    pub id: String,
    pub timestamp: String,
    pub device: String,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPageSnapshot {
    #[serde(default)]
    pub status: SyncStatus,
    pub server_url: String,
    pub username: String,
    pub last_sync: Option<String>,
    pub auto_sync: bool,
    pub conflict: Option<SyncConflictSnapshot>,
    pub snapshots: Vec<SnapshotItemSnapshot>,
    #[serde(default = "unobserved_history")]
    pub history_status: PageStatus,
}

fn unobserved_history() -> PageStatus {
    PageStatus::Loading
}
