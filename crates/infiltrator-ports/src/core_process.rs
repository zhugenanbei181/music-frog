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

    /// Reconcile a process record left by a previous host instance. A host
    /// without a process table (Android/iOS bridge) may safely return `None`.
    async fn cleanup_orphaned(&self) -> Result<Option<u32>, PortError> {
        Ok(None)
    }

    async fn pid(&self) -> Option<u32> {
        None
    }
}

/// Proves that the controller is ready without exposing an HTTP client or
/// polling/timer implementation to the application boundary.
#[async_trait]
pub trait CoreReadiness: Send + Sync {
    async fn probe(&self) -> Result<String, PortError>;
}
