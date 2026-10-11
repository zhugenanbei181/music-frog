use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::core_identity::{CoreIdentity, CoreVersion};
use infiltrator_contract::snapshot::CoreLifecycle;

/// Controls an engine execution host without exposing process handles.
#[async_trait]
pub trait CoreProcess: Send + Sync {
    /// Stable engine/configured-instance identity. Missing identity disables
    /// scoped core commands instead of guessing which backend owns the process.
    fn core_identity(&self) -> Option<CoreIdentity> {
        None
    }

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

    /// Authoritative version reported by the ready running engine, when the
    /// host can read it. Absence leaves version-scoped operations unavailable.
    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(None)
    }
}
