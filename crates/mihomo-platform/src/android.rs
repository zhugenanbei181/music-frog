use crate::android_bridge::{AndroidBridge, get_android_bridge};
use async_trait::async_trait;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_ports::core_process::CoreProcess;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::MihomoError;
use std::path::PathBuf;
use std::result;
use std::sync::Arc;

pub struct AndroidCoreController;

#[async_trait]
impl CoreProcess for AndroidCoreController {
    async fn start(&self) -> result::Result<(), PortError> {
        require_bridge("core start")?
            .core_start()
            .await
            .map_err(map_port_error)
    }

    async fn stop(&self) -> result::Result<(), PortError> {
        require_bridge("core stop")?
            .core_stop()
            .await
            .map_err(map_port_error)
    }

    async fn status(&self) -> result::Result<CoreLifecycle, PortError> {
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
    async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
        require_bridge("credential get")?
            .credential_get(service, key)
            .await
            .map_err(map_port_error)
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        require_bridge("credential set")?
            .credential_set(service, key, value)
            .await
            .map_err(map_port_error)
    }

    async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
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

fn map_port_error(error: MihomoError) -> PortError {
    match error {
        MihomoError::Io(error) => PortError::Io(error.to_string()),
        MihomoError::Http(error) => PortError::Network(error.to_string()),
        MihomoError::WebSocket(error) => PortError::Network(error.to_string()),
        MihomoError::NotFound(message) => PortError::NotFound(message),
        other => PortError::Failed(other.to_string()),
    }
}

fn require_bridge(context: &str) -> result::Result<Arc<dyn AndroidBridge>, PortError> {
    get_android_bridge()
        .ok_or_else(|| PortError::Failed(format!("Android bridge is not configured ({context})")))
}
