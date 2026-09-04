//! Profile CRUD: load/save/list/delete profile YAML files and read/write
//! per-profile metadata.

use std::path::PathBuf;

use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::ConfigManager;
use super::metadata::{
    apply_profile_metadata, ensure_table, set_bool, set_optional_datetime, set_optional_i64,
    set_optional_string, set_optional_u32, set_optional_u64,
};
use super::paths::sanitized_profile_key;
use super::subscription_store::{delete_subscription_url, store_subscription_url};
use crate::profile::Profile;
use crate::yaml;

impl<S: SecureStore> ConfigManager<S> {
    pub async fn load(&self, profile: &str) -> Result<String> {
        let path = self.existing_profile_yaml_path(profile).await?;
        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    /// Read the transient pre-save copy, if one exists. Runtime apply flows
    /// use it to recover the actual previous content after another operation
    /// has already written a validated profile file.
    pub async fn load_backup(&self, profile: &str) -> Result<Option<String>> {
        let path = self.profile_yaml_path(profile).await?;
        match fs::read_to_string(backup_path(&path)).await {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn save(&self, profile: &str, content: &str) -> Result<()> {
        yaml::validate(content)?;
        let path = self.profile_yaml_path(profile).await?;
        if fs::try_exists(&path).await? {
            let previous = fs::read(&path).await?;
            atomic_write(&backup_path(&path), &previous).await?;
        } else {
            let _ = fs::remove_file(backup_path(&path)).await;
        }
        atomic_write(&path, content.as_bytes()).await?;

        Ok(())
    }

    /// Restore the last profile content saved through [`Self::save`]. The
    /// backup is deliberately one-shot and is cleared after the caller has
    /// successfully applied/rebuilt the new configuration.
    pub async fn restore_backup(&self, profile: &str) -> Result<bool> {
        let path = self.profile_yaml_path(profile).await?;
        let backup = backup_path(&path);
        if !fs::try_exists(&backup).await? {
            return Ok(false);
        }
        let previous = fs::read(&backup).await?;
        yaml::validate(std::str::from_utf8(&previous).map_err(|error| {
            MihomoError::Config(format!("profile backup is not UTF-8: {error}"))
        })?)?;
        atomic_write(&path, &previous).await?;
        Ok(true)
    }

    /// Remove the transient last-save backup once the new configuration is
    /// known to be live (or when a save did not require a runtime rebuild).
    pub async fn clear_backup(&self, profile: &str) -> Result<()> {
        let path = self.profile_yaml_path(profile).await?;
        match fs::remove_file(backup_path(&path)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
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

    /// 只删除单个 profile 在 OS 凭证库中的订阅 URL（恢复出厂清理用）。
    /// 与 [`Self::delete_profile`] 不同：不删 YAML 文件、不动 settings
    /// 元数据。名字先做与存储侧一致的消毒，再拼 `subscription:<key>`。
    pub async fn delete_subscription_credential(&self, profile: &str) -> Result<()> {
        let key = sanitized_profile_key(profile)?;
        delete_subscription_url(&self.credential_store, &key).await
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
        // The subscription URL embeds the account token, so it only stays
        // in the metadata file as a plaintext fallback when the secure
        // store is unavailable — otherwise it lives in the keyring alone.
        let mut subscription_fallback = metadata.subscription_url.clone();
        if let Some(url) = metadata.subscription_url.as_deref() {
            match store_subscription_url(&self.credential_store, &key, url).await {
                Ok(key) => {
                    subscription_key = Some(key);
                    subscription_fallback = None;
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
        set_optional_u64(profile_table, "traffic_upload", metadata.traffic_upload);
        set_optional_u64(profile_table, "traffic_download", metadata.traffic_download);
        set_optional_u64(profile_table, "traffic_total", metadata.traffic_total);
        set_optional_i64(profile_table, "expire_at", metadata.expire_at);

        let content = toml::to_string(&settings)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&self.settings_file, content).await?;
        Ok(())
    }
}

fn backup_path(path: &std::path::Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.yaml");
    path.with_file_name(format!("{file_name}.bak"))
}

async fn atomic_write(path: &std::path::Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        MihomoError::Config(format!("profile path has no parent: {}", path.display()))
    })?;
    fs::create_dir_all(parent).await?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temporary = parent.join(format!(".{file_name}.tmp-{}-{stamp}", std::process::id()));

    let result = async {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .await?;
        file.write_all(content).await?;
        file.sync_all().await?;
        drop(file);

        // `rename` is atomic on Unix. Windows refuses to replace an existing
        // file, so remove only the exact target as a platform fallback.
        #[cfg(windows)]
        if fs::try_exists(path).await? {
            fs::remove_file(path).await?;
        }
        fs::rename(&temporary, path).await?;
        Ok::<(), std::io::Error>(())
    }
    .await;

    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result.map_err(Into::into)
}
