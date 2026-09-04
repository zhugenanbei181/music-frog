//! ProfileStore implementation over the concrete ConfigManager.

use infiltrator_domain::profiles::{ProfileInfo, ProfileMetadata};
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::secure_store::SecureStore;
use std::path::PathBuf;

use crate::manager::ConfigManager;
use crate::profile::Profile;

#[async_trait::async_trait]
impl<S> ProfileStore for ConfigManager<S>
where
    S: SecureStore,
{
    fn config_dir(&self) -> PathBuf {
        ConfigManager::config_dir(self).to_path_buf()
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        ConfigManager::list_profiles(self)
            .await
            .map(|profiles| profiles.into_iter().map(profile_info).collect())
            .map_err(storage_error)
    }

    async fn get_current(&self) -> Result<String, PortError> {
        ConfigManager::get_current(self)
            .await
            .map_err(storage_error)
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
        ConfigManager::update_profile_metadata(self, profile, &concrete)
            .await
            .map_err(storage_error)
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
    }
}

fn storage_error<E: std::fmt::Display>(error: E) -> PortError {
    PortError::Io(error.to_string())
}
