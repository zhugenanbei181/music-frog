//! Outbound port definition for media and AI unlock capability probing (FLPARITY-020).

use async_trait::async_trait;
use infiltrator_contract::media_unlock::{NodeUnlockReport, UnlockServiceKind};
use lurker_core_ports::error::PortError;

/// Port abstraction for performing network unlock probes against proxy nodes.
#[async_trait]
pub trait MediaUnlockProbePort: Send + Sync {
    /// Probe the specified media/AI unlock services for a proxy node.
    async fn probe_node_unlock(
        &self,
        node_name: &str,
        services: &[UnlockServiceKind],
    ) -> Result<NodeUnlockReport, PortError>;

    /// Probe the specified media/AI unlock services through the currently active egress outbound.
    async fn probe_active_unlock(
        &self,
        services: &[UnlockServiceKind],
    ) -> Result<NodeUnlockReport, PortError>;
}
