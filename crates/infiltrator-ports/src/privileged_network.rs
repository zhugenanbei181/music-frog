//! Host port for privileged network injection and rollback tests.

use async_trait::async_trait;
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkRequest, PrivilegedNetworkSnapshot,
};

use crate::error::PortError;

#[async_trait]
pub trait PrivilegedNetworkPort: Send + Sync {
    async fn inject(
        &self,
        request: PrivilegedNetworkRequest,
    ) -> Result<PrivilegedNetworkSnapshot, PortError>;
    async fn cleanup(&self) -> Result<PrivilegedNetworkSnapshot, PortError>;
    async fn snapshot(&self) -> Result<PrivilegedNetworkSnapshot, PortError>;
}
