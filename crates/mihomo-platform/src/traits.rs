use async_trait::async_trait;
use std::path::PathBuf;

use mihomo_api::Result;

#[async_trait]
pub trait CoreController: Send + Sync {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn is_running(&self) -> bool;
    fn controller_url(&self) -> Option<String>;

    async fn pid(&self) -> Option<u32> {
        None
    }
}

#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get(&self, service: &str, key: &str) -> Result<Option<String>>;
    async fn set(&self, service: &str, key: &str, value: &str) -> Result<()>;
    async fn delete(&self, service: &str, key: &str) -> Result<()>;
}

pub trait DataDirProvider: Send + Sync {
    fn data_dir(&self) -> Option<PathBuf>;
    fn cache_dir(&self) -> Option<PathBuf> {
        None
    }
}

#[cfg(not(target_os = "android"))]
pub type DefaultCredentialStore = super::KeyringCredentialStore;

#[cfg(target_os = "android")]
pub type DefaultCredentialStore = super::AndroidCredentialStore;

#[cfg(not(target_os = "android"))]
pub type DefaultDataDirProvider = super::DesktopDataDirProvider;

#[cfg(target_os = "android")]
pub type DefaultDataDirProvider = super::AndroidDataDirProvider;
