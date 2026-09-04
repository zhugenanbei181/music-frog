//! Runtime-neutral profile/configuration persistence port.

use crate::error::PortError;
use async_trait::async_trait;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use std::path::PathBuf;

/// Profile persistence operations needed by application/inbound surfaces.
///
/// The implementation may use a filesystem, database, or platform sync store;
/// callers receive only domain values and owned strings. No `ConfigManager`,
/// keyring implementation, Tokio channel, or controller type crosses this
/// boundary.
#[async_trait]
pub trait ProfileStore: Send + Sync {
    fn config_dir(&self) -> PathBuf;
    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError>;
    async fn get_current(&self) -> Result<String, PortError>;
    async fn set_current(&self, profile: &str) -> Result<(), PortError>;
    async fn load(&self, profile: &str) -> Result<String, PortError>;
    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError>;
    async fn delete_profile(&self, profile: &str) -> Result<(), PortError>;
    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError>;
    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError>;
    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError>;
    async fn delete_options(&self, profile: &str) -> Result<(), PortError>;
    async fn clear_backup(&self, profile: &str) -> Result<(), PortError>;
    async fn restore_backup(&self, profile: &str) -> Result<bool, PortError>;
}
