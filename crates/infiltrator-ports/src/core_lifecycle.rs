use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use std::time::Duration;

/// Application-facing lifecycle capability used by config transactions and
/// inbound adapters. It contains no session implementation or executor type.
#[async_trait]
pub trait CoreLifecyclePort: Send + Sync {
    fn lifecycle(&self) -> CoreLifecycle;
    fn generation(&self) -> u64;

    async fn start(&self) -> Result<u64, PortError>;
    async fn stop(&self) -> Result<(), PortError>;
    async fn restart(&self) -> Result<u64, PortError>;
    async fn wait_for_ready(&self, generation: u64, timeout: Duration) -> Result<(), PortError>;
}
