//! Engine-scoped ProfileStore implementation over the codec-bound ConfigManager.

use crate::manager::ConfigManager;
use crate::profile::Profile;
use crate::profile_workspace_store::WorkspaceTarget;
use crate::sidecar_store::{read_optional, remove_optional, write_document};
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceCasOutcome, ActiveProfileReferenceState,
    ActiveProfileReferenceTarget, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::profile_document_identity::{ProfileId, ProfileWorkspaceIdentity};
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::active_profile_reference_store::ActiveProfileReferenceStore;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::{ActiveProfileWorkspaceWriteLease, ProfileStore};
use infiltrator_ports::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use infiltrator_ports::secure_store::SecureStore;
use std::fmt::Display;
use std::path;
use std::path::PathBuf;

#[cfg(test)]
#[path = "profile_store_test.rs"]
mod profile_store_test;

#[async_trait::async_trait]
impl<S> ProfileStore for ConfigManager<S>
where
    S: SecureStore,
{
    fn engine_id(&self) -> EngineId {
        ConfigManager::engine_id(self)
    }

    fn config_dir(&self) -> PathBuf {
        ConfigManager::config_dir(self).to_path_buf()
    }

    fn apply_transaction(&self, profile: &str) -> Option<ApplyTransactionSnapshot> {
        ConfigManager::apply_transaction(self, profile)
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        ConfigManager::list_profiles(self)
            .await
            .map(|profiles| profiles.into_iter().map(profile_info).collect())
            .map_err(storage_error)
    }

    async fn get_current(&self) -> Result<String, PortError> {
        let lease = ActiveProfileReferenceStore::acquire_selection_lease(self).await?;
        match lease.state() {
            ActiveProfileReferenceState::Unselected { .. } => Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                "no product-wide active profile is selected",
            )),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == ConfigManager::engine_id(self) =>
            {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Selected(reference) => Err(PortError::unsupported(
                infiltrator_contract::capability::Capability::Profiles,
                format!(
                    "the active profile belongs to engine {} instead of {}",
                    reference.engine_id.as_str(),
                    ConfigManager::engine_id(self).as_str()
                ),
            )),
        }
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        ActiveProfileReferenceStore::read(self).await
    }

    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let expected_revision = expected.revision();
        match ActiveProfileReferenceStore::compare_and_set(
            self,
            expected_revision,
            target,
            authorization,
        )
        .await
        {
            Ok(current) => Ok(ActiveProfileReferenceCasOutcome::Committed {
                previous: expected,
                current,
            }),
            Err(PortError::Rejected(failure))
                if failure.code == infiltrator_contract::error::ErrorCode::NotReady =>
            {
                let observed = ActiveProfileReferenceStore::read(self).await?;
                if observed.revision() == expected_revision {
                    Err(PortError::Rejected(failure))
                } else {
                    Ok(ActiveProfileReferenceCasOutcome::Stale { expected, observed })
                }
            }
            Err(error) => Err(error),
        }
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        ConfigManager::load(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        self.validate_document_bytes(content.as_bytes())?;
        ConfigManager::save_inactive_profile(self, profile, content)
            .await
            .map_err(storage_error)
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::delete_profile(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        ConfigManager::get_profile_metadata(self, profile)
            .await
            .map(profile_metadata)
            .map_err(storage_error)
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        let mut concrete = Profile::new(profile.to_string(), PathBuf::new(), false);
        concrete.subscription_url = metadata.subscription_url.clone();
        concrete.auto_update_enabled = metadata.auto_update_enabled;
        concrete.update_interval_hours = metadata.update_interval_hours;
        concrete.last_updated = metadata.last_updated;
        concrete.next_update = metadata.next_update;
        concrete.traffic_upload = metadata.traffic_upload;
        concrete.traffic_download = metadata.traffic_download;
        concrete.traffic_total = metadata.traffic_total;
        concrete.expire_at = metadata.expire_at;
        concrete.user_agent = metadata.user_agent.clone();
        concrete.etag = metadata.etag.clone();
        concrete.last_modified = metadata.last_modified.clone();
        concrete.cron_expression = metadata.cron_expression.clone();
        concrete.insecure_skip_verify = metadata.insecure_skip_verify;
        concrete.auto_reload_core = metadata.auto_reload_core;
        ConfigManager::update_profile_metadata(self, profile, &concrete)
            .await
            .map_err(storage_error)
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.read_workspace(profile).await
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        self.read_active_workspace().await
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        ConfigManager::acquire_active_workspace_write_lease(
            self,
            expected_reference,
            expected_workspace,
            authorization,
        )
        .await
    }

    async fn compare_and_save_unowned_active_derived_document(
        &self,
        expected_reference: ActiveProfileReference,
        expected_workspace: ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        ConfigManager::compare_and_save_unowned_active_derived_document(
            self,
            expected_reference,
            expected_workspace,
            update,
        )
        .await
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.save_workspace(expected, update, WorkspaceTarget::Inactive)
            .await
    }

    async fn restore_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        self.recover_workspace(expected, previous).await
    }

    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::delete_subscription_credential(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
        ensure_namespace(self).await?;
        let _guard = self.lock_profile_writes().await.map_err(storage_error)?;
        let path = aggregation_templates_path(ConfigManager::config_dir(self));
        let Some(text) = read_optional(&path).await? else {
            return Ok(Vec::new());
        };
        serde_yaml_ng::from_str(&text).map_err(storage_error)
    }

    async fn save_aggregation_templates(
        &self,
        templates: &[AggregationTemplate],
    ) -> Result<(), PortError> {
        ensure_namespace(self).await?;
        let _guard = crate::manager::lock_reset_fenced_profile_mutation_for_dir(
            ConfigManager::config_dir(self),
        )
        .await?;
        let path = aggregation_templates_path(ConfigManager::config_dir(self));
        if templates.is_empty() {
            return remove_optional(&path).await;
        }
        let text = serde_yaml_ng::to_string(templates).map_err(storage_error)?;
        write_document(&path, &text).await
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::clear_backup(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        ConfigManager::restore_inactive_profile_backup(self, profile)
            .await
            .map_err(storage_error)
    }
}

