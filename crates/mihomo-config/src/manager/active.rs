//! Product-wide active-profile reads and settings-file read/remove plumbing.

use super::ConfigManager;
use super::paths::profile_id;
use crate::yaml::validate;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use std::path::PathBuf;
use tokio::fs;

impl<S: SecureStore> ConfigManager<S> {
    pub(super) async fn remove_profile_metadata(&self, profile: &str) -> Result<()> {
        let key = profile_id(profile)?;
        self.profile_store
            .remove_profile_metadata(key.as_str())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        Ok(())
    }

    pub async fn get_current(&self) -> Result<String> {
        self.initialize_profile_namespace().await?;
        let lease = ActiveProfileReferenceStore::acquire_selection_lease(self)
            .await
            .map_err(active_reference_error)?;
        match lease.state() {
            ActiveProfileReferenceState::Unselected { .. } => Err(MihomoError::Config(
                "no product-wide active profile is selected".to_owned(),
            )),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id() =>
            {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Selected(reference) => Err(MihomoError::Config(format!(
                "active profile belongs to engine {} instead of {}",
                reference.engine_id.as_str(),
                self.engine_id().as_str()
            ))),
        }
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
        let profile = ProfileStore::get_current(self)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        Ok(self.config_dir().join(format!("{}.yaml", profile)))
    }

    /// Validate the product-wide active profile before any Core process is
    /// started. This follows the cross-engine reference through ProfileStore.
    pub async fn validate_current_profile(&self) -> Result<()> {
        let profile = ProfileStore::get_current(self)
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
