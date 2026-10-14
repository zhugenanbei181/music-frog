//! Shared Android VpnService session contract.

use crate::android_control::VpnOwnerSnapshot;
use crate::core_identity::CoreScope;
use crate::error::Failure;
use serde::{Deserialize, Serialize};

pub const MIN_VPN_MTU_BYTES: u32 = 1280;
pub const MAX_VPN_MTU_BYTES: u32 = 9000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnRoute {
    pub address: String,
    pub prefix: u8,
    pub exclude: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnConfiguration {
    pub proxy_endpoint: String,
    pub mtu: u32,
    pub routes: Vec<VpnRoute>,
    pub dns_servers: Vec<String>,
    pub ipv6: bool,
    pub foreground_requested: bool,
}

/// Owner-local generation of one Android VpnService activation attempt.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct VpnGeneration(u64);

impl VpnGeneration {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Opaque identity for a single frozen Builder/worker configuration handoff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VpnPreparationId(u64);

impl VpnPreparationId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Frozen VPN Builder configuration bound to one VPN and Core activation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedVpnConfiguration {
    pub preparation_id: VpnPreparationId,
    pub vpn_generation: VpnGeneration,
    /// Owner-data revision captured atomically with the frozen Core, TUN and
    /// DNS inputs. Activation must compare this value again before Builder.
    pub owner_data_revision: u64,
    pub core_scope: CoreScope,
    pub configuration: VpnConfiguration,
}

/// Opaque lease that keeps owner-data mutations from changing the frozen VPN
/// configuration between Builder admission and data-plane start readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnConfigurationActivationLease {
    pub lease_id: u64,
    pub preparation_id: VpnPreparationId,
    pub vpn_generation: VpnGeneration,
    pub owner_data_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnStartRequest {
    pub tun_fd: i32,
    pub prepared: PreparedVpnConfiguration,
}

/// Target of a VPN stop command. A generation target fences asynchronous
/// worker-exit cleanup so an old worker cannot stop a newer activation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnStopTarget {
    Current,
    Generation(VpnGeneration),
}

impl VpnStopTarget {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::Current => 0,
            Self::Generation(_) => 1,
        }
    }

    /// Zero is ignored for `Current`; a generation target must carry a
    /// non-zero owner generation.
    pub const fn generation_wire_value(self) -> u64 {
        match self {
            Self::Current => 0,
            Self::Generation(generation) => generation.value(),
        }
    }
}

/// Result of asking the Android host to obtain VPN consent and start its
/// foreground `VpnService`. This remains distinct from the later typed
/// `VpnStartReceipt` for the Rust packet worker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnStartOutcome {
    Running,
    PermissionRequired,
    OwnerUnavailable,
    Failed,
    TimedOut,
}

/// Result of submitting the route/DNS/MTU transaction to Android `Builder`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnConfigurationApplyOutcome {
    Accepted,
    Rejected,
    OwnerUnavailable,
}

/// Terminal acknowledgement from the Android VpnService shutdown transaction.
/// Every unsuccessful terminal preserves its typed failure. Variants with
/// owner readback require that snapshot; variants without readback cannot be
/// mistaken for an owner-authored terminal observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "receipt", deny_unknown_fields)]
pub enum VpnOwnerStopReceipt {
    Stopped {
        snapshot: VpnOwnerSnapshot,
    },
    OwnerUnavailable {
        failure: Failure,
    },
    OwnerUnavailableWithReadback {
        failure: Failure,
        snapshot: VpnOwnerSnapshot,
    },
    Failed {
        failure: Failure,
    },
    FailedWithReadback {
        failure: Failure,
        snapshot: VpnOwnerSnapshot,
    },
    TimedOut {
        failure: Failure,
    },
    TimedOutWithReadback {
        failure: Failure,
        snapshot: VpnOwnerSnapshot,
    },
    /// A generation-scoped request referred to an activation older than the
    /// current owner state and therefore performed no cleanup.
    Superseded {
        failure: Failure,
    },
    SupersededWithReadback {
        failure: Failure,
        snapshot: VpnOwnerSnapshot,
    },
    /// The caller stopped waiting before the owner returned its readback.
    Canceled {
        failure: Failure,
    },
    CanceledWithReadback {
        failure: Failure,
        snapshot: VpnOwnerSnapshot,
    },
}

impl VpnConfigurationApplyOutcome {
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::Accepted => 0,
            Self::Rejected => 1,
            Self::OwnerUnavailable => 2,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Accepted),
            1 => Some(Self::Rejected),
            2 => Some(Self::OwnerUnavailable),
            _ => None,
        }
    }
}

