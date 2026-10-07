//! ProfileStore implementation over the concrete ConfigManager.

use crate::manager::ConfigManager;
use crate::profile::Profile;
use crate::profile_option_store;
use crate::profile_workspace_store::{WorkspaceTarget, config_error};
use crate::sidecar_store::{read_optional, remove_optional, write_document};
use infiltrator_contract::aggregator::AggregationTemplate;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_domain::profile_options::ProfileOptions;
use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
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
        ConfigManager::get_current(self).await.map_err(config_error)
    }

    async fn set_current(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::set_current(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        ConfigManager::load(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn save(&self, profile: &str, content: &str) -> Result<(), PortError> {
        ConfigManager::save(self, profile, content)
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

    async fn delete_options(&self, profile: &str) -> Result<(), PortError> {
        profile_option_store::delete_options(self.config_dir(), profile).await
    }

    async fn load_workspace(&self, profile: &str) -> Result<ProfileWorkspace, PortError> {
        self.read_workspace(profile).await
    }
    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        self.read_active_workspace().await
    }

    async fn compare_and_save_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.save_workspace(expected, update, WorkspaceTarget::Any)
            .await
    }

    async fn compare_and_save_active_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.save_workspace(expected, update, WorkspaceTarget::Active)
            .await
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.save_workspace(expected, update, WorkspaceTarget::Inactive)
            .await
    }

    async fn restore_workspace(
        &self,
        expected: &ProfileSourceIdentity,
        previous: &ProfileWorkspace,
    ) -> Result<ProfileWorkspace, PortError> {
        self.recover_workspace(expected, previous).await
    }

    async fn load_options(&self, profile: &str) -> Result<ProfileOptions, PortError> {
        profile_option_store::load_options(self.config_dir(), profile).await
    }

    async fn save_options(&self, profile: &str, options: &ProfileOptions) -> Result<(), PortError> {
        profile_option_store::save_options(self.config_dir(), profile, options).await
    }

    async fn delete_subscription_credential(&self, profile: &str) -> Result<(), PortError> {
        ConfigManager::delete_subscription_credential(self, profile)
            .await
            .map_err(storage_error)
    }

    async fn load_aggregation_templates(&self) -> Result<Vec<AggregationTemplate>, PortError> {
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

    async fn restore_backup(&self, profile: &str) -> Result<bool, PortError> {
        ConfigManager::restore_backup(self, profile)
            .await
            .map_err(storage_error)
    }
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
