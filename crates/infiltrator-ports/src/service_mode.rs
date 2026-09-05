//! Host-owned privileged service-mode operations.

use async_trait::async_trait;
use infiltrator_contract::service_mode::ServiceModeSnapshot;

use crate::error::PortError;

#[async_trait]
pub trait ServiceModePort: Send + Sync {
    async fn snapshot(&self) -> Result<ServiceModeSnapshot, PortError>;
    async fn prepare(&self) -> Result<ServiceModeSnapshot, PortError>;
}
