//! Engine-scoped profile CRUD and per-profile metadata persistence.

use super::paths::profile_id;
use super::{
    ConfigManager, ensure_profile_reset_not_pending_for_dir,
    lock_reset_fenced_profile_mutation_for_dir,
};
use crate::profile::Profile;
use crate::profile_workspace_store::config_error;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{
    ProfileWorkspace, ProfileWorkspacePurpose, ProfileWorkspaceUpdate,
};
use infiltrator_ports::secure_store::SecureStore;
use infiltrator_profile_store::metadata::delete_subscription_url;
use lurker_core_contract::active_profile_reference::ActiveProfileReferenceState;
use lurker_core_ports::error::PortError;
use mihomo_driver::error::{MihomoError, Result};
use std::path::PathBuf;
use std::result;

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
        ensure_profile_reset_not_pending_for_dir(self.config_dir())
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
        self.save_inactive_profile_port(profile, content)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    pub(crate) async fn save_inactive_profile_port(
        &self,
        profile: &str,
        content: &str,
    ) -> result::Result<(), PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let selection_lease =
            ActiveProfileReferenceStore::acquire_selection_lease(self.profile_store.as_ref())
                .await?;
        let profile_id = profile_id(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.profile_store
            .save_inactive_profile_under_selection_lease(
                &profile_id,
                content.as_bytes(),
                selection_lease.as_ref(),
            )
            .await
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
        let reference = ProfileStore::read_active_profile_reference(self.profile_store.as_ref())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        match reference {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == identity.document.engine_id
                    && reference.profile_id == identity.document.profile_id =>
            {
                ProfileStore::compare_and_save_unowned_active_derived_document(
                    self.profile_store.as_ref(),
                    reference.clone(),
                    identity,
                    &update,
                )
                .await
                .map_err(|error| MihomoError::Config(error.to_string()))?;
            }
            _ => {
                ProfileStore::compare_and_save_inactive_workspace(
                    self.profile_store.as_ref(),
                    &identity,
                    &update,
                )
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
        let selection_lease =
            ActiveProfileReferenceStore::acquire_selection_lease(self.profile_store.as_ref())
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
        let _guard = lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile = profile_id(profile)?;
        self.profile_store
            .clear_profile_document_backup(&profile)
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    pub(crate) async fn list_profile_info(&self) -> result::Result<Vec<ProfileInfo>, PortError> {
        self.initialize_profile_namespace()
            .await
            .map_err(config_error)?;
        if !self.config_dir().exists() {
            return Ok(vec![]);
        }

        // Profile activity is product-wide and comes only from the typed
        // engine+profile reference. Keep its read lease while projecting the
        // list so a concurrent selection cannot make this view point at a
        // different document halfway through enumeration.
        let selection_lease =
            ActiveProfileReferenceStore::acquire_selection_lease(self.profile_store.as_ref())
                .await?;
        let _profile_guard = self.lock_profile_writes().await.map_err(config_error)?;
        self.profile_store
            .list_profile_info_locked(selection_lease.state())
            .await
    }

    pub async fn list_profiles(&self) -> Result<Vec<Profile>> {
        self.list_profile_info()
            .await
            .map(|profiles| profiles.into_iter().map(profile_from_info).collect())
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    pub async fn delete_profile(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        // Acquire the product selection fence before the engine write lock.
        // This prevents a profile becoming selected between the active check
        // and deletion, and preserves the root→engine lock order used by
        // active workspace writes.
        let selection_lease =
            ActiveProfileReferenceStore::acquire_selection_lease(self.profile_store.as_ref())
                .await
                .map_err(|error| MihomoError::Config(error.to_string()))?;
        let profile_id = profile_id(profile)?;
        self.profile_store
            .delete_profile_under_selection_lease(&profile_id, selection_lease.as_ref())
            .await
            .map_err(|error| MihomoError::Config(error.to_string()))
    }

    /// 只删除单个 profile 在 OS 凭证库中的订阅 URL（恢复出厂清理用）。
    /// 与 [`Self::delete_profile`] 不同：不删 YAML 文件、不动 settings
    /// 元数据。profile 名使用共享领域校验后再拼 engine-scoped subscription key。
    pub async fn delete_subscription_credential(&self, profile: &str) -> Result<()> {
        self.initialize_profile_namespace().await?;
        let _guard = lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
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
        let _guard = lock_reset_fenced_profile_mutation_for_dir(self.config_dir())
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

fn profile_from_info(info: ProfileInfo) -> Profile {
    let mut profile = Profile::new(info.name, PathBuf::from(info.path), info.active);
    apply_metadata(
        &mut profile,
        ProfileMetadata {
            subscription_url: info.subscription_url,
            auto_update_enabled: info.auto_update_enabled,
            update_interval_hours: info.update_interval_hours,
            last_updated: info.last_updated,
            next_update: info.next_update,
            traffic_upload: info.traffic_upload,
            traffic_download: info.traffic_download,
            traffic_total: info.traffic_total,
            expire_at: info.expire_at,
            user_agent: info.user_agent,
            etag: info.etag,
            last_modified: info.last_modified,
            cron_expression: info.cron_expression,
            insecure_skip_verify: info.insecure_skip_verify,
            auto_reload_core: info.auto_reload_core,
        },
    );
    profile.has_backup = info.has_backup;
    profile
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
