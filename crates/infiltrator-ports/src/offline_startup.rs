//! Local-only startup validation supplied by a host composition.

use async_trait::async_trait;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;

use crate::error::PortError;

/// Host proof for an offline-first Mihomo cold start.
#[async_trait]
pub trait OfflineStartupPort: Send + Sync {
    async fn validate_offline_startup(&self) -> Result<OfflineStartupSnapshot, PortError>;
}