impl VpnStartOutcome {
    /// Stable wire values used only at the Kotlin/JNI boundary.
    pub const fn wire_code(self) -> i32 {
        match self {
            Self::Running => 0,
            Self::PermissionRequired => 1,
            Self::OwnerUnavailable => 2,
            Self::Failed => 3,
            Self::TimedOut => 4,
        }
    }

    pub const fn from_wire_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Running),
            1 => Some(Self::PermissionRequired),
            2 => Some(Self::OwnerUnavailable),
            3 => Some(Self::Failed),
            4 => Some(Self::TimedOut),
            _ => None,
        }
    }
}

impl VpnStartRequest {
    pub fn configuration(&self) -> &VpnConfiguration {
        &self.prepared.configuration
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnSessionState {
    #[default]
    Idle,
    PermissionRequired,
    Starting,
    Running,
    Stopping,
    Stopped,
    Revoked,
    Unsupported {
        reason: String,
    },
    Failed {
        failure: Failure,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnSessionSnapshot {
    pub state: VpnSessionState,
    pub foreground: bool,
    pub mtu: Option<u32>,
    pub route_count: usize,
    pub dns_servers: Vec<String>,
    pub ipv6: bool,
    pub revision: u64,
}

/// The VPN read model published with a shared surface snapshot.
///
/// Android surfaces carry the exact immutable snapshot authored by the `:vpn`
/// owner. Hosts without this service report an explicit typed disposition;
/// they do not synthesize an Android-shaped lifecycle from local flags.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnSurfaceObservation {
    AndroidOwner(Box<VpnOwnerSnapshot>),
    Unsupported {
        reason: String,
    },
    #[default]
    NotApplicable,
    Unavailable {
        failure: Failure,
    },
}

impl VpnSurfaceObservation {
    pub fn from_android_owner(snapshot: VpnOwnerSnapshot) -> Self {
        Self::AndroidOwner(Box::new(snapshot))
    }

    pub fn android_owner(&self) -> Result<&VpnOwnerSnapshot, &Self> {
        match self {
            Self::AndroidOwner(snapshot) => Ok(snapshot),
            Self::Unsupported { .. } | Self::NotApplicable | Self::Unavailable { .. } => Err(self),
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            Self::AndroidOwner(snapshot) => snapshot.is_running(),
            Self::Unsupported { .. } | Self::NotApplicable | Self::Unavailable { .. } => false,
        }
    }
}

impl VpnSessionSnapshot {
    pub fn unsupported(revision: u64, reason: impl Into<String>) -> Self {
        Self {
            state: VpnSessionState::Unsupported {
                reason: reason.into(),
            },
            revision,
            ..Self::default()
        }
    }

    pub fn failed(revision: u64, failure: Failure) -> Self {
        Self {
            state: VpnSessionState::Failed { failure },
            revision,
            ..Self::default()
        }
    }

    pub fn running(
        revision: u64,
        mtu: u32,
        route_count: usize,
        dns_servers: Vec<String>,
        ipv6: bool,
        foreground: bool,
    ) -> Self {
        Self {
            state: VpnSessionState::Running,
            foreground,
            mtu: Some(mtu),
            route_count,
            dns_servers,
            ipv6,
            revision,
        }
    }

    pub fn is_running(&self) -> bool {
        self.state == VpnSessionState::Running && self.foreground
    }
}

#[cfg(test)]
mod tests {
    use super::{
        VpnConfigurationApplyOutcome, VpnGeneration, VpnOwnerStopReceipt, VpnStartOutcome,
        VpnStopTarget,
    };
    use crate::android_control::{
        VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerResourceFact,
        VpnOwnerResourceKind, VpnOwnerResourceState, VpnOwnerSnapshot,
    };
    use crate::error::{ErrorCode, Failure};

    #[test]
    fn stop_target_wire_fields_preserve_current_and_generation_semantics() {
        assert_eq!(VpnStopTarget::Current.wire_code(), 0);
        assert_eq!(VpnStopTarget::Current.generation_wire_value(), 0);

        let generation = VpnStopTarget::Generation(VpnGeneration::new(17));
        assert_eq!(generation.wire_code(), 1);
        assert_eq!(generation.generation_wire_value(), 17);
    }

    #[test]
    fn start_outcome_wire_codes_round_trip_and_reject_unknown_values() {
        let outcomes = [
            VpnStartOutcome::Running,
            VpnStartOutcome::PermissionRequired,
            VpnStartOutcome::OwnerUnavailable,
            VpnStartOutcome::Failed,
            VpnStartOutcome::TimedOut,
        ];
        for outcome in outcomes {
            assert_eq!(
                VpnStartOutcome::from_wire_code(outcome.wire_code()),
                Some(outcome)
            );
        }
        assert_eq!(VpnStartOutcome::from_wire_code(-1), None);
        assert_eq!(VpnStartOutcome::from_wire_code(5), None);
    }

    #[test]
    fn configuration_apply_wire_codes_round_trip_and_reject_unknown_values() {
        let outcomes = [
            VpnConfigurationApplyOutcome::Accepted,
            VpnConfigurationApplyOutcome::Rejected,
            VpnConfigurationApplyOutcome::OwnerUnavailable,
        ];
        for outcome in outcomes {
            assert_eq!(
                VpnConfigurationApplyOutcome::from_wire_code(outcome.wire_code()),
                Some(outcome)
            );
        }
        assert_eq!(VpnConfigurationApplyOutcome::from_wire_code(-1), None);
        assert_eq!(VpnConfigurationApplyOutcome::from_wire_code(3), None);
    }

    #[test]
    fn stop_receipt_union_requires_readback_only_on_explicit_readback_variants() {
        let snapshot = VpnOwnerSnapshot {
            owner_session_id: "owner-a".to_owned(),
            revision: 8,
            vpn_generation: VpnGeneration::new(5),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: [
                VpnOwnerResourceKind::PreparedConfiguration,
                VpnOwnerResourceKind::Worker,
                VpnOwnerResourceKind::TunnelDescriptor,
                VpnOwnerResourceKind::RoutingLease,
            ]
            .into_iter()
            .map(|kind| VpnOwnerResourceFact {
                kind,
                state: VpnOwnerResourceState::Absent,
            })
            .collect(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Unavailable {
                failure_message: "Core readback unavailable".to_owned(),
            },
        };
        let failure = Failure::new(ErrorCode::NotReady, "stop could not be confirmed", true);
        let receipts = [
            VpnOwnerStopReceipt::Stopped {
                snapshot: snapshot.clone(),
            },
            VpnOwnerStopReceipt::OwnerUnavailable {
                failure: failure.clone(),
            },
            VpnOwnerStopReceipt::OwnerUnavailableWithReadback {
                failure: failure.clone(),
                snapshot: snapshot.clone(),
            },
            VpnOwnerStopReceipt::Failed {
                failure: failure.clone(),
            },
            VpnOwnerStopReceipt::FailedWithReadback {
                failure: failure.clone(),
                snapshot: snapshot.clone(),
            },
            VpnOwnerStopReceipt::TimedOut {
                failure: failure.clone(),
            },
            VpnOwnerStopReceipt::TimedOutWithReadback {
                failure: failure.clone(),
                snapshot: snapshot.clone(),
            },
            VpnOwnerStopReceipt::Superseded {
                failure: failure.clone(),
            },
            VpnOwnerStopReceipt::SupersededWithReadback {
                failure: failure.clone(),
                snapshot: snapshot.clone(),
            },
            VpnOwnerStopReceipt::Canceled {
                failure: failure.clone(),
            },
            VpnOwnerStopReceipt::CanceledWithReadback { failure, snapshot },
        ];
        for receipt in receipts {
            let encoded = serde_json::to_string(&receipt).expect("serialize Stop receipt");
            assert!(encoded.contains("\"receipt\":"));
            assert!(!encoded.contains("\"outcome\":"));
            assert_eq!(
                serde_json::from_str::<VpnOwnerStopReceipt>(&encoded)
                    .expect("deserialize Stop receipt"),
                receipt,
            );
        }

        let legacy_impossible_pair = r#"{"outcome":"Stopped","snapshot":null}"#;
        assert!(serde_json::from_str::<VpnOwnerStopReceipt>(legacy_impossible_pair).is_err());
        let missing_stopped_readback = r#"{"receipt":"Stopped"}"#;
        assert!(serde_json::from_str::<VpnOwnerStopReceipt>(missing_stopped_readback).is_err());
        let nullable_stopped_readback = r#"{"receipt":"Stopped","snapshot":null}"#;
        assert!(serde_json::from_str::<VpnOwnerStopReceipt>(nullable_stopped_readback).is_err());
        let missing_failure = r#"{"receipt":"Failed"}"#;
        assert!(serde_json::from_str::<VpnOwnerStopReceipt>(missing_failure).is_err());
    }
}
