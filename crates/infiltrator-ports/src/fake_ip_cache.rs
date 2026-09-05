//! Fake-IP cache maintenance capability supplied by a host adapter.

use crate::error::PortError;

#[async_trait::async_trait]
pub trait FakeIpCachePort: Send + Sync {
    async fn clear(&self) -> Result<bool, PortError>;
}
