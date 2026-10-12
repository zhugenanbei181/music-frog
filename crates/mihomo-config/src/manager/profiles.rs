//! Engine-scoped profile CRUD and per-profile metadata persistence.

use super::ConfigManager;
use super::paths::profile_id;
use crate::profile::Profile;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_domain::profile_options::options_path;
use infiltrator_domain::profiles::ProfileMetadata;
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::document_store::{remove_optional, write_document_bytes_durable};
use infiltrator_profile_store::metadata::{delete_subscription_url, read_profile_metadata};
use infiltrator_profile_store::profile_files::profile_document_backup_path;
use mihomo_api::error::{MihomoError, Result};
use std::path;
use std::path::PathBuf;
use tokio::fs;

impl<S: SecureStore> ConfigManager<S> {
    pub async fn load(&self, profile: &str) -> Result<String> {
        self.initialize_profile_namespace().await?;
        let _guard = self.lock_profile_writes().await?;
        let profile = profile_id(profile)?;
        self.profile_store
            .read_profile_document(&profile)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    /// Read the transient pre-save copy, if one exists. Runtime apply flows
    /// use it to recover the actual previous content after another operation
    /// has already written a validated profile file.
    pub async fn load_backup(&self, profile: &str) -> Result<Option<String>> {
        self.initialize_profile_namespace().await?;
        let _guard = self.lock_profile_writes().await?;
        crate::manager::ensure_profile_reset_not_pending_for_dir(self.config_dir())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile = profile_id(profile)?;
        self.profile_store
            .read_profile_document_backup(&profile)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
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
        let profile_id = profile_id(profile)?;
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
        let profile = profile_id(profile)?;
        self.profile_store
            .write_profile_document_locked(&profile, content.as_bytes())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
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
        let profile_id = profile_id(profile)?;
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
        self.profile_store
            .restore_profile_document_backup_locked(&profile_id)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    /// Remove the transient last-save backup once the new configuration is
    /// known to be live (or when a save did not require a runtime rebuild).
    pub async fn clear_backup(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile = profile_id(profile)?;
        self.profile_store
            .clear_profile_document_backup(&profile)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.initialize_profile_namespace().await?;
        if !self.config_dir().exists() {
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

        let mut entries = fs::read_dir(self.config_dir()).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some(self.document_extension().as_str())
            {
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
                    let metadata = read_profile_metadata(
                        self.credential_store(),
                        &engine,
                        &name,
                        profile_table,
                    )
                    .await;
                    apply_metadata(&mut profile, metadata);
                }
                profile.has_backup = match profile_id(&name) {
                    Ok(profile_id) => self
                        .profile_store
                        .profile_document_has_backup(&profile_id)
                        .await
                        .unwrap_or(false),
                    Err(_) => false,
                };
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
        let profile_id = profile_id(profile)?;
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
        let path = self.existing_profile_document_path(profile).await?;

        let previous_document = fs::read(&path).await?;
        fs::remove_file(&path).await?;
        let profile_key = profile_id.as_str();
        if let Err(error) = remove_optional(&options_path(self.config_dir(), profile_key)).await {
            if let Err(rollback) = atomic_write(&path, &previous_document).await {
                return Err(MihomoError::Config(format!(
                    "profile deletion sidecar cleanup failed ({error}); profile restore failed ({rollback})"
                )));
            }
            return Err(MihomoError::Config(error.to_string()));
        }
        if let Err(error) = remove_optional(&profile_document_backup_path(&path)).await {
            log::warn!("deleted profile backup cleanup failed: {error}");
        }
        self.profile_store.remove_apply_transaction(profile);
        if let Err(err) = delete_subscription_url(
            self.credential_store(),
            &self.engine_id(),
            profile_id.as_str(),
        )
        .await
        {
            log::warn!("failed to delete subscription entry: {err}");
        }
        self.remove_profile_metadata(profile).await?;
        Ok(())
    }

    /// 只删除单个 profile 在 OS 凭证库中的订阅 URL（恢复出厂清理用）。
    /// 与 [`Self::delete_profile`] 不同：不删 YAML 文件、不动 settings
    /// 元数据。profile 名使用共享领域校验后再拼 engine-scoped subscription key。
    pub async fn delete_subscription_credential(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let key = profile_id(profile)?;
        delete_subscription_url(self.credential_store(), &self.engine_id(), key.as_str())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    pub async fn get_profile_metadata(&self, profile: &str) -> Result<Profile> {
        self.initialize_profile_namespace().await?;
        let _guard = self.lock_profile_writes().await?;
        self.get_profile_metadata_unlocked(profile).await
    }

    /// Read profile metadata after namespace initialization. Workspace reads
    /// call this while holding the shared profile write boundary.
    pub(crate) async fn get_profile_metadata_unlocked(&self, profile: &str) -> Result<Profile> {
        let key = profile_id(profile)?;
        let metadata = self
            .profile_store
            .get_profile_metadata(key.as_str())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        Ok(profile_from_metadata(profile, metadata))
    }

    pub async fn update_profile_metadata(&self, profile: &str, metadata: &Profile) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let key = profile_id(profile)?;
        let metadata = metadata_from_profile(metadata);
        self.profile_store
            .update_profile_metadata(key.as_str(), &metadata)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        Ok(())
    }
}

fn apply_metadata(profile: &mut Profile, metadata: ProfileMetadata) {
    profile.subscription_url = metadata.subscription_url;
    profile.auto_update_enabled = metadata.auto_update_enabled;
    profile.update_interval_hours = metadata.update_interval_hours;
    profile.last_updated = metadata.last_updated;
    profile.next_update = metadata.next_update;
    profile.traffic_upload = metadata.traffic_upload;
    profile.traffic_download = metadata.traffic_download;
    profile.traffic_total = metadata.traffic_total;
    profile.expire_at = metadata.expire_at;
    profile.user_agent = metadata.user_agent;
    profile.etag = metadata.etag;
    profile.last_modified = metadata.last_modified;
    profile.cron_expression = metadata.cron_expression;
    profile.insecure_skip_verify = metadata.insecure_skip_verify;
    profile.auto_reload_core = metadata.auto_reload_core;
}

fn profile_from_metadata(name: &str, metadata: ProfileMetadata) -> Profile {
    let mut profile = Profile::new(name.to_owned(), PathBuf::new(), false);
    apply_metadata(&mut profile, metadata);
    profile
}

fn metadata_from_profile(profile: &Profile) -> ProfileMetadata {
    ProfileMetadata {
        subscription_url: profile.subscription_url.clone(),
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated,
        next_update: profile.next_update,
        traffic_upload: profile.traffic_upload,
        traffic_download: profile.traffic_download,
        traffic_total: profile.traffic_total,
        expire_at: profile.expire_at,
        user_agent: profile.user_agent.clone(),
        etag: profile.etag.clone(),
        last_modified: profile.last_modified.clone(),
        cron_expression: profile.cron_expression.clone(),
        insecure_skip_verify: profile.insecure_skip_verify,
        auto_reload_core: profile.auto_reload_core,
    }
}

async fn atomic_write(path: &path::Path, content: &[u8]) -> Result<()> {
    write_document_bytes_durable(path, content)
        .await
        .map_err(|error| MihomoError::Config(error.to_string()))
}
