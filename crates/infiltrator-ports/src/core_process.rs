use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;

/// Controls the mihomo execution host without exposing process handles.
#[async_trait]
pub trait CoreProcess: Send + Sync {
    async fn start(&self) -> Result<(), PortError>;
    async fn stop(&self) -> Result<(), PortError>;
    async fn status(&self) -> Result<CoreLifecycle, PortError>;
    fn controller_endpoint(&self) -> Option<String>;
}
