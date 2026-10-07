//! Host port for AppContainer loopback discovery and exemption mutation.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::uwp::UwpPackageSnapshot;

#[async_trait]
pub trait UwpLoopbackPort: Send + Sync {
    async fn scan(&self) -> Result<Vec<UwpPackageSnapshot>, PortError>;
    async fn set_exempt(&self, sid: &str, exempt: bool) -> Result<(), PortError>;
    async fn set_all(&self, exempt: bool) -> Result<(), PortError>;
}
