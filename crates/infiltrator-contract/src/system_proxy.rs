//! Shared system HTTP/SOCKS proxy observation and application state.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxyObservation {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
}

/// The last proxy target confirmed by the application. Host adapters use a
/// shared store for this value so command and surface compositions can
/// reconcile the same ownership even when they wrap the port separately.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxyDesiredState {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyRecoveryReport {
    NotNeeded,
    Restored {
        previous: SystemProxyObservation,
        restored: SystemProxyObservation,
    },
    SkippedExternal {
        expected: SystemProxyDesiredState,
        observed: SystemProxyObservation,
    },
    SkippedLiveOwner {
        owner_pid: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyStatus {
    Unknown,
    Disabled,
    Enabled,
    Unsupported { failure: Failure },
    Failed { failure: Failure },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyOwnership {
    #[default]
    Unknown,
    Unmanaged,
    Owned,
    Repaired,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyRecoveryStatus {
    Unknown,
    NotNeeded,
    Restored {
        previous: SystemProxyObservation,
        restored: SystemProxyObservation,
    },
    SkippedExternal {
        expected: SystemProxyDesiredState,
        observed: SystemProxyObservation,
    },
    SkippedLiveOwner {
        owner_pid: u32,
    },
    Failed {
        failure: Failure,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxyRecoverySnapshot {
    pub status: SystemProxyRecoveryStatus,
    pub revision: u64,
}

impl Default for SystemProxyRecoverySnapshot {
    fn default() -> Self {
        Self {
            status: SystemProxyRecoveryStatus::Unknown,
            revision: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxySnapshot {
    pub status: SystemProxyStatus,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
    pub revision: u64,
    #[serde(default)]
    pub ownership: SystemProxyOwnership,
    #[serde(default)]
    pub repair_count: u64,
}

impl Default for SystemProxySnapshot {
    fn default() -> Self {
        Self {
            status: SystemProxyStatus::Unknown,
            endpoint: None,
            bypass: None,
            revision: 0,
            ownership: SystemProxyOwnership::Unknown,
            repair_count: 0,
        }
    }
}

impl SystemProxySnapshot {
    pub fn from_observation(revision: u64, observation: SystemProxyObservation) -> Self {
        Self {
            status: if observation.enabled {
                SystemProxyStatus::Enabled
            } else {
                SystemProxyStatus::Disabled
            },
            endpoint: observation.endpoint,
            bypass: observation.bypass,
            revision,
            ownership: SystemProxyOwnership::Unmanaged,
            repair_count: 0,
        }
    }

    pub fn unsupported(revision: u64, message: impl Into<String>) -> Self {
        Self {
            status: SystemProxyStatus::Unsupported {
                failure: Failure::unsupported(message),
            },
            revision,
            ..Self::default()
        }
    }

    pub fn failed(revision: u64, failure: Failure) -> Self {
        Self {
            status: SystemProxyStatus::Failed { failure },
            revision,
            ..Self::default()
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.status == SystemProxyStatus::Enabled
    }

    pub fn with_owned(mut self) -> Self {
        self.ownership = SystemProxyOwnership::Owned;
        self
    }

    pub fn with_repaired(mut self, repair_count: u64) -> Self {
        self.ownership = SystemProxyOwnership::Repaired;
        self.repair_count = repair_count;
        self
    }

    pub fn with_repair_count(mut self, repair_count: u64) -> Self {
        self.repair_count = repair_count;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_projects_enabled_endpoint_and_bypass() {
        let snapshot = SystemProxySnapshot::from_observation(
            3,
            SystemProxyObservation {
                enabled: true,
                endpoint: Some("127.0.0.1:7890".to_owned()),
                bypass: Some("localhost".to_owned()),
            },
        );
        assert!(snapshot.is_enabled());
        assert_eq!(snapshot.ownership, SystemProxyOwnership::Unmanaged);
        assert_eq!(snapshot.endpoint.as_deref(), Some("127.0.0.1:7890"));
        assert_eq!(snapshot.bypass.as_deref(), Some("localhost"));
    }

    #[test]
    fn unsupported_is_not_enabled() {
        let snapshot = SystemProxySnapshot::unsupported(4, "no host proxy port");
        assert!(!snapshot.is_enabled());
        assert!(matches!(
            snapshot.status,
            SystemProxyStatus::Unsupported { .. }
        ));
    }
}
