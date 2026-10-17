//! Product-wide active-profile reads and settings-file read/remove plumbing.

use super::ConfigManager;
use crate::yaml::validate;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use lurker_core_ports::error::PortError;
use mihomo_driver::error::{MihomoError, Result};
use std::path::PathBuf;
use tokio::fs;

impl<S: SecureStore> ConfigManager<S> {
    pub async fn get_current(&self) -> Result<String> {
        self.initialize_profile_namespace().await?;
        let lease =
            ActiveProfileReferenceStore::acquire_selection_lease(self.profile_store.as_ref())
                .await
                .map_err(active_reference_error)?;
        self.profile_store
            .get_current_under_selection_lease(lease.as_ref())
            .map_err(active_reference_error)
    }

    /// Read the legacy per-engine pointer only inside explicit migration and
    /// historical-file browsing code. It does not select a Core provider.
    pub(crate) async fn get_current_unlocked(&self) -> Result<String> {
        if !self.settings_file().exists() {
            return Ok("default".to_string());
        }

        let content = fs::read_to_string(self.settings_file()).await?;
        let config: toml::Value = toml::from_str(&content)
            .map_err(|e| MihomoError::Config(format!("Invalid config: {}", e)))?;

        Ok(config
            .get("default")
            .and_then(|d| d.get("profile"))
            .and_then(|p| p.as_str())
            .unwrap_or("default")
            .to_string())
    }

    pub async fn get_current_path(&self) -> Result<PathBuf> {
        let profile = self
            .profile_store
            .get_current()
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        Ok(self.config_dir().join(format!("{}.yaml", profile)))
    }

    /// Validate the product-wide active profile before any Core process is
    /// started. This follows the cross-engine reference through ProfileStore.
    pub async fn validate_current_profile(&self) -> Result<()> {
        let profile = self
            .profile_store
            .get_current()
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let content = self.load(&profile).await?;
        validate(&content)
    }
}

fn active_reference_error(error: PortError) -> MihomoError {
    match error {
        PortError::Rejected(failure) => MihomoError::Config(failure.message),
        PortError::Unsupported { reason, .. } => MihomoError::Config(reason),
        error => MihomoError::Config(error.to_string()),
    }
}
