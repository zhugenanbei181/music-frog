//! Host-local Fake-IP file-cache maintenance; this does not address the
//! running controller's Fake-IP table. That operation uses
//! [`crate::controller_fake_ip_cache::ControllerFakeIpCachePort`].

use crate::error::PortError;

#[async_trait::async_trait]
pub trait FakeIpCachePort: Send + Sync {
    async fn clear(&self) -> Result<bool, PortError>;
}
