//! Profile reset capability supplied by a host adapter.

use lurker_core_ports::error::PortError;

#[async_trait::async_trait]
pub trait ProfileResetPort: Send + Sync {
    async fn reset_to_default(&self) -> Result<(), PortError>;
}
