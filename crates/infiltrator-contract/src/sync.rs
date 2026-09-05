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
