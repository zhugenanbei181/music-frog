//! Host port for physical-link observations and safe TUN route repair.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::network_roaming::{
    NetworkObservation, NetworkRoamingRepairRequest, NetworkRoamingRepairResult,
};

#[async_trait]
pub trait NetworkRoamingPort: Send + Sync {
    async fn observe(&self) -> Result<NetworkObservation, PortError>;

    async fn repair(
        &self,
        request: NetworkRoamingRepairRequest,
    ) -> Result<NetworkRoamingRepairResult, PortError>;
}
