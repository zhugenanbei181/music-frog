//! Shared system HTTP/SOCKS proxy observation and application state.

use lurker_core_contract::error::Failure;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxyObservation {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
}

/// The most recently confirmed target. Host adapters share this read model
/// between command and surface compositions in one process.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxyDesiredState {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub bypass: Option<String>,
}

/// Exact previous and desired host state recorded for a proxy claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemProxyClaim {
    pub revision: u64,
    pub owner_pid: u32,
    #[serde(default)]
    pub owner_start_time: u64,
    pub previous: SystemProxyObservation,
    pub desired: SystemProxyDesiredState,
    /// Exact value read back from the host after claim application.
    pub observed: SystemProxyObservation,
}

/// Explicit single-owner claim state shared by command and readback compositions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyClaimState {
    #[default]
    Unclaimed,
    Claimed(SystemProxyClaim),
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

impl SystemProxyDesiredState {
    /// Whether the host still reflects the value this application claimed.
    pub fn matches(&self, observation: &SystemProxyObservation) -> bool {
        if self.enabled != observation.enabled {
            return false;
        }
        !self.enabled
            || (self.endpoint == observation.endpoint
                && self
                    .bypass
                    .as_ref()
                    .is_none_or(|bypass| observation.bypass.as_ref() == Some(bypass)))
    }
}

/// Result of releasing the application's system-proxy claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemProxyReleaseReport {
    /// No durable application claim exists; the observed host value is left alone.
    NoClaim { observed: SystemProxyObservation },
    /// The host still matched the claim and the pre-claim value was restored.
    Restored {
        previous: SystemProxyObservation,
        restored: SystemProxyObservation,
    },
    /// The host no longer matched the claim, so the third-party value was preserved.
    SkippedExternal {
        expected: SystemProxyClaim,
        observed: SystemProxyObservation,
    },
    /// The durable claim does not match the claim this caller is releasing.
    SkippedStaleClaim {
        current: SystemProxyClaim,
        observed: SystemProxyObservation,
    },
    /// Another live process owns the durable claim; this caller did not change it.
    SkippedLiveOwner {
        current: SystemProxyClaim,
        observed: SystemProxyObservation,
    },
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

    #[test]
    fn desired_state_requires_the_owned_endpoint_and_declared_bypass() {
        let desired = SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        };
        assert!(desired.matches(&SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("localhost".to_owned()),
        }));
        assert!(!desired.matches(&SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:8080".to_owned()),
            bypass: Some("localhost".to_owned()),
        }));
        assert!(!desired.matches(&SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: Some("127.0.0.1".to_owned()),
        }));
    }
}
