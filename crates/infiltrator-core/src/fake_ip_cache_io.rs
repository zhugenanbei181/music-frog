//! Host filesystem adapter for the Mihomo Fake-IP cache.

use crate::error::{CoreContext, CoreError};
use crate::settings_io::app_config_manager;
use infiltrator_ports::fake_ip_cache::FakeIpCachePort;
use lurker_core_ports::error::PortError;
use tokio::fs::{remove_file, try_exists};

pub struct FileFakeIpCache;

impl FileFakeIpCache {
    pub fn current() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FakeIpCachePort for FileFakeIpCache {
    async fn clear(&self) -> Result<bool, PortError> {
        clear_fake_ip_cache_impl()
            .await
            .map_err(|error| PortError::Io(error.to_string()))
    }
}

pub async fn clear_fake_ip_cache() -> Result<bool, CoreError> {
    FileFakeIpCache::current()
        .clear()
        .await
        .map_err(|error| CoreError::other(error.to_string()))
}

async fn clear_fake_ip_cache_impl() -> Result<bool, CoreError> {
    let manager = app_config_manager()
        .await
        .core_context("init config manager")?;
    let profile_path = manager
        .get_current_path()
        .await
        .core_context("load current profile path")?;
    let config_dir = profile_path
        .parent()
        .ok_or_else(|| CoreError::invalid("profile path has no parent directory"))?;
    let cache_path = config_dir.join("fake-ip-cache");
    if try_exists(&cache_path)
        .await
        .core_context("check fake-ip cache")?
    {
        remove_file(&cache_path)
            .await
            .core_context("remove fake-ip cache")?;
        return Ok(true);
    }
    Ok(false)
}
