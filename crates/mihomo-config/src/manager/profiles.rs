//! Profile CRUD: load/save/list/delete profile YAML files and read/write
//! per-profile metadata.

use super::ConfigManager;
use super::metadata::{
    apply_profile_metadata, ensure_table, set_bool, set_optional_datetime, set_optional_i64,
    set_optional_string, set_optional_u32, set_optional_u64,
};
use super::paths::sanitized_profile_key;
use super::subscription_store::{delete_subscription_url, store_subscription_url};
use crate::profile::Profile;
use crate::sidecar_store::{remove_optional, write_document_bytes_durable};
use crate::yaml;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_domain::profile_options::options_path;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::error::{MihomoError, Result};
use std::io::ErrorKind;
use std::path;
use std::path::PathBuf;
use std::str::from_utf8;
use tokio::fs;
use toml::map::Map;

impl<S: SecureStore> ConfigManager<S> {
    pub async fn load(&self, profile: &str) -> Result<String> {
        self.initialize_profile_namespace().await?;
        let _guard = self.lock_profile_writes().await?;
        let path = self.existing_profile_yaml_path(profile).await?;
        let content = fs::read_to_string(&path).await?;
        Ok(content)
    }

    /// Read the transient pre-save copy, if one exists. Runtime apply flows
    /// use it to recover the actual previous content after another operation
    /// has already written a validated profile file.
    pub async fn load_backup(&self, profile: &str) -> Result<Option<String>> {
        self.initialize_profile_namespace().await?;
        let _guard = self.lock_profile_writes().await?;
        crate::manager::ensure_profile_reset_not_pending_for_dir(&self.config_dir)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let path = self.profile_yaml_path(profile).await?;
        match fs::read_to_string(backup_path(&path)).await {
            Ok(content) => Ok(Some(content)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Save a profile only while it is not selected by the product-wide
    /// reference. The reference lease is held before the cross-process
    /// profile-write lock, so a concurrent selection cannot turn this into an
    /// unowned active-profile write.
    pub async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let selection_lease = ActiveProfileReferenceStore::acquire_selection_lease(self)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile_id = ProfileId::new(profile.to_owned())
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        if matches!(
            selection_lease.state(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id == profile_id
        ) {
            return Err(MihomoError::Config(
                "Cannot write the active profile through an inactive-profile operation".into(),
            ));
        }
        let _guard = self.lock_profile_writes().await?;
        self.save_profile_locked(profile, content).await
    }

    pub(crate) async fn save_profile_locked(&self, profile: &str, content: &str) -> Result<()> {
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

    /// Publish an engine-derived document update against the complete
    /// workspace identity. If it is active, the write requires an unowned
    /// Core and an exact reference lease; otherwise it uses the inactive
    /// target fence.
    pub(crate) async fn persist_derived_workspace_content(
        &self,
        workspace: &ProfileWorkspace,
        content: &str,
    ) -> Result<()> {
        let identity = workspace.identity();
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: content.as_bytes().to_vec(),
            sidecar_bytes: workspace.sidecar_bytes.clone(),
        };
        let reference = ProfileStore::read_active_profile_reference(self)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        match reference {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == identity.document.engine_id
                    && reference.profile_id == identity.document.profile_id =>
            {
                ProfileStore::compare_and_save_unowned_active_derived_document(
                    self,
                    reference.clone(),
                    identity,
                    &update,
                )
                .await
                .map_err(|error| MihomoError::Config(error.to_string()))?;
            }
            _ => {
                ProfileStore::compare_and_save_inactive_workspace(self, &identity, &update)
                    .await
                    .map_err(|error| MihomoError::Config(error.to_string()))?;
            }
        }
        Ok(())
    }

    /// Restore a profile backup only while the profile is inactive under the
    /// product-wide selection fence. Active recovery must use the managed
    /// workspace transaction lease.
    pub async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool> {
        self.initialize_profile_namespace().await?;
        let selection_lease = ActiveProfileReferenceStore::acquire_selection_lease(self)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile_id = ProfileId::new(profile.to_owned())
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        if matches!(
            selection_lease.state(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id == profile_id
        ) {
            return Err(MihomoError::Config(
                "Cannot restore an active profile backup through an inactive-profile operation"
                    .into(),
            ));
        }
        let _guard = self.lock_profile_writes().await?;
        let path = self.profile_yaml_path(profile).await?;
        let backup = backup_path(&path);
        if !fs::try_exists(&backup).await? {
            return Ok(false);
        }
        let previous = fs::read(&backup).await?;
        yaml::validate(from_utf8(&previous).map_err(|error| {
            MihomoError::Config(format!("profile backup is not UTF-8: {error}"))
        })?)?;
        atomic_write(&path, &previous).await?;
        Ok(true)
    }

    /// Remove the transient last-save backup once the new configuration is
    /// known to be live (or when a save did not require a runtime rebuild).
    pub async fn clear_backup(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(&self.config_dir)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let path = self.profile_yaml_path(profile).await?;
        match fs::remove_file(backup_path(&path)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.initialize_profile_namespace().await?;
        if !self.config_dir.exists() {
            return Ok(vec![]);
        }

        // Profile activity is product-wide and comes only from the typed
        // engine+profile reference. Keep its read lease while projecting the
        // list so a concurrent selection cannot make this view point at a
        // different document halfway through enumeration.
        let selection_lease = ActiveProfileReferenceStore::acquire_selection_lease(self)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let _profile_guard = self.lock_profile_writes().await?;
        let active_profile = match selection_lease.state() {
            ActiveProfileReferenceState::Unselected { .. } => None,
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id() =>
            {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Selected(_) => None,
        };
        let settings = self.read_settings_value().await.ok();
        let metadata_table = settings
            .as_ref()
            .and_then(|value| value.get("profiles"))
            .and_then(|value| value.as_table());
        let engine = self.engine_id();
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
                let active = active_profile.as_ref() == Some(&name);
                let mut profile = Profile::new(name.clone(), path, active);
                if let Some(table) = metadata_table.and_then(|table| table.get(&name))
                    && let Some(profile_table) = table.as_table()
                {
                    apply_profile_metadata(
                        &self.credential_store,
                        &engine,
                        &mut profile,
                        profile_table,
                    )
                    .await;
                }
                profile.has_backup = fs::try_exists(backup_path(&profile.path))
                    .await
                    .unwrap_or(false);
                profiles.push(profile);
            }
        }

        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(profiles)
    }

    pub async fn delete_profile(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        // Acquire the product selection fence before the engine write lock.
        // This prevents a profile becoming selected between the active check
        // and deletion, and preserves the root→engine lock order used by
        // active workspace writes.
        let selection_lease = ActiveProfileReferenceStore::acquire_selection_lease(self)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile_id = ProfileId::new(profile.to_owned())
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        if matches!(
            selection_lease.state(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id == profile_id
        ) {
            return Err(MihomoError::Config(
                "Cannot delete the active profile".to_string(),
            ));
        }
        let _guard = self.lock_profile_writes().await?;
        let path = self.existing_profile_yaml_path(profile).await?;

        let previous_document = fs::read(&path).await?;
        fs::remove_file(&path).await?;
        let profile_key = sanitized_profile_key(profile)?;
        if let Err(error) = remove_optional(&options_path(self.config_dir(), &profile_key)).await {
            if let Err(rollback) = atomic_write(&path, &previous_document).await {
                return Err(MihomoError::Config(format!(
                    "profile deletion sidecar cleanup failed ({error}); profile restore failed ({rollback})"
                )));
            }
            return Err(MihomoError::Config(error.to_string()));
        }
        if let Err(error) = remove_optional(&backup_path(&path)).await {
            log::warn!("deleted profile backup cleanup failed: {error}");
        }
        self.apply_observations
            .write()
            .expect("apply observations")
            .remove(profile);
        if let Err(err) =
            delete_subscription_url(&self.credential_store, &self.engine_id(), profile).await
        {
            log::warn!("failed to delete subscription entry: {err}");
        }
        self.remove_profile_metadata(profile).await?;
        Ok(())
    }

    /// 只删除单个 profile 在 OS 凭证库中的订阅 URL（恢复出厂清理用）。
    /// 与 [`Self::delete_profile`] 不同：不删 YAML 文件、不动 settings
    /// 元数据。名字先做与存储侧一致的消毒，再拼 engine-scoped subscription key。
    pub async fn delete_subscription_credential(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(&self.config_dir)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let key = sanitized_profile_key(profile)?;
        delete_subscription_url(&self.credential_store, &self.engine_id(), &key).await
    }

    pub async fn get_profile_metadata(&self, profile: &str) -> Result<Profile> {
        self.initialize_profile_namespace().await?;
        let _guard = self.lock_profile_writes().await?;
        self.get_profile_metadata_unlocked(profile).await
    }

    /// Read profile metadata after namespace initialization. Workspace reads
    /// call this while holding the shared profile write boundary.
    pub(crate) async fn get_profile_metadata_unlocked(&self, profile: &str) -> Result<Profile> {
        let key = sanitized_profile_key(profile)?;
        let mut profile_info = Profile::new(profile.to_string(), PathBuf::new(), false);
        let settings = self.read_settings_value().await?;
        if let Some(table) = settings
            .get("profiles")
            .and_then(|value| value.as_table())
            .and_then(|table| table.get(&key))
            .and_then(|value| value.as_table())
        {
            apply_profile_metadata(
                &self.credential_store,
                &self.engine_id(),
                &mut profile_info,
                table,
            )
            .await;
        }
        Ok(profile_info)
    }

    pub async fn update_profile_metadata(&self, profile: &str, metadata: &Profile) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(&self.config_dir)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let key = sanitized_profile_key(profile)?;
        let mut settings = self.read_settings_value().await?;
        let root_table = ensure_table(&mut settings)?;
        let profiles_value = root_table
            .entry("profiles".to_string())
            .or_insert_with(|| toml::Value::Table(Map::new()));
        let profiles_table = ensure_table(profiles_value)?;
        let profile_value = profiles_table
            .entry(key.clone())
            .or_insert_with(|| toml::Value::Table(Map::new()));
        let profile_table = ensure_table(profile_value)?;

        let mut subscription_key = None;
        // The subscription URL embeds the account token, so it only stays
        // in the metadata file as a plaintext fallback when the secure
        // store is unavailable — otherwise it lives in the keyring alone.
        let mut subscription_fallback = metadata.subscription_url.clone();
        if let Some(url) = metadata.subscription_url.as_deref() {
            match store_subscription_url(&self.credential_store, &self.engine_id(), &key, url).await
            {
                Ok(key) => {
                    subscription_key = Some(key);
                    subscription_fallback = None;
                }
                Err(err) => {
                    log::warn!("failed to store subscription url securely: {err}");
                }
            }
        } else if let Err(err) =
            delete_subscription_url(&self.credential_store, &self.engine_id(), &key).await
        {
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
        set_optional_string(profile_table, "user_agent", metadata.user_agent.clone());
        set_optional_string(profile_table, "etag", metadata.etag.clone());
        set_optional_string(
            profile_table,
            "last_modified",
            metadata.last_modified.clone(),
        );
        set_optional_string(
            profile_table,
            "cron_expression",
            metadata.cron_expression.clone(),
        );
        set_bool(
            profile_table,
            "insecure_skip_verify",
            metadata.insecure_skip_verify,
        );
        set_bool(profile_table, "auto_reload_core", metadata.auto_reload_core);

        let content = toml::to_string(&settings)
            .map_err(|e| MihomoError::Config(format!("Failed to serialize config: {}", e)))?;
        if let Some(parent) = self.settings_file.parent() {
            fs::create_dir_all(parent).await?;
        }
        write_document_bytes_durable(&self.settings_file, content.as_bytes())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        Ok(())
    }
}

pub(crate) fn backup_path(path: &path::Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("profile.yaml");
    path.with_file_name(format!("{file_name}.bak"))
}

async fn atomic_write(path: &path::Path, content: &[u8]) -> Result<()> {
    write_document_bytes_durable(path, content)
        .await
        .map_err(|error| MihomoError::Config(error.to_string()))
}
