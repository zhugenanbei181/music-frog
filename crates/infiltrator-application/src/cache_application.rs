//! Cache maintenance use-cases over host-provided cache ports.

use infiltrator_contract::error::Failure;
use infiltrator_ports::fake_ip_cache::FakeIpCachePort;
use std::sync::Arc;

#[derive(Clone)]
pub struct CacheApplication {
    fake_ip: Arc<dyn FakeIpCachePort>,
}

impl CacheApplication {
    pub fn new(fake_ip: Arc<dyn FakeIpCachePort>) -> Self {
        Self { fake_ip }
    }

    pub async fn clear_fake_ip(&self) -> Result<bool, Failure> {
        self.fake_ip.clear().await.map_err(Failure::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_ports::error::PortError;
    use std::sync::Arc;

    struct FakeCache {
        removed: bool,
    }

    #[async_trait]
    impl FakeIpCachePort for FakeCache {
        async fn clear(&self) -> Result<bool, PortError> {
            Ok(self.removed)
        }
    }

    #[tokio::test]
    async fn clears_fake_ip_through_the_port() {
        let application = CacheApplication::new(Arc::new(FakeCache { removed: true }));
        assert!(application.clear_fake_ip().await.expect("clear cache"));
    }
}
