//! Engine-neutral capability vocabulary.

use serde::{Deserialize, Serialize};

/// A capability describes what a host can actually perform.
///
/// The availability, status and snapshot types remain product-owned; this
/// vocabulary is shared so a neutral port can name the capability it rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    CoreLifecycle,
    Profiles,
    RoutingIntent,
    Connections,
    Logs,
    Dns,
    Tun,
    SystemProxy,
    LanAccessControl,
    Ipv6Routing,
    UwpLoopback,
    PacService,
    NetworkRoaming,
    VpnService,
    Autostart,
    CoreVersionInstall,
    WebDavSync,
    AppRouting,
    Speedtest,
}
