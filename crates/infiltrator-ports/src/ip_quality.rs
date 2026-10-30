//! Outbound port definition for IP quality and fraud intelligence probing (FLPARITY-001).

use async_trait::async_trait;
use infiltrator_contract::ip_quality::IpQualityRecord;
use lurker_core_ports::error::PortError;

/// Port abstraction for performing IP quality and fraud scoring probes.
#[async_trait]
pub trait IpQualityProbePort: Send + Sync {
    /// Probe IP quality facts for an explicit IP address.
    async fn probe_ip_quality(&self, ip: &str) -> Result<IpQualityRecord, PortError>;

    /// Probe IP quality facts through the currently active egress connection.
    async fn probe_active_ip_quality(&self) -> Result<IpQualityRecord, PortError>;
}
