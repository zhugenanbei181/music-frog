//! Host port for AppContainer loopback discovery and exemption mutation.

use async_trait::async_trait;
use infiltrator_contract::uwp::UwpPackageSnapshot;

use crate::error::PortError;

#[async_trait]
pub trait UwpLoopbackPort: Send + Sync {
    async fn scan(&self) -> Result<Vec<UwpPackageSnapshot>, PortError>;
    async fn set_exempt(&self, sid: &str, exempt: bool) -> Result<(), PortError>;
    async fn set_all(&self, exempt: bool) -> Result<(), PortError>;
}
