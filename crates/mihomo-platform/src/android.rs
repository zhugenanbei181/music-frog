use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;
use std::sync::Arc;

use crate::android_bridge::{AndroidBridge, get_android_bridge};

pub struct AndroidCoreController;

#[async_trait]
impl CoreProcess for AndroidCoreController {
    async fn start(&self) -> std::result::Result<(), PortError> {
        require_bridge("core start")?
            .core_start()
            .await
            .map_err(map_port_error)
    }

    async fn stop(&self) -> std::result::Result<(), PortError> {
        require_bridge("core stop")?
            .core_stop()
            .await
            .map_err(map_port_error)
    }

    async fn status(&self) -> std::result::Result<CoreLifecycle, PortError> {
        let running = require_bridge("core status")?
            .core_is_running()
            .await
            .map_err(map_port_error)?;
        Ok(if running {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }

    fn controller_endpoint(&self) -> Option<String> {
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
impl SecureStore for AndroidCredentialStore {
    async fn get(
        &self,
        service: &str,
        key: &str,
    ) -> std::result::Result<Option<String>, PortError> {
        require_bridge("credential get")?
            .credential_get(service, key)
            .await
            .map_err(map_port_error)
    }

    async fn set(
        &self,
        service: &str,
        key: &str,
        value: &str,
    ) -> std::result::Result<(), PortError> {
        require_bridge("credential set")?
            .credential_set(service, key, value)
            .await
            .map_err(map_port_error)
    }

    async fn delete(&self, service: &str, key: &str) -> std::result::Result<(), PortError> {
        require_bridge("credential delete")?
            .credential_delete(service, key)
            .await
            .map_err(map_port_error)
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

fn map_port_error(error: mihomo_api::error::MihomoError) -> PortError {
    match error {
        mihomo_api::error::MihomoError::Io(error) => PortError::Io(error.to_string()),
        mihomo_api::error::MihomoError::Http(error) => PortError::Network(error.to_string()),
        mihomo_api::error::MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        mihomo_api::error::MihomoError::NotFound(message) => PortError::NotFound(message),
        other => PortError::Failed(other.to_string()),
    }
}

fn require_bridge(context: &str) -> std::result::Result<Arc<dyn AndroidBridge>, PortError> {
    get_android_bridge()
        .ok_or_else(|| PortError::Failed(format!("Android bridge is not configured ({context})")))
}
