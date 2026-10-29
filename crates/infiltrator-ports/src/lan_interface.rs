//! Outbound port definition for local network interface enumeration (FLPARITY-026).

use async_trait::async_trait;
use infiltrator_contract::lan_share::NetworkInterfaceInfo;
use lurker_core_ports::error::PortError;

/// Port abstraction for inspecting local host network interfaces and addresses.
#[async_trait]
pub trait LanInterfaceProbePort: Send + Sync {
    /// Enumerate all active network interfaces and their IPv4/IPv6 addresses.
    async fn list_local_interfaces(&self) -> Result<Vec<NetworkInterfaceInfo>, PortError>;
}
