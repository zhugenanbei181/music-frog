//! Contract for booting from local state when no network is available.
//!
//! The snapshot is deliberately about the startup dependency boundary, not
//! about a particular filesystem or process API.  A native mobile host can
//! validate its packaged assets and report the same value as the desktop
//! host, while both UI surfaces render the same degraded/blocked decision.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

/// Network policy for the first core start.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupNetworkPolicy {
    /// Local config and the packaged/installed core are sufficient to start.
    #[default]
    OfflineFirst,
}

/// Status of a local asset that may enrich startup but must not block an
/// otherwise valid local core launch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalAssetStatus {
    NotRequired,
    Available,
    Missing,
}

/// Result of the local-only startup preflight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineStartupState {
    Unknown,
    Checking,
    Ready,
    Degraded,
    Blocked,
}

/// Remote services are for post-start enhancement only.  In particular,
/// release feeds, GeoIP downloads and remote authentication are not startup
/// prerequisites under the 0.30 policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupRemoteDependency {
    #[default]
    Optional,
}

/// Shared proof of what the host found before attempting to start Mihomo.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineStartupSnapshot {
    pub policy: StartupNetworkPolicy,
    pub state: OfflineStartupState,
    pub config_valid: bool,
    pub binary_available: bool,
    pub geoip: LocalAssetStatus,
    pub remote_dependency: StartupRemoteDependency,
    pub failure: Option<Failure>,
}

impl Default for OfflineStartupSnapshot {
    fn default() -> Self {
        Self {
            policy: StartupNetworkPolicy::OfflineFirst,
            state: OfflineStartupState::Unknown,
            config_valid: false,
            binary_available: false,
            geoip: LocalAssetStatus::NotRequired,
            remote_dependency: StartupRemoteDependency::Optional,
            failure: None,
        }
    }
}

impl OfflineStartupSnapshot {
    /// Build a successful local preflight.  Missing GeoIP is a visible
    /// degradation, not a false success and not a startup blocker.
    pub fn ready(geoip: LocalAssetStatus) -> Self {
        Self {
            policy: StartupNetworkPolicy::OfflineFirst,
            state: if geoip == LocalAssetStatus::Missing {
                OfflineStartupState::Degraded
            } else {
                OfflineStartupState::Ready
            },
            config_valid: true,
            binary_available: true,
            geoip,
            remote_dependency: StartupRemoteDependency::Optional,
            failure: None,
        }
    }

    /// Build a fail-closed local preflight result without exposing secrets.
    pub fn blocked(failure: Failure) -> Self {
        Self {
            policy: StartupNetworkPolicy::OfflineFirst,
            state: OfflineStartupState::Blocked,
            config_valid: false,
            binary_available: false,
            geoip: LocalAssetStatus::NotRequired,
            remote_dependency: StartupRemoteDependency::Optional,
            failure: Some(failure),
        }
    }

    /// Whether the host may attempt a local core start without a remote call.
    pub fn is_offline_startable(&self) -> bool {
        self.config_valid
            && self.binary_available
            && matches!(
                self.state,
                OfflineStartupState::Ready | OfflineStartupState::Degraded
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{ErrorCode, Failure};

    #[test]
    fn missing_geoip_is_degraded_but_startable() {
        let snapshot = OfflineStartupSnapshot::ready(LocalAssetStatus::Missing);
        assert_eq!(snapshot.state, OfflineStartupState::Degraded);
        assert!(snapshot.is_offline_startable());
        assert_eq!(
            snapshot.remote_dependency,
            StartupRemoteDependency::Optional
        );
    }

    #[test]
    fn invalid_local_state_is_blocked() {
        let snapshot = OfflineStartupSnapshot::blocked(Failure::new(
            ErrorCode::Configuration,
            "invalid local profile",
            false,
        ));
        assert_eq!(snapshot.state, OfflineStartupState::Blocked);
        assert!(!snapshot.is_offline_startable());
        assert_eq!(snapshot.failure.unwrap().code, ErrorCode::Configuration);
    }
}
