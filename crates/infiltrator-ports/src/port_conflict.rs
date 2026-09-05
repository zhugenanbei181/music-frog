//! Host-owned port conflict detection and safe repair.

use async_trait::async_trait;
use infiltrator_contract::port_conflict::PortConflictSnapshot;

use crate::error::PortError;

#[async_trait]
pub trait PortConflictPort: Send + Sync {
    async fn snapshot(&self) -> Result<PortConflictSnapshot, PortError>;
    async fn repair(&self) -> Result<PortConflictSnapshot, PortError>;
}
