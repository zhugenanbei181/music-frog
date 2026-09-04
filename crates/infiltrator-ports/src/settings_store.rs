//! Runtime-neutral application settings persistence port.

use async_trait::async_trait;
use infiltrator_domain::settings::AppSettings;

use crate::error::PortError;

#[async_trait]
pub trait SettingsStore: Send + Sync {
    async fn load(&self) -> Result<AppSettings, PortError>;
    async fn load_hydrated(&self) -> Result<AppSettings, PortError>;
    async fn save(&self, settings: &AppSettings) -> Result<(), PortError>;
}
