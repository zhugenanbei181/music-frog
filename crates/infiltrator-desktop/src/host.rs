//! Desktop host capability declaration for the 0.30 application seam.

use infiltrator_contract::capability::{
    Availability, Capability, CapabilitySnapshot, CapabilityStatus,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::capability_provider::CapabilityProvider;

/// Static desktop capability set. Dynamic availability (for example a
/// missing privilege or an unavailable helper service) is reported by the
/// corresponding port operation, not hidden behind a false static claim.
#[derive(Clone, Copy, Debug, Default)]
pub struct DesktopHostCapabilities;

impl CapabilityProvider for DesktopHostCapabilities {
    fn host_kind(&self) -> HostKind {
        HostKind::Desktop
    }

    fn capabilities(&self) -> CapabilitySnapshot {
        let supported = |capability| CapabilityStatus {
            capability,
            availability: Availability::Supported,
        };
        CapabilitySnapshot::new(
            HostKind::Desktop,
            1,
            vec![
                supported(Capability::CoreLifecycle),
                supported(Capability::Profiles),
                supported(Capability::ProxyMode),
                supported(Capability::Connections),
                supported(Capability::Logs),
                supported(Capability::Dns),
                supported(Capability::Tun),
                supported(Capability::SystemProxy),
                supported(Capability::Autostart),
                supported(Capability::CoreVersionInstall),
                supported(Capability::WebDavSync),
                supported(Capability::AppRouting),
            ],
        )
    }
}
