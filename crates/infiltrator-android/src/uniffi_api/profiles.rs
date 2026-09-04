//! Profile operations: listing, creating, selecting, updating, saving and
//! deleting profiles, plus subscription metadata management. Selection and
//! save go through the session apply transaction in [`super::session`].

use chrono::Utc;

use super::session::apply_current_profile_status;
use super::support::{
    build_config_manager, get_runtime, map_anyhow_error, map_application_failure,
    subscription_source,
};
use crate::ffi::{FfiErrorCode, FfiStatus};
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_domain::profiles::{ProfileInfo, sanitize_profile_name};
use std::sync::Arc;

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
            let application = match build_profile_application().await {
                Ok(application) => application,
                Err(status) => {
                    return ProfilesResult {
                        status,
                        profiles: Vec::new(),
                    };
                }
            };
            match application.list_profiles().await {
                Ok(profiles) => ProfilesResult {
                    status: FfiStatus::ok(),
                    profiles: profiles.into_iter().map(profile_to_summary).collect(),
                },
                Err(failure) => ProfilesResult {
                    status: map_application_failure(failure),
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
            let application = match build_profile_application().await {
                Ok(application) => application,
                Err(status) => return status,
            };
            let source = subscription_source();
            match application.import_subscription(&source, &name, &url).await {
                Ok(_) => FfiStatus::ok(),
                Err(failure) => map_application_failure(failure),
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
            let application = match build_profile_application().await {
                Ok(application) => application,
                Err(status) => return status,
            };
            let previous = application.current_profile().await.ok();
            match application.select_profile(&name).await {
                // Apply the newly current profile through the session
                // transaction; on rollback the switch above is undone.
                Ok(_) => apply_current_profile_status(previous).await,
                Err(failure) => map_application_failure(failure),
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
            let application = match build_profile_application().await {
                Ok(application) => application,
                Err(status) => return status,
            };
            let previous = application.current_profile().await.ok();
            let source = subscription_source();
            match application.update_subscription(&source, &name).await {
                Ok(profile) => {
                    if profile.active {
                        apply_current_profile_status(previous).await
                    } else {
                        FfiStatus::ok()
                    }
                }
                Err(failure) => map_application_failure(failure),
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
            let application = match build_profile_application().await {
                Ok(application) => application,
                Err(status) => {
                    return ProfileDetailResult {
                        status,
                        profile: None,
                    };
                }
            };
            match application.load_profile_detail(&name).await {
                Ok(profile) => ProfileDetailResult {
                    status: FfiStatus::ok(),
                    profile: Some(profile_detail_to_record(profile)),
                },
                Err(failure) => ProfileDetailResult {
                    status: map_application_failure(failure),
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
            if let Err(err) = infiltrator_domain::config::validate_yaml(&content) {
                return map_anyhow_error(err);
            }

            let application = match build_profile_application().await {
                Ok(value) => value,
                Err(status) => return status,
            };
            let previous = application.current_profile().await.ok();
            if let Err(failure) = application.save_profile(&profile_name, &content).await {
                return map_application_failure(failure);
            }

            let should_apply = activate || previous.as_deref() == Some(profile_name.as_str());
            if activate && let Err(failure) = application.select_profile(&profile_name).await {
                return map_application_failure(failure);
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
            let application = match build_profile_application().await {
                Ok(value) => value,
                Err(status) => return status,
            };
            application
                .delete_profile(&profile_name)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_application_failure)
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
            let profile_name = match sanitize_profile_name(&name) {
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

            let application = match build_profile_application().await {
                Ok(value) => value,
                Err(status) => return status,
            };

            if let Err(failure) = application.load_profile_info(&profile_name).await {
                return map_application_failure(failure);
            }

            let mut metadata = match application.load_metadata(&profile_name).await {
                Ok(value) => value,
                Err(failure) => return map_application_failure(failure),
            };
            metadata.subscription_url = Some(source_url.to_string());
            metadata.auto_update_enabled = auto_update_enabled;
            metadata.update_interval_hours = update_interval_hours;
            if auto_update_enabled {
                metadata.next_update = metadata
                    .update_interval_hours
                    .map(|hours| Utc::now() + chrono::Duration::hours(hours as i64));
            } else {
                metadata.next_update = None;
            }
            application
                .update_metadata(&profile_name, &metadata)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_application_failure)
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
            let profile_name = match sanitize_profile_name(&name) {
                Ok(value) => value,
                Err(err) => return map_anyhow_error(err),
            };
            let application = match build_profile_application().await {
                Ok(value) => value,
                Err(status) => return status,
            };

            if let Err(failure) = application.load_profile_info(&profile_name).await {
                return map_application_failure(failure);
            }

            let mut metadata = match application.load_metadata(&profile_name).await {
                Ok(value) => value,
                Err(failure) => return map_application_failure(failure),
            };
            metadata.subscription_url = None;
            metadata.auto_update_enabled = false;
            metadata.update_interval_hours = None;
            metadata.last_updated = None;
            metadata.next_update = None;
            application
                .update_metadata(&profile_name, &metadata)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_application_failure)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

async fn build_profile_application() -> Result<ProfileApplication, FfiStatus> {
    let manager = build_config_manager().await?;
    Ok(ProfileApplication::new(Arc::new(manager)))
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

fn profile_detail_to_record(profile: infiltrator_domain::profiles::ProfileDetail) -> ProfileDetail {
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
