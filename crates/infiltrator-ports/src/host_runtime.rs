//! Host-owned runtime handle exposed to inbound surfaces.

use crate::core_lifecycle::CoreLifecyclePort;
use crate::runtime_gateway::ManagedRuntime;
use crate::service_mode::ServiceModePort;
use crate::mtu_probe::MtuProbePort;
use crate::network_roaming::NetworkRoamingPort;
use crate::vpn_service::VpnServicePort;
use crate::system_proxy::SystemProxyPort;
use crate::pac::PacServicePort;
use std::path::PathBuf;
use std::sync::Arc;

/// Cross-platform value describing TUN privilege/service state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TunServiceStatus {
    InstalledAndRunning,
    InstalledStopped,
    NotInstalled,
    MissingPrivilege,
    Unsupported,
}

/// The only runtime handle an inbound UI/FFI surface should store.
///
/// Concrete process managers, controller clients and platform service types
/// stay behind the host adapter. `lifecycle_port` is provided for composition
/// code that needs a typed restart without exposing the application object.
pub trait HostRuntime: ManagedRuntime {
    fn controller_url(&self) -> String;
    fn core_binary_path(&self) -> PathBuf;
    fn tun_service_status(&self) -> TunServiceStatus;
    /// Optional typed privileged-service adapter. Legacy/mobile hosts may
    /// omit it and continue to report an explicit unsupported state.
    fn service_mode_port(&self) -> Option<Arc<dyn ServiceModePort>> {
        None
    }
    /// Optional physical-link MTU observer. Mobile hosts may return `None`
    /// until their native bridge exposes the active link facts.
    fn mtu_probe_port(&self) -> Option<Arc<dyn MtuProbePort>> {
        None
    }
    /// Optional host system HTTP/SOCKS proxy controller. Mobile hosts may
    /// omit it because VPN routing owns process traffic instead.
    fn system_proxy_port(&self) -> Option<Arc<dyn SystemProxyPort>> {
        None
    }
    /// Optional local PAC service. Mobile hosts may omit this when they do
    /// not expose a desktop-style loopback proxy configuration surface.
    fn pac_service_port(&self) -> Option<Arc<dyn PacServicePort>> {
        None
    }
    /// Optional physical-link/default-gateway observer and TUN route repair
    /// adapter. Mobile hosts remain explicitly unsupported until their native
    /// VPN bridge exposes equivalent facts and route ownership.
    fn network_roaming_port(&self) -> Option<Arc<dyn NetworkRoamingPort>> {
        None
    }
    /// Optional native VPN service adapter. Desktop/iOS hosts return `None`
    /// and surface a typed unsupported state instead of emulating Android.
    fn vpn_service_port(&self) -> Option<Arc<dyn VpnServicePort>> {
        None
    }
    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort>;
}