async fn ensure_namespace<S: SecureStore>(manager: &ConfigManager<S>) -> Result<(), PortError> {
    manager
        .initialize_profile_namespace()
        .await
        .map_err(storage_error)
}

fn profile_info(profile: Profile) -> ProfileInfo {
    ProfileInfo {
        name: profile.name,
        active: profile.active,
        path: profile.path.to_string_lossy().to_string(),
        controller_url: None,
        controller_changed: None,
        subscription_url: profile.subscription_url,
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
        has_backup: profile.has_backup,
    }
}

fn profile_metadata(profile: Profile) -> ProfileMetadata {
    ProfileMetadata {
        subscription_url: profile.subscription_url,
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated,
        next_update: profile.next_update,
        traffic_upload: profile.traffic_upload,
        traffic_download: profile.traffic_download,
        traffic_total: profile.traffic_total,
        expire_at: profile.expire_at,
        user_agent: profile.user_agent,
        etag: profile.etag,
        last_modified: profile.last_modified,
        cron_expression: profile.cron_expression,
        insecure_skip_verify: profile.insecure_skip_verify,
        auto_reload_core: profile.auto_reload_core,
    }
}

fn storage_error<E: Display>(error: E) -> PortError {
    PortError::Io(error.to_string())
}

/// DUAL-08-13: aggregation-template library sidecar. The leading dot keeps it
/// out of the profile-name space (`options/<profile>.yaml`).
fn aggregation_templates_path(config_dir: &path::Path) -> PathBuf {
    config_dir
        .join("options")
        .join(".aggregation-templates.yaml")
}
