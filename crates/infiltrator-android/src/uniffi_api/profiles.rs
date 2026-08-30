//! Profile operations: listing, creating, selecting, updating, saving and
//! deleting profiles, plus subscription metadata management. Selection and
//! save go through the session apply transaction in [`super::session`].

use chrono::{Duration as ChronoDuration, Utc};

use infiltrator_core::profiles::{
    ProfileDetail as CoreProfileDetail, ProfileInfo, create_profile_from_url, list_profile_infos,
    load_profile_detail, sanitize_profile_name, select_profile as core_select_profile,
    update_profile as core_update_profile,
};
use infiltrator_core::{config as core_config, profiles as core_profiles};
use mihomo_config::manager::ConfigManager;

use super::session::apply_current_profile_status;
use super::support::{get_runtime, map_anyhow_error, map_mihomo_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

// --- Profiles API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileSummary {
    pub name: String,
    pub active: bool,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<String>,
    pub next_update: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfilesResult {
    pub status: FfiStatus,
    pub profiles: Vec<ProfileSummary>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileDetail {
    pub name: String,
    pub active: bool,
    pub content: String,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<String>,
    pub next_update: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileDetailResult {
    pub status: FfiStatus,
    pub profile: Option<ProfileDetail>,
}

#[uniffi::export]
pub async fn profiles_list() -> ProfilesResult {
    get_runtime()
        .spawn(async move {
            match list_profile_infos().await.map_err(map_anyhow_error) {
                Ok(profiles) => ProfilesResult {
                    status: FfiStatus::ok(),
                    profiles: profiles.into_iter().map(profile_to_summary).collect(),
                },
                Err(status) => ProfilesResult {
                    status,
                    profiles: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| ProfilesResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            profiles: Vec::new(),
        })
}

#[uniffi::export]
pub async fn profile_create(name: String, url: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match create_profile_from_url(&name, &url).await {
                Ok(_) => FfiStatus::ok(),
                Err(err) => map_anyhow_error(err),
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_select(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let manager = match ConfigManager::new() {
                Ok(manager) => manager,
                Err(err) => return map_mihomo_error(err),
            };
            let previous = manager.get_current().await.ok();
            match core_select_profile(&name).await {
                // Apply the newly current profile through the session
                // transaction; on rollback the switch above is undone.
                Ok(_) => apply_current_profile_status(previous).await,
                Err(err) => map_anyhow_error(err),
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_update(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let manager = match ConfigManager::new() {
                Ok(manager) => manager,
                Err(err) => return map_mihomo_error(err),
            };
            let previous = manager.get_current().await.ok();
            match core_update_profile(&name).await {
                Ok(profile) => {
                    if profile.active {
                        apply_current_profile_status(previous).await
                    } else {
                        FfiStatus::ok()
                    }
                }
                Err(err) => map_anyhow_error(err),
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_detail(name: String) -> ProfileDetailResult {
    get_runtime()
        .spawn(async move {
            match load_profile_detail(&name).await {
                Ok(profile) => ProfileDetailResult {
                    status: FfiStatus::ok(),
                    profile: Some(profile_detail_to_record(profile)),
                },
                Err(err) => ProfileDetailResult {
                    status: map_anyhow_error(err),
                    profile: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| ProfileDetailResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            profile: None,
        })
}

#[uniffi::export]
pub async fn profile_save(name: String, content: String, activate: bool) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let profile_name = match sanitize_profile_name(&name) {
                Ok(value) => value,
                Err(err) => return map_anyhow_error(err),
            };
            if let Err(err) = core_config::validate_yaml(&content) {
                return map_anyhow_error(err);
            }

            let manager = match ConfigManager::new() {
                Ok(value) => value,
                Err(err) => return map_mihomo_error(err),
            };
            if let Err(err) = manager.save(&profile_name, &content).await {
                return map_mihomo_error(err);
            }

            let previous = manager.get_current().await.ok();
            let should_apply = activate || previous.as_deref() == Some(profile_name.as_str());
            if activate && let Err(err) = manager.set_current(&profile_name).await {
                return map_mihomo_error(err);
            }
            if should_apply {
                // set_current + apply transaction; on rollback the activation
                // above is restored to the previous profile.
                let restore_to = if activate { previous } else { None };
                return apply_current_profile_status(restore_to).await;
            }
            FfiStatus::ok()
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_delete(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let profile_name = match sanitize_profile_name(&name) {
                Ok(value) => value,
                Err(err) => return map_anyhow_error(err),
            };
            let manager = match ConfigManager::new() {
                Ok(value) => value,
                Err(err) => return map_mihomo_error(err),
            };
            manager
                .delete_profile(&profile_name)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_mihomo_error)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_subscription_save(
    name: String,
    url: String,
    auto_update_enabled: bool,
    update_interval_hours: Option<u32>,
) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let profile_name = match core_profiles::sanitize_profile_name(&name) {
                Ok(value) => value,
                Err(err) => return map_anyhow_error(err),
            };
            let source_url = url.trim();
            if source_url.is_empty() {
                return FfiStatus::err(FfiErrorCode::InvalidInput, "subscription url is empty");
            }
            if auto_update_enabled && update_interval_hours.unwrap_or(0) == 0 {
                return FfiStatus::err(
                    FfiErrorCode::InvalidInput,
                    "update_interval_hours must be set when auto update is enabled",
                );
            }

            let manager = match ConfigManager::new() {
                Ok(value) => value,
                Err(err) => return map_mihomo_error(err),
            };

            if let Err(err) = manager.load(&profile_name).await {
                return map_mihomo_error(err);
            }

            let mut metadata = match manager.get_profile_metadata(&profile_name).await {
                Ok(value) => value,
                Err(err) => return map_mihomo_error(err),
            };
            metadata.subscription_url = Some(source_url.to_string());
            metadata.auto_update_enabled = auto_update_enabled;
            metadata.update_interval_hours = update_interval_hours;
            if auto_update_enabled {
                metadata.next_update = metadata
                    .update_interval_hours
                    .map(|hours| Utc::now() + ChronoDuration::hours(hours as i64));
            } else {
                metadata.next_update = None;
            }
            manager
                .update_profile_metadata(&profile_name, &metadata)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_mihomo_error)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_subscription_clear(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let profile_name = match core_profiles::sanitize_profile_name(&name) {
                Ok(value) => value,
                Err(err) => return map_anyhow_error(err),
            };
            let manager = match ConfigManager::new() {
                Ok(value) => value,
                Err(err) => return map_mihomo_error(err),
            };

            if let Err(err) = manager.load(&profile_name).await {
                return map_mihomo_error(err);
            }

            let mut metadata = match manager.get_profile_metadata(&profile_name).await {
                Ok(value) => value,
                Err(err) => return map_mihomo_error(err),
            };
            metadata.subscription_url = None;
            metadata.auto_update_enabled = false;
            metadata.update_interval_hours = None;
            metadata.last_updated = None;
            metadata.next_update = None;
            manager
                .update_profile_metadata(&profile_name, &metadata)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_mihomo_error)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

fn profile_to_summary(profile: ProfileInfo) -> ProfileSummary {
    ProfileSummary {
        name: profile.name,
        active: profile.active,
        subscription_url: profile.subscription_url,
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated.map(|value| value.to_rfc3339()),
        next_update: profile.next_update.map(|value| value.to_rfc3339()),
    }
}

fn profile_detail_to_record(profile: CoreProfileDetail) -> ProfileDetail {
    ProfileDetail {
        name: profile.name,
        active: profile.active,
        content: profile.content,
        subscription_url: profile.subscription_url,
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated.map(|value| value.to_rfc3339()),
        next_update: profile.next_update.map(|value| value.to_rfc3339()),
    }
}
