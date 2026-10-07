//! Host-owned privileged service-mode operations.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::service_mode::ServiceModeSnapshot;

#[async_trait]
pub trait ServiceModePort: Send + Sync {
    async fn snapshot(&self) -> Result<ServiceModeSnapshot, PortError>;
    async fn prepare(&self) -> Result<ServiceModeSnapshot, PortError>;
}
