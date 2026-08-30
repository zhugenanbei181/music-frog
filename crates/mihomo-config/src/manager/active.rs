//! Active-profile selection (`default.profile` in the settings file) and the
//! settings-file read/remove plumbing shared by the other manager seams.

use std::path::PathBuf;

use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::traits::CredentialStore;
use tokio::fs;

use super::ConfigManager;
use super::paths::sanitized_profile_key;

impl<S: CredentialStore> ConfigManager<S> {
    pub async fn set_current(&self, profile: &str) -> Result<()> {
        self.existing_profile_yaml_path(profile).await?;

        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut config = if self.settings_file.exists() {
            let content = fs::read_to_string(&self.settings_file).await?;
            toml::from_str(&content).unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()))
        } else {
            toml::Value::Table(toml::map::Map::new())
        };

        if let toml::Value::Table(ref mut table) = config {
            let default_table = table
                .entry("default".to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

            if let toml::Value::Table(default) = default_table {
                default.insert(
                    "profile".to_string(),
                    toml::Value::String(profile.to_string()),
                );
            }
        }

        let content = toml::to_string(&config)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&self.settings_file, content).await?;

        Ok(())
    }

    pub(super) async fn read_settings_value(&self) -> Result<toml::Value> {
        if !self.settings_file.exists() {
            return Ok(toml::Value::Table(toml::map::Map::new()));
        }
        let content = fs::read_to_string(&self.settings_file).await?;
        toml::from_str(&content).map_err(|e| MihomoError::Config(format!("Invalid config: {}", e)))
    }

    pub(super) async fn remove_profile_metadata(&self, profile: &str) -> Result<()> {
        let key = sanitized_profile_key(profile)?;
        if !self.settings_file.exists() {
            return Ok(());
        }
        let mut settings = self.read_settings_value().await?;
        if let toml::Value::Table(ref mut root) = settings
            && let Some(toml::Value::Table(profiles)) = root.get_mut("profiles")
        {
            profiles.remove(&key);
        }
        let content = toml::to_string(&settings)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        fs::write(&self.settings_file, content).await?;
        Ok(())
    }

    pub async fn get_current(&self) -> Result<String> {
        if !self.settings_file.exists() {
            return Ok("default".to_string());
        }

        let content = fs::read_to_string(&self.settings_file).await?;
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
        let profile = self.get_current().await?;
        Ok(self.config_dir.join(format!("{}.yaml", profile)))
    }
}
