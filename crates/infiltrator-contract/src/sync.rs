//! Cross-surface WebDAV synchronization results.

use serde::{Deserialize, Serialize};

/// Configuration, observation and execution are distinct facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Disconnected,
    /// Enabled settings exist; no successful server observation is implied.
    Configured,
    Connected,
    Syncing,
    Conflict,
    Error,
    #[default]
    #[serde(other)]
    Unknown,
}

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

#[cfg(test)]
mod tests {
    use super::SyncStatus;
    #[test]
    fn unfamiliar_wire_status_is_unknown_and_known_states_roundtrip() {
        assert_eq!(
            serde_json::from_str::<SyncStatus>("\"future_state\"").unwrap(),
            SyncStatus::Unknown
        );
        for status in [
            SyncStatus::Configured,
            SyncStatus::Connected,
            SyncStatus::Disconnected,
            SyncStatus::Unknown,
            SyncStatus::Syncing,
            SyncStatus::Conflict,
            SyncStatus::Error,
        ] {
            assert_eq!(
                serde_json::from_str::<SyncStatus>(&serde_json::to_string(&status).unwrap())
                    .unwrap(),
                status
            );
        }
    }
}
