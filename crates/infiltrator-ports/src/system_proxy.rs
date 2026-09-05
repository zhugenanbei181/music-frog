//! Host-owned system HTTP/SOCKS proxy capability.

use async_trait::async_trait;
use infiltrator_contract::system_proxy::SystemProxyObservation;

use crate::error::PortError;

#[async_trait]
pub trait SystemProxyPort: Send + Sync {
    async fn snapshot(&self) -> Result<SystemProxyObservation, PortError>;
    async fn apply(
        &self,
        endpoint: Option<String>,
        bypass: Option<String>,
    ) -> Result<(), PortError>;
}
