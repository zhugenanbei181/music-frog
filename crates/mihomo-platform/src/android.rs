use async_trait::async_trait;
use mihomo_api::{MihomoError, Result};
use std::path::PathBuf;
use std::sync::Arc;

use crate::android_bridge::{get_android_bridge, AndroidBridge};
use crate::traits::{CoreController, CredentialStore, DataDirProvider};

pub struct AndroidCoreController;

#[async_trait]
impl CoreController for AndroidCoreController {
    async fn start(&self) -> Result<()> {
        let bridge = require_service_bridge("core start")?;
        bridge.core_start().await
    }

    async fn stop(&self) -> Result<()> {
        let bridge = require_service_bridge("core stop")?;
        bridge.core_stop().await
    }

    async fn is_running(&self) -> bool {
        match get_android_bridge() {
            Some(bridge) => bridge.core_is_running().await.unwrap_or_else(|err| {
                log::warn!("android core is_running failed: {err}");
                false
            }),
            None => false,
        }
    }

    fn controller_url(&self) -> Option<String> {
        get_android_bridge().and_then(|bridge| bridge.core_controller_url())
    }
}

pub struct AndroidCredentialStore;

impl Default for AndroidCredentialStore {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl CredentialStore for AndroidCredentialStore {
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>> {
        let bridge = require_config_bridge("credential get")?;
        bridge.credential_get(service, key).await
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()> {
        let bridge = require_config_bridge("credential set")?;
        bridge.credential_set(service, key, value).await
    }

    async fn delete(&self, service: &str, key: &str) -> Result<()> {
        let bridge = require_config_bridge("credential delete")?;
        bridge.credential_delete(service, key).await
    }
}

pub struct AndroidDataDirProvider;

impl Default for AndroidDataDirProvider {
    fn default() -> Self {
        Self
    }
}

impl DataDirProvider for AndroidDataDirProvider {
    fn data_dir(&self) -> Option<PathBuf> {
        get_android_bridge().and_then(|bridge| bridge.data_dir())
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        get_android_bridge().and_then(|bridge| bridge.cache_dir())
    }
}

fn require_service_bridge(context: &str) -> Result<Arc<dyn AndroidBridge>> {
    get_android_bridge().ok_or_else(|| {
        MihomoError::Service(format!("Android bridge is not configured ({context})"))
    })
}

fn require_config_bridge(context: &str) -> Result<Arc<dyn AndroidBridge>> {
    get_android_bridge().ok_or_else(|| {
        MihomoError::Config(format!("Android bridge is not configured ({context})"))
    })
}
