//! Shared physical-link roaming and default-gateway recovery contract.

use crate::error::Failure;
use serde::{Deserialize, Serialize};

/// Host-independent classification used to rank physical egress links.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkInterfaceKind {
    Ethernet,
    Wifi,
    Cellular,
    Tun,
    Loopback,
    Bridge,
    #[default]
    Other,
}

impl NetworkInterfaceKind {
    pub const fn default_metric(self) -> u32 {
        match self {
            Self::Ethernet => 100,
            Self::Wifi => 200,
            Self::Cellular => 300,
            Self::Bridge => 350,
            Self::Other => 400,
            Self::Tun => 500,
            Self::Loopback => 999,
        }
    }

    pub const fn is_physical(self) -> bool {
        matches!(self, Self::Ethernet | Self::Wifi | Self::Cellular)
    }

    pub const fn is_tun(self) -> bool {
        matches!(self, Self::Tun)
    }
}

/// One host-observed interface. It contains facts only; selection and repair
/// decisions belong to the domain/application layers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterfaceSnapshot {
    pub name: String,
    pub kind: NetworkInterfaceKind,
    pub is_up: bool,
    pub is_default_gateway: bool,
    #[serde(default)]
    pub gateway_ip: Option<String>,
    #[serde(default)]
    pub ip_addresses: Vec<String>,
    #[serde(default)]
    pub mtu: Option<u32>,
    #[serde(default)]
    pub metric: Option<u32>,
    #[serde(default)]
    pub dns_servers: Vec<String>,
}

/// Atomic host observation consumed by the roaming decision function.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkObservation {
    pub interfaces: Vec<NetworkInterfaceSnapshot>,
    pub observed_at_epoch_ms: Option<i64>,
}

/// Host action requested after a physical gateway migration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkRoamingRepairRequest {
    pub physical_interface: String,
    pub gateway_ip: Option<String>,
    pub tun_interface: String,
    pub strict_route: bool,
}

/// Readback returned by the host route adapter after a repair attempt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkRoamingRepairResult {
    pub route_generation: u64,
    pub detail: String,
}

/// User-visible event retained in the shared roaming snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkRoamingEvent {
    InitialObservation {
        interface: Option<String>,
        gateway_ip: Option<String>,
    },
    GatewayChanged {
        old_interface: Option<String>,
        new_interface: Option<String>,
        old_gateway_ip: Option<String>,
        new_gateway_ip: Option<String>,
    },
    InterfaceAddressChanged {
        interface: String,
    },
    RoutesRepaired {
        physical_interface: String,
        tun_interface: String,
        detail: String,
    },
    RepairSkipped {
        reason: String,
    },
    RepairFailed {
        failure: Failure,
    },
}

/// Stable roaming status. Unsupported/unavailable is never represented by an
/// empty successful interface list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkRoamingStatus {
    #[default]
    Unknown,
    Stable,
    Recovering,
    Degraded {
        reason: String,
    },
    Unsupported {
        reason: String,
    },
    Failed {
        failure: Failure,
    },
}

/// Canonical network roaming read model shared by Iced, Bevy and native hosts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkRoamingSnapshot {
    pub status: NetworkRoamingStatus,
    pub interfaces: Vec<NetworkInterfaceSnapshot>,
    pub active_interface: Option<String>,
    pub default_gateway: Option<String>,
    pub previous_interface: Option<String>,
    pub tun_interface: Option<String>,
    pub physical_mtu: Option<u32>,
    pub recommended_tun_mtu: Option<u32>,
    pub tcp_mss: Option<u32>,
    pub route_repair_count: u64,
    pub last_event: Option<NetworkRoamingEvent>,
    pub observed_at_epoch_ms: Option<i64>,
    pub revision: u64,
}

impl NetworkRoamingSnapshot {
    pub fn unsupported(revision: u64, reason: impl Into<String>) -> Self {
        Self {
            status: NetworkRoamingStatus::Unsupported {
                reason: reason.into(),
            },
            revision,
            ..Self::default()
        }
    }

    pub fn failed(revision: u64, failure: Failure) -> Self {
        Self {
            status: NetworkRoamingStatus::Failed { failure },
            revision,
            ..Self::default()
        }
    }
}
