use crate::android_bridge::{AndroidBridge, get_android_bridge};
use crate::credentials::credential_store_override;
use async_trait::async_trait;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_ports::error::PortError;
use std::path::PathBuf;
use std::result;
use std::sync::Arc;

pub struct AndroidCredentialStore;

impl Default for AndroidCredentialStore {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl SecureStore for AndroidCredentialStore {
    async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
        if let Some(store) = credential_store_override() {
            return store.get(service, key).await;
        }
        require_bridge("credential get")?
            .credential_get(service, key)
            .await
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        if let Some(store) = credential_store_override() {
            return store.set(service, key, value).await;
        }
        require_bridge("credential set")?
            .credential_set(service, key, value)
            .await
    }

    async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
        if let Some(store) = credential_store_override() {
            return store.delete(service, key).await;
        }
        require_bridge("credential delete")?
            .credential_delete(service, key)
            .await
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

fn require_bridge(context: &str) -> result::Result<Arc<dyn AndroidBridge>, PortError> {
    get_android_bridge()
        .ok_or_else(|| PortError::Failed(format!("Android bridge is not configured ({context})")))
}
