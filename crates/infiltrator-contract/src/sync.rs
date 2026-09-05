//! Cross-surface WebDAV synchronization results.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncReport {
    pub success_count: u64,
    pub failed_count: u64,
    pub total_actions: u64,
    pub uploaded: u64,
    pub downloaded: u64,
    pub conflicts: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProgress {
    pub phase: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncConflict {
    pub profile: String,
    pub remote_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncTransferReport {
    pub uploaded: u64,
    pub downloaded: u64,
    pub conflicts: u64,
    pub active_profile_changed: bool,
    pub conflict_files: Vec<SyncConflict>,
}
