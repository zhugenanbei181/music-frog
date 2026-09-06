//! Host port for Android VpnService permission, foreground and tunnel state.

use async_trait::async_trait;
use infiltrator_contract::vpn::{VpnConfiguration, VpnSessionSnapshot, VpnStartRequest};

use crate::error::PortError;

#[async_trait]
pub trait VpnServicePort: Send + Sync {
    async fn request_start(&self) -> Result<VpnSessionSnapshot, PortError>;
    async fn prepare(
        &self,
        configuration: VpnConfiguration,
    ) -> Result<VpnSessionSnapshot, PortError>;
    async fn start(&self, request: VpnStartRequest) -> Result<VpnSessionSnapshot, PortError>;
    async fn stop(&self) -> Result<VpnSessionSnapshot, PortError>;
    async fn revoke(&self) -> Result<VpnSessionSnapshot, PortError>;
    async fn snapshot(&self) -> Result<VpnSessionSnapshot, PortError>;
}
