//! Host-owned port conflict detection and safe repair.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::port_conflict::PortConflictSnapshot;

#[async_trait]
pub trait PortConflictPort: Send + Sync {
    async fn snapshot(&self) -> Result<PortConflictSnapshot, PortError>;
    async fn repair(&self) -> Result<PortConflictSnapshot, PortError>;
}
