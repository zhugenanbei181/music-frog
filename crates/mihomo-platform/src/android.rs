use async_trait::async_trait;

use mihomo_api::{MihomoError, Result};

use crate::traits::{CoreController, CredentialStore, DataDirProvider};

pub struct AndroidCoreController;

#[async_trait]
impl CoreController for AndroidCoreController {
    async fn start(&self) -> Result<()> {
        Err(MihomoError::Service(
            "Android core controller is not implemented".to_string(),
        ))
    }

    async fn stop(&self) -> Result<()> {
        Err(MihomoError::Service(
            "Android core controller is not implemented".to_string(),
        ))
    }

    async fn is_running(&self) -> bool {
        false
    }

    fn controller_url(&self) -> Option<String> {
        None
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
    async fn get(&self, _service: &str, _key: &str) -> Result<Option<String>> {
        Err(MihomoError::Config(
            "Android credential store is not implemented".to_string(),
        ))
    }

    async fn set(&self, _service: &str, _key: &str, _value: &str) -> Result<()> {
        Err(MihomoError::Config(
            "Android credential store is not implemented".to_string(),
        ))
    }

    async fn delete(&self, _service: &str, _key: &str) -> Result<()> {
        Err(MihomoError::Config(
            "Android credential store is not implemented".to_string(),
        ))
    }
}

pub struct AndroidDataDirProvider;

impl Default for AndroidDataDirProvider {
    fn default() -> Self {
        Self
    }
}

impl DataDirProvider for AndroidDataDirProvider {
    fn data_dir(&self) -> Option<std::path::PathBuf> {
        None
    }
}
