//! Outbound port for public-egress probing.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::snapshot::PublicIpSnapshot;

/// Explicit route selected for one public-egress observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicIpProbeRoute {
    Direct,
    ThroughHttpProxy { endpoint: String },
}

#[async_trait]
pub trait PublicIpProbe: Send + Sync {
    async fn probe(&self, route: PublicIpProbeRoute) -> Result<PublicIpSnapshot, PortError>;
}
