//! Host-owned physical-link MTU observation.

use async_trait::async_trait;
use infiltrator_contract::mtu::PhysicalMtuSnapshot;

use crate::error::PortError;

#[async_trait]
pub trait MtuProbePort: Send + Sync {
    async fn probe_physical_mtu(&self) -> Result<PhysicalMtuSnapshot, PortError>;
}
