//! Host-owned runtime handle exposed to inbound surfaces.

use crate::core_lifecycle::CoreLifecyclePort;
use crate::mtu_probe::MtuProbePort;
use crate::network_roaming::NetworkRoamingPort;
use crate::pac::PacServicePort;
use crate::privileged_network::PrivilegedNetworkPort;
use crate::runtime_gateway::ManagedRuntime;
use crate::service_mode::ServiceModePort;
use crate::system_proxy::SystemProxyPort;
use crate::vpn_service::VpnServicePort;
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
    /// Optional host adapter used only by the privileged network regression
    /// transaction. No host may claim support without an injected adapter.
    fn privileged_network_port(&self) -> Option<Arc<dyn PrivilegedNetworkPort>> {
        None
    }
    /// DUAL-05-13: optional CA-bundle file reader. Hosts without one surface a
    /// typed unsupported certificate state instead of claiming a CA was loaded.
    fn certificate_authority_port(
        &self,
    ) -> Option<Arc<dyn crate::certificate_authority::CertificateAuthorityPort>> {
        None
    }
    /// Optional native VPN service adapter. Desktop/iOS hosts return `None`
    /// and surface a typed unsupported state instead of emulating Android.
    fn vpn_service_port(&self) -> Option<Arc<dyn VpnServicePort>> {
        None
    }
    /// Optional speedtest / jitter / packet-loss engine adapter. Hosts without
    /// one surface a typed unsupported state instead of fabricating metrics.
    fn speedtest_port(&self) -> Option<Arc<dyn crate::speedtest::SpeedtestPort>> {
        None
    }
    /// Optional live rule tracer adapter backed by the shared AST simulation
    /// engine. Hosts without one surface a typed unsupported state instead of
    /// replaying a UI-local decision chain.
    fn rule_tracer_port(&self) -> Option<Arc<dyn crate::rule_tracer::RuleTracerPort>> {
        None
    }
    /// Optional operating-system resolver cache adapter. Hosts without one
    /// surface a typed unsupported OS-cache outcome instead of claiming the
    /// system cache was refreshed.
    fn system_dns_cache_port(
        &self,
    ) -> Option<Arc<dyn crate::system_dns_cache::SystemDnsCachePort>> {
        None
    }
    /// Optional floating Mini HUD window adapter. Hosts that cannot move an
    /// always-on-top frameless window omit it; the persisted placement still
    /// round-trips through settings and is reported as typed unsupported.
    fn mini_hud_window_port(&self) -> Option<Arc<dyn crate::mini_hud_window::MiniHudWindowPort>> {
        None
    }
    /// DUAL-14-10: optional per-nameserver latency prober. Hosts without one
    /// publish a typed unsupported latency status instead of a made-up number.
    fn dns_latency_probe_port(&self) -> Option<Arc<dyn crate::dns_latency::DnsLatencyProbePort>> {
        None
    }
    /// DUAL-11-06/07: the kernel's local rule-provider files. Hosts without a
    /// resolvable kernel home directory omit it instead of reporting a purge
    /// that never happened.
    fn rule_provider_cache_port(
        &self,
    ) -> Option<Arc<dyn crate::rule_provider_cache::RuleProviderCachePort>> {
        None
    }
    fn lifecycle_port(&self) -> Arc<dyn CoreLifecyclePort>;
}
