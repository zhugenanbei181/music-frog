//! Host port for physical-link observations and safe TUN route repair.

use async_trait::async_trait;
use infiltrator_contract::network_roaming::{
    NetworkObservation, NetworkRoamingRepairRequest, NetworkRoamingRepairResult,
};

use crate::error::PortError;

#[async_trait]
pub trait NetworkRoamingPort: Send + Sync {
    async fn observe(&self) -> Result<NetworkObservation, PortError>;

    async fn repair(
        &self,
        request: NetworkRoamingRepairRequest,
    ) -> Result<NetworkRoamingRepairResult, PortError>;
}
