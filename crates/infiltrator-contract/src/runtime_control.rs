//! Controller observations independent of successful preference reads.
use crate::command::ProxyMode;
use crate::error::Failure;
use crate::ipv6::Ipv6RoutingSnapshot;
use crate::lan::LanSecuritySnapshot;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeControlStatus {
    #[default]
    Unobserved,
    Ready,
    Failed {
        failure: Failure,
    },
    Unsupported {
        failure: Failure,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeControlSnapshot {
    pub status: RuntimeControlStatus,
    pub mode: Option<ProxyMode>,
    pub script_available: Option<bool>,
    pub tun_enabled: Option<bool>,
    pub mixed_port: Option<u16>,
    pub allow_lan: Option<bool>,
    pub log_level: Option<String>,
    pub ipv6_routing: Option<Ipv6RoutingSnapshot>,
    pub lan_bind_address: Option<String>,
    pub lan_security: Option<LanSecuritySnapshot>,
    pub tun_stack: Option<String>,
    pub tun_auto_route: Option<bool>,
    pub tun_strict_route: Option<bool>,
}
