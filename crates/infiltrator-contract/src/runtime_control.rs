//! Controller observations independent of successful preference reads.
use crate::ipv6::Ipv6RoutingSnapshot;
use crate::lan::{LanSecuritySnapshot, LanSharingSnapshot};
use lurker_core_contract::error::Failure;
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

/// The runtime configuration readback of the host.
///
/// Every fact is one typed observation: the LAN sharing configuration is
/// published as one [`LanSharingSnapshot`] instead of three independent
/// optional fields, so a consumer can tell "sharing is off" from "the
/// runtime configuration was never observed".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeControlSnapshot {
    pub status: RuntimeControlStatus,
    pub tun_enabled: Option<bool>,
    /// The confirmed LAN sharing configuration. `None` means the runtime
    /// configuration was never observed — not that sharing is off.
    #[serde(default)]
    pub lan_sharing: Option<LanSharingSnapshot>,
    pub log_level: Option<String>,
    pub ipv6_routing: Option<Ipv6RoutingSnapshot>,
    pub lan_security: Option<LanSecuritySnapshot>,
    pub tun_stack: Option<String>,
    pub tun_auto_route: Option<bool>,
    pub tun_strict_route: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runtime_control_publishes_the_lan_sharing_readback_as_one_fact() {
        let sharing = LanSharingSnapshot::new(3, true, 7890, "*");
        let observed = RuntimeControlSnapshot {
            status: RuntimeControlStatus::Ready,
            lan_sharing: Some(sharing.clone()),
            ..Default::default()
        };
        assert_eq!(observed.lan_sharing, Some(sharing));
        assert!(observed.lan_sharing.as_ref().is_some_and(|s| s.enabled));

        // An unobserved runtime configuration keeps the readback absent
        // instead of reporting the sharing as off.
        let unobserved = RuntimeControlSnapshot::default();
        assert!(unobserved.lan_sharing.is_none());
    }
}
