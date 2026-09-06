//! Shared live contract for Mihomo IPv6 routing policy.

use serde::{Deserialize, Serialize};

/// Confirmed top-level Mihomo IPv6 policy plus the current TUN context.
///
/// This is deliberately a core-level policy snapshot; it does not claim to mutate
/// the host operating system's global IPv6 sysctl or firewall state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ipv6RoutingSnapshot {
    pub enabled: bool,
    pub tun_enabled: bool,
    pub revision: u64,
}

impl Default for Ipv6RoutingSnapshot {
    fn default() -> Self {
        Self {
            enabled: true,
            tun_enabled: false,
            revision: 0,
        }
    }
}

impl Ipv6RoutingSnapshot {
    pub const fn new(revision: u64, enabled: bool, tun_enabled: bool) -> Self {
        Self {
            enabled,
            tun_enabled,
            revision,
        }
    }
}
