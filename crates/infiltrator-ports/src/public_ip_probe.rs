//! Outbound port for public-egress probing.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::snapshot::PublicIpSnapshot;

#[async_trait]
pub trait PublicIpProbe: Send + Sync {
    async fn probe(&self, proxy_endpoint: Option<String>) -> Result<PublicIpSnapshot, PortError>;
}
