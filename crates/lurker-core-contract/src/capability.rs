//! Engine-neutral capability vocabulary.

use serde::{Deserialize, Serialize};

/// A capability describes what a host can actually perform.
///
/// The availability, status and snapshot types remain product-owned; this
/// vocabulary is shared so a neutral port can name the capability it rejects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Start, stop and observe the lifecycle of a proxy core.
    CoreLifecycle,
    /// Manage the profile catalogue and the active profile.
    Profiles,
    /// Switch the routing intent of the active core.
    RoutingIntent,
    /// Observe and terminate established connections.
    Connections,
    /// Follow the core log stream.
    Logs,
    /// Configure and observe DNS handling.
    Dns,
    /// Configure the TUN device of the core.
    Tun,
    /// Configure the operating system HTTP proxy settings.
    SystemProxy,
    /// Control which LAN peers may reach the proxy.
    LanAccessControl,
    /// Route IPv6 traffic through the core.
    Ipv6Routing,
    /// Expose the UWP loopback helper.
    UwpLoopback,
    /// Publish a PAC script for the operating system.
    PacService,
    /// React to network roaming (interface change) events.
    NetworkRoaming,
    /// Run the Android VPN service.
    VpnService,
    /// Register the application to start with the operating system.
    Autostart,
    /// Install or update a core binary version.
    CoreVersionInstall,
    /// Synchronise profiles with a WebDAV endpoint.
    WebDavSync,
    /// Route application traffic through the core.
    AppRouting,
    /// Measure the throughput of the active proxy path.
    Speedtest,
}
