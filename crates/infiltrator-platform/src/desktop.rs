use crate::credentials::credential_store_override;
use crate::paths::get_home_dir;
use async_trait::async_trait;
use infiltrator_ports::data_dir::DataDirProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;
use std::result;
use tokio::task::spawn_blocking;

pub struct KeyringCredentialStore;

impl Default for KeyringCredentialStore {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl SecureStore for KeyringCredentialStore {
    async fn get(&self, service: &str, key: &str) -> result::Result<Option<String>, PortError> {
        if let Some(store) = credential_store_override() {
            return store.get(service, key).await;
        }
        let service = service.to_string();
        let key = key.to_string();
        spawn_blocking(move || -> result::Result<Option<String>, PortError> {
            let entry = match keyring::Entry::new(&service, &key) {
                Ok(entry) => entry,
                Err(err) => {
                    log::warn!("keyring init failed: {err}");
                    return Err(PortError::Failed(format!("Keyring init failed: {err}")));
                }
            };
            match entry.get_password() {
                Ok(value) => Ok(Some(value)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(err) => {
                    log::warn!("keyring get failed: {err}");
                    Err(PortError::Failed(format!("Keyring get failed: {err}")))
                }
            }
        })
        .await
        .map_err(|e| PortError::Failed(format!("Keyring task failed: {e}")))?
    }

    async fn set(&self, service: &str, key: &str, value: &str) -> result::Result<(), PortError> {
        if let Some(store) = credential_store_override() {
            return store.set(service, key, value).await;
        }
        let service = service.to_string();
        let key = key.to_string();
        let value = value.to_string();
        spawn_blocking(move || -> result::Result<(), PortError> {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|err| PortError::Failed(format!("Keyring init failed: {err}")))?;
            entry
                .set_password(&value)
                .map_err(|err| PortError::Failed(format!("Keyring set failed: {err}")))?;
            Ok(())
        })
        .await
        .map_err(|e| PortError::Failed(format!("Keyring task failed: {e}")))?
    }

    async fn delete(&self, service: &str, key: &str) -> result::Result<(), PortError> {
        if let Some(store) = credential_store_override() {
            return store.delete(service, key).await;
        }
        let service = service.to_string();
        let key = key.to_string();
        spawn_blocking(move || -> result::Result<(), PortError> {
            let entry = keyring::Entry::new(&service, &key)
                .map_err(|err| PortError::Failed(format!("Keyring init failed: {err}")))?;
            entry
                .delete_credential()
                .map_err(|err| PortError::Failed(format!("Keyring delete failed: {err}")))?;
            Ok(())
        })
        .await
        .map_err(|e| PortError::Failed(format!("Keyring task failed: {e}")))?
    }
}

pub struct DesktopDataDirProvider;

impl Default for DesktopDataDirProvider {
    fn default() -> Self {
        Self
    }
}

impl DataDirProvider for DesktopDataDirProvider {
    fn data_dir(&self) -> Option<PathBuf> {
        get_home_dir().ok()
    }
}
