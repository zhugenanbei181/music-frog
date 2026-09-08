//! Shared read model for core reload/reconnect graceful degradation mask.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReconnectMaskStatus {
    #[default]
    Normal,
    Reloading,
    Reconnecting,
    Degraded,
}

/// Snapshot indicating whether an overlay degradation mask should protect the view
/// during core configuration reload, process crash recovery, or network disconnection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ReconnectMaskSnapshot {
    pub status: ReconnectMaskStatus,
    pub reloading: bool,
    pub message: Option<String>,
    pub preserves_last_frame: bool,
    pub attempt: Option<u32>,
    pub retry_in_ms: Option<u64>,
}

impl ReconnectMaskSnapshot {
    pub fn normal() -> Self {
        Self {
            status: ReconnectMaskStatus::Normal,
            reloading: false,
            message: None,
            preserves_last_frame: false,
            attempt: None,
            retry_in_ms: None,
        }
    }

    pub fn reloading(message: impl Into<String>) -> Self {
        Self {
            status: ReconnectMaskStatus::Reloading,
            reloading: true,
            message: Some(message.into()),
            preserves_last_frame: true,
            attempt: None,
            retry_in_ms: None,
        }
    }

    pub fn reconnecting(attempt: u32, retry_in_ms: u64, reason: impl Into<String>) -> Self {
        Self {
            status: ReconnectMaskStatus::Reconnecting,
            reloading: true,
            message: Some(reason.into()),
            preserves_last_frame: true,
            attempt: Some(attempt),
            retry_in_ms: Some(retry_in_ms),
        }
    }

    pub fn is_active(&self) -> bool {
        self.reloading || self.status != ReconnectMaskStatus::Normal
    }

    pub fn demo_fixture() -> Self {
        Self::normal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_snapshot_is_not_active() {
        let snap = ReconnectMaskSnapshot::normal();
        assert!(!snap.is_active());
        assert!(!snap.reloading);
        assert!(!snap.preserves_last_frame);
    }

    #[test]
    fn reloading_snapshot_is_active_and_preserves_last_frame() {
        let snap = ReconnectMaskSnapshot::reloading("reloading core configs");
        assert!(snap.is_active());
        assert!(snap.reloading);
        assert!(snap.preserves_last_frame);
        assert_eq!(snap.message.as_deref(), Some("reloading core configs"));
    }

    #[test]
    fn reconnecting_snapshot_tracks_attempts() {
        let snap = ReconnectMaskSnapshot::reconnecting(3, 500, "waiting watchdog retry");
        assert!(snap.is_active());
        assert_eq!(snap.attempt, Some(3));
        assert_eq!(snap.retry_in_ms, Some(500));
    }
}
