//! Shared contract for destructive/privileged network transaction tests.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

/// Privileged network surfaces that a host test adapter may exercise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivilegedNetworkOperation {
    TunService,
    SystemProxy,
    RouteRepair,
}

/// A bounded, explicit operation set. The application rejects empty or
/// duplicated lists before an adapter can touch the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivilegedNetworkRequest {
    pub operations: Vec<PrivilegedNetworkOperation>,
}

impl PrivilegedNetworkRequest {
    pub fn standard() -> Self {
        Self {
            operations: vec![
                PrivilegedNetworkOperation::TunService,
                PrivilegedNetworkOperation::SystemProxy,
                PrivilegedNetworkOperation::RouteRepair,
            ],
        }
    }
}

/// Transaction state shown by a diagnostic surface. `Cleaned` is a successful
/// terminal state; `Failed` is reserved for a primary failure or a failed
/// rollback and never silently reports a clean host.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrivilegedNetworkState {
    #[default]
    Idle,
    Injecting,
    Active,
    RollingBack,
    Cleaned,
    Unsupported {
        reason: String,
    },
    Failed {
        failure: Failure,
    },
}

/// Readback for one privileged transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivilegedNetworkSnapshot {
    pub state: PrivilegedNetworkState,
    pub operation_count: usize,
    pub injected: bool,
    pub cleanup_attempted: bool,
    pub rollback_attempted: bool,
    pub revision: u64,
}

impl PrivilegedNetworkSnapshot {
    pub fn unsupported(revision: u64, reason: impl Into<String>) -> Self {
        Self {
            state: PrivilegedNetworkState::Unsupported {
                reason: reason.into(),
            },
            revision,
            ..Self::default()
        }
    }

    pub fn failed(revision: u64, failure: Failure) -> Self {
        Self {
            state: PrivilegedNetworkState::Failed { failure },
            revision,
            ..Self::default()
        }
    }

    pub fn failed_after_injection(
        revision: u64,
        operation_count: usize,
        cleanup_attempted: bool,
        rollback_attempted: bool,
        failure: Failure,
    ) -> Self {
        Self {
            state: PrivilegedNetworkState::Failed { failure },
            operation_count,
            injected: true,
            cleanup_attempted,
            rollback_attempted,
            revision,
        }
    }

    pub fn active(revision: u64, operation_count: usize) -> Self {
        Self {
            state: PrivilegedNetworkState::Active,
            operation_count,
            injected: true,
            revision,
            ..Self::default()
        }
    }

    pub fn cleaned(revision: u64, operation_count: usize, rollback_attempted: bool) -> Self {
        Self {
            state: PrivilegedNetworkState::Cleaned,
            operation_count,
            cleanup_attempted: true,
            rollback_attempted,
            revision,
            ..Self::default()
        }
    }

    pub fn is_clean(&self) -> bool {
        self.state == PrivilegedNetworkState::Cleaned && !self.injected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_request_covers_each_privileged_network_surface_once() {
        let request = PrivilegedNetworkRequest::standard();
        assert_eq!(request.operations.len(), 3);
        assert_eq!(
            request.operations,
            vec![
                PrivilegedNetworkOperation::TunService,
                PrivilegedNetworkOperation::SystemProxy,
                PrivilegedNetworkOperation::RouteRepair,
            ]
        );
    }

    #[test]
    fn cleaned_snapshot_is_explicitly_not_injected() {
        let snapshot = PrivilegedNetworkSnapshot::cleaned(3, 2, true);
        assert!(snapshot.is_clean());
        assert!(snapshot.rollback_attempted);
        assert!(snapshot.cleanup_attempted);
    }
}
