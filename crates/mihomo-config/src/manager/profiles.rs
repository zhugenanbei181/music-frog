//! Profile CRUD: load/save/list/delete profile YAML files and read/write
//! per-profile metadata.

use std::path::PathBuf;

use mihomo_api::error::{MihomoError, Result};
use mihomo_platform::traits::CredentialStore;
use tokio::fs;

use super::ConfigManager;
use super::metadata::{
    apply_profile_metadata, ensure_table, set_bool, set_optional_datetime, set_optional_string,
    set_optional_u32,
};
use super::paths::sanitized_profile_key;
use super::subscription_store::{delete_subscription_url, store_subscription_url};
use crate::profile::Profile;
use crate::yaml;

impl<S: CredentialStore> ConfigManager<S> {
    pub async fn load(&self, profile: &str) -> Result<String> {
        let path = self.existing_profile_yaml_path(profile).await?;
        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    pub async fn save(&self, profile: &str, content: &str) -> Result<()> {
        yaml::validate(content)?;
        let path = self.profile_yaml_path(profile).await?;
        fs::write(&path, content).await?;

        Ok(())
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        if !self.config_dir.exists() {
            return Ok(vec![]);
        }

        let current = self.get_current().await.ok();
        let settings = self.read_settings_value().await.ok();
        let metadata_table = settings
            .as_ref()
            .and_then(|value| value.get("profiles"))
            .and_then(|value| value.as_table());
        let mut profiles = vec![];

        let mut entries = fs::read_dir(&self.config_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let active = current.as_ref() == Some(&name);
                let mut profile = Profile::new(name.clone(), path, active);
                if let Some(table) = metadata_table.and_then(|table| table.get(&name))
                    && let Some(profile_table) = table.as_table()
                {
                    apply_profile_metadata(&self.credential_store, &mut profile, profile_table)
                        .await;
                }
                profiles.push(profile);
            }
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    pub async fn delete_profile(&self, profile: &str) -> Result<()> {
        let path = self.existing_profile_yaml_path(profile).await?;
        let current = self.get_current().await.ok();
        if current.as_ref() == Some(&profile.to_string()) {
            return Err(MihomoError::Config(
                "Cannot delete the active profile".to_string(),
            ));
        }

        fs::remove_file(path).await?;
        if let Err(err) = delete_subscription_url(&self.credential_store, profile).await {
            log::warn!("failed to delete subscription entry: {err}");
        }
        self.remove_profile_metadata(profile).await?;
        Ok(())
    }

    pub async fn get_profile_metadata(&self, profile: &str) -> Result<Profile> {
        let key = sanitized_profile_key(profile)?;
        let mut profile_info = Profile::new(profile.to_string(), PathBuf::new(), false);
        let settings = self.read_settings_value().await?;
        if let Some(table) = settings
            .get("profiles")
            .and_then(|value| value.as_table())
            .and_then(|table| table.get(&key))
            .and_then(|value| value.as_table())
        {
            apply_profile_metadata(&self.credential_store, &mut profile_info, table).await;
        }
        Ok(profile_info)
    }

    pub async fn update_profile_metadata(&self, profile: &str, metadata: &Profile) -> Result<()> {
        let key = sanitized_profile_key(profile)?;
        let mut settings = self.read_settings_value().await?;
        let root_table = ensure_table(&mut settings)?;
        let profiles_value = root_table
            .entry("profiles".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let profiles_table = ensure_table(profiles_value)?;
        let profile_value = profiles_table
            .entry(key.clone())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let profile_table = ensure_table(profile_value)?;

        let mut subscription_key = None;
        let subscription_fallback = metadata.subscription_url.clone();
        if let Some(url) = metadata.subscription_url.as_deref() {
            match store_subscription_url(&self.credential_store, &key, url).await {
                Ok(key) => {
                    subscription_key = Some(key);
                }
                Err(err) => {
                    log::warn!("failed to store subscription url securely: {err}");
                }
            }
        } else if let Err(err) = delete_subscription_url(&self.credential_store, &key).await {
            log::warn!("failed to delete subscription url: {err}");
        }
        set_optional_string(profile_table, "subscription_url_key", subscription_key);
        set_optional_string(profile_table, "subscription_url", subscription_fallback);
        set_bool(
            profile_table,
            "auto_update_enabled",
            metadata.auto_update_enabled,
        );
        set_optional_u32(
            profile_table,
            "update_interval_hours",
            metadata.update_interval_hours,
        );
        set_optional_datetime(profile_table, "last_updated", metadata.last_updated);
        set_optional_datetime(profile_table, "next_update", metadata.next_update);

        let content = toml::to_string(&settings)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&self.settings_file, content).await?;
        Ok(())
    }
}
