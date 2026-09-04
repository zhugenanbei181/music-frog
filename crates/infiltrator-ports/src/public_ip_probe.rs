//! Outbound port for public-egress probing.

use async_trait::async_trait;
use infiltrator_contract::snapshot::PublicIpSnapshot;

use crate::error::PortError;

#[async_trait]
pub trait PublicIpProbe: Send + Sync {
    async fn probe(&self, proxy_endpoint: Option<String>) -> Result<PublicIpSnapshot, PortError>;
}
