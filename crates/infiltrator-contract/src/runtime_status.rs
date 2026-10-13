//! Cross-surface runtime status derived from owner readbacks.
//!
//! Each field is an independent status axis. An available control service, a
//! running Core, a ready VPN data plane, and verified proxy traffic are
//! separate facts and must never be collapsed into one `running` flag.

use crate::android_control::{VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceKind};
use crate::error::Failure;
use crate::outbound::identity::OutboundId;
use crate::vpn::VpnGeneration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum ControlServiceObservation {
    #[default]
    Unknown,
    NotApplicable,
    Available {
        owner_session_id: String,
        revision: u64,
    },
    Unavailable {
        failure: Failure,
    },
    Unsupported {
        reason: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum CoreProcessObservation {
    #[default]
    Unknown,
    Unselected,
    Stopped,
    Starting,
    Ready,
    Running,
    Stopping,
    Failed {
        failure: Option<Failure>,
    },
}

/// Controller reachability is separate from whether an authentication secret
/// is configured (`ControllerAuthSnapshot`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum ControllerApiObservation {
    #[default]
    Unknown,
    NotRunning,
    Connected {
        core_generation: u64,
        core_revision: u64,
    },
    Unavailable {
        failure: Failure,
    },
    Unsupported {
        failure: Failure,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum VpnAuthorizationObservation {
    #[default]
    Unknown,
    NotApplicable,
    Required,
    Granted,
    Unavailable {
        failure: Failure,
    },
    Unsupported {
        reason: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum TunDataPlaneObservation {
    #[default]
    Unknown,
    NotApplicable,
    PermissionRequired,
    StartRequested,
    Starting {
        readiness: VpnOwnerReadiness,
    },
    Ready {
        generation: VpnGeneration,
    },
    Stopping,
    Stopped,
    Revoked,
    CleanupPending {
        generation: VpnGeneration,
        resource: VpnOwnerResourceKind,
    },
    Failed {
        failure: String,
    },
    Unavailable {
        failure: String,
    },
    Unsupported {
        reason: String,
    },
    Inconsistent {
        lifecycle: VpnOwnerLifecycle,
        readiness: VpnOwnerReadiness,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyReadinessBlocker {
    CoreNotRunning,
    ControllerApiUnavailable,
    OwnerServiceUnavailable,
    VpnAuthorizationRequired,
    VpnAuthorizationUnavailable,
    VpnAuthorizationUnsupported,
    VpnAuthorizationUnknown,
    TunDataPlaneNotReady,
}

/// Evidence about traffic actually observed through the proxy path.
/// Routing intent or a running Core alone cannot produce a verified variant.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "details", rename_all = "snake_case")]
pub enum ProxyEffectivenessObservation {
    #[default]
    Unknown,
    DemoFixture,
    NotReady {
        blocker: ProxyReadinessBlocker,
    },
    NoActiveTraffic,
    ActiveTrafficUnattributed {
        active_connections: u32,
    },
    ProxiedTrafficObserved {
        core_generation: u64,
        outbound_id: OutboundId,
        active_connections: u32,
    },
    DirectTrafficObserved {
        active_connections: u32,
    },
    Unavailable {
        failure: Failure,
    },
    Unsupported {
        reason: String,
    },
}

/// Read-only status projection included in the shared surface snapshot.
/// Raw Core, VPN owner, and topology snapshots remain the proof source; this
/// model only joins their independently observed axes for frontend use.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeStatusSnapshot {
    pub control_service: ControlServiceObservation,
    pub core_process: CoreProcessObservation,
    pub controller_api: ControllerApiObservation,
    pub vpn_authorization: VpnAuthorizationObservation,
    pub tun_data_plane: TunDataPlaneObservation,
    pub proxy_effectiveness: ProxyEffectivenessObservation,
}

#[cfg(test)]
mod tests {
    use super::{
        ControllerApiObservation, ProxyEffectivenessObservation, ProxyReadinessBlocker,
        RuntimeStatusSnapshot,
    };
    use crate::core_identity::EngineId;
    use crate::outbound::identity::OutboundId;

    #[test]
    fn independent_runtime_axes_serialize_as_tagged_states() {
        let snapshot = RuntimeStatusSnapshot {
            controller_api: ControllerApiObservation::Connected {
                core_generation: 7,
                core_revision: 9,
            },
            proxy_effectiveness: ProxyEffectivenessObservation::NotReady {
                blocker: ProxyReadinessBlocker::TunDataPlaneNotReady,
            },
            ..RuntimeStatusSnapshot::default()
        };

        let value = serde_json::to_value(snapshot).expect("serialize runtime status");
        assert_eq!(value["controller_api"]["state"], "connected");
        assert_eq!(value["controller_api"]["details"]["core_generation"], 7);
        assert_eq!(value["proxy_effectiveness"]["state"], "not_ready");
        assert_eq!(
            value["proxy_effectiveness"]["details"]["blocker"],
            "tun_data_plane_not_ready"
        );
    }

    #[test]
    fn verified_proxy_observation_requires_an_engine_scoped_outbound_id() {
        let engine = EngineId::new("mihomo").expect("engine ID");
        let outbound_id = OutboundId::new(engine, "proxy-a").expect("outbound ID");
        let observation = ProxyEffectivenessObservation::ProxiedTrafficObserved {
            core_generation: 7,
            outbound_id: outbound_id.clone(),
            active_connections: 2,
        };

        let encoded = serde_json::to_string(&observation).expect("serialize proxy observation");
        let decoded: ProxyEffectivenessObservation =
            serde_json::from_str(&encoded).expect("deserialize proxy observation");
        assert_eq!(decoded, observation);
        assert_eq!(
            outbound_id.engine_id(),
            &EngineId::new("mihomo").expect("engine ID")
        );
    }
}
