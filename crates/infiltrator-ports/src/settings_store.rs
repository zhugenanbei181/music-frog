//! Runtime-neutral application settings persistence port.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_domain::settings::AppSettings;

#[async_trait]
pub trait SettingsStore: Send + Sync {
    async fn load(&self) -> Result<AppSettings, PortError>;
    async fn load_hydrated(&self) -> Result<AppSettings, PortError>;
    async fn save(&self, settings: &AppSettings) -> Result<(), PortError>;
}
