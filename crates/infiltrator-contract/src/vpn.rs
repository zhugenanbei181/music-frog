//! Shared Android VpnService session contract.

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VpnStartRequest {
    pub tun_fd: i32,
    pub proxy_endpoint: String,
    pub mtu: u32,
    pub routes: Vec<VpnRoute>,
    pub dns_servers: Vec<String>,
    pub ipv6: bool,
    pub foreground_requested: bool,
}

impl VpnStartRequest {
    pub fn configuration(&self) -> VpnConfiguration {
        VpnConfiguration {
            proxy_endpoint: self.proxy_endpoint.clone(),
            mtu: self.mtu,
            routes: self.routes.clone(),
            dns_servers: self.dns_servers.clone(),
            ipv6: self.ipv6,
            foreground_requested: self.foreground_requested,
        }
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
