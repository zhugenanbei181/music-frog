//! Host port for serving a generated PAC script on a local endpoint.

use async_trait::async_trait;

use crate::error::PortError;

#[async_trait]
pub trait PacServicePort: Send + Sync {
    async fn start(&self, script: String) -> Result<String, PortError>;
    async fn stop(&self) -> Result<(), PortError>;
    async fn status(&self) -> Result<Option<String>, PortError>;
}
