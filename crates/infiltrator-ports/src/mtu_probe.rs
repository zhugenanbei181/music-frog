//! Host-owned physical-link MTU observation.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::mtu::PhysicalMtuSnapshot;

#[async_trait]
pub trait MtuProbePort: Send + Sync {
    async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError>;
}
