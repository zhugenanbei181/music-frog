//! Profile operations: listing, creating, selecting, updating, saving and
//! deleting profiles, plus subscription metadata management. Selection and
//! save go through the session apply transaction in [`super::session`].

use super::owner_operation::wire::route_owner_operation;
use super::owner_operation::{
    AndroidActiveProfileReferenceObservation, AndroidOwnerDataDomain, AndroidOwnerDataSnapshotData,
    AndroidOwnerOperation, AndroidOwnerOperationReadback, AndroidOwnerOperationReply,
    AndroidOwnerOperationRoute, AndroidProfileSelectionTarget,
};
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::host_session::switch::select_profile_target_status;
use crate::host_session::{
    activate_profile_status, active_profile_reference_state,
    apply_selected_profile_reference_status,
};
use crate::host_support::{
    build_config_manager, get_runtime, map_anyhow_error, map_application_failure,
    subscription_source,
};
use chrono::Utc;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_domain::config::validate_yaml;
use infiltrator_domain::profiles;
use infiltrator_domain::profiles::{ProfileInfo, sanitize_profile_name};

// --- Profiles API ---

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
pub struct ProfileSummary {
    pub name: String,
    pub active: bool,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<String>,
    pub next_update: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
pub struct ProfilesResult {
    pub status: FfiStatus,
    pub profiles: Vec<ProfileSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record, serde::Serialize, serde::Deserialize)]
pub struct ProfileDetailResult {
    pub status: FfiStatus,
    pub profile: Option<ProfileDetail>,
}

pub(super) async fn read_owner_profiles() -> ProfilesResult {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfilesList).await {
                Err(status) => {
                    return ProfilesResult {
                        status,
                        profiles: Vec::new(),
                    };
                }
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return match &reply.readback {
                        AndroidOwnerOperationReadback::ProfilesList(result)
                            if profile_snapshot_is_valid(&reply)
                                && reply.snapshot.as_ref().is_some_and(|snapshot| {
                                    matches!(&snapshot.data, AndroidOwnerDataSnapshotData::Profiles(profiles) if profiles == result)
                                }) =>
                        {
                            result.clone()
                        }
                        _ => ProfilesResult {
                            status: FfiStatus::err(
                                FfiErrorCode::NotReady,
                                "Android owner returned an invalid profiles readback",
                            ),
                            profiles: Vec::new(),
                        },
                    };
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
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

pub(super) async fn create_owner_profile(name: String, url: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileCreate {
                name: name.clone(),
                url: url.clone(),
            })
            .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_mutation_status(*reply);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
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

pub(super) async fn select_owner_profile(target: AndroidProfileSelectionTarget) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileSelect {
                target: target.clone(),
            })
            .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_selection_status(*reply, &target);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
            match target.into_reference_target() {
                Ok(target) => select_profile_target_status(target).await,
                Err(status) => status,
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

pub(super) async fn update_owner_profile(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileUpdate { name: name.clone() })
                .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_mutation_status(*reply);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
            let application = match build_profile_application().await {
                Ok(application) => application,
                Err(status) => return status,
            };
            let previous = match active_profile_reference_state().await {
                Ok(previous) => previous,
                Err(status) => return status,
            };
            let source = subscription_source();
            match application.update_subscription(&source, &name).await {
                Ok(profile) => {
                    if profile.active {
                        apply_selected_profile_reference_status(previous).await
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

pub(super) async fn read_owner_profile_detail(name: String) -> ProfileDetailResult {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileDetail { name: name.clone() })
                .await
            {
                Err(status) => {
                    return ProfileDetailResult {
                        status,
                        profile: None,
                    };
                }
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return match &reply.readback {
                        AndroidOwnerOperationReadback::ProfileDetail(result)
                            if profile_snapshot_is_valid(&reply) =>
                        {
                            result.clone()
                        }
                        _ => ProfileDetailResult {
                            status: FfiStatus::err(
                                FfiErrorCode::NotReady,
                                "Android owner returned an invalid profile detail readback",
                            ),
                            profile: None,
                        },
                    };
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
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

pub(super) async fn save_owner_profile(name: String, content: String, activate: bool) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileSave {
                name: name.clone(),
                content: content.clone(),
                activate,
            })
            .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_mutation_status(*reply);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
            let profile_name = match sanitize_profile_name(&name) {
                Ok(value) => value,
                Err(err) => return map_anyhow_error(err),
            };
            if let Err(err) = validate_yaml(&content) {
                return map_anyhow_error(err);
            }

            let application = match build_profile_application().await {
                Ok(value) => value,
                Err(status) => return status,
            };
            let previous = match active_profile_reference_state().await {
                Ok(previous) => previous,
                Err(status) => return status,
            };
            if let Err(failure) = application.save_profile(&profile_name, &content).await {
                return map_application_failure(failure);
            }

            let was_selected = matches!(
                &previous,
                ActiveProfileReferenceState::Selected(reference)
                    if reference.profile_id.as_str() == profile_name
            );
            if activate {
                return activate_profile_status(&profile_name).await;
            }
            if was_selected {
                return apply_selected_profile_reference_status(previous).await;
            }
            FfiStatus::ok()
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

pub(super) async fn delete_owner_profile(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileDelete { name: name.clone() })
                .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_mutation_status(*reply);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
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

pub(super) async fn save_owner_profile_subscription(
    name: String,
    url: String,
    auto_update_enabled: bool,
    update_interval_hours: Option<u32>,
) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileSubscriptionSave {
                name: name.clone(),
                url: url.clone(),
                auto_update_enabled,
                update_interval_hours,
            })
            .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_mutation_status(*reply);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
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

pub(super) async fn clear_owner_profile_subscription(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match route_owner_operation(AndroidOwnerOperation::ProfileSubscriptionClear {
                name: name.clone(),
            })
            .await
            {
                Err(status) => return status,
                Ok(AndroidOwnerOperationRoute::UiClient(reply)) => {
                    return profile_mutation_status(*reply);
                }
                Ok(AndroidOwnerOperationRoute::CoreOwner) => {}
            }
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

pub(super) async fn build_profile_application() -> Result<ProfileApplication, FfiStatus> {
    let manager = build_config_manager().await?;
    Ok(ProfileApplication::new(
        manager.shared_engine_profile_store(),
    ))
}

fn profile_snapshot_is_valid(reply: &AndroidOwnerOperationReply) -> bool {
    reply.data_domain == Some(AndroidOwnerDataDomain::Profiles)
        && matches!(
            reply.snapshot.as_ref(),
            Some(snapshot)
                if snapshot.domain == AndroidOwnerDataDomain::Profiles
                    && matches!(&snapshot.data, AndroidOwnerDataSnapshotData::Profiles(_))
        )
}

fn profile_mutation_status(reply: AndroidOwnerOperationReply) -> FfiStatus {
    if !profile_snapshot_is_valid(&reply) {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner mutation did not include a profiles snapshot readback",
        );
    }
    match reply.readback {
        AndroidOwnerOperationReadback::ProfileMutation(status) => status,
        _ => FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner returned an invalid profile mutation result",
        ),
    }
}

fn profile_selection_status(
    reply: AndroidOwnerOperationReply,
    target: &AndroidProfileSelectionTarget,
) -> FfiStatus {
    if !profile_snapshot_is_valid(&reply) {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner selection did not include a profiles snapshot readback",
        );
    }
    let AndroidOwnerOperationReadback::ProfileSelection(readback) = reply.readback else {
        return FfiStatus::err(
            FfiErrorCode::NotReady,
            "Android owner returned an invalid profile selection readback",
        );
    };
    match &readback.active_reference {
        AndroidActiveProfileReferenceObservation::Selected {
            target: observed, ..
        } if observed == target && readback.operation_status.code == FfiErrorCode::Ok => {
            readback.status()
        }
        _ => match readback.operation_status.code {
            FfiErrorCode::Ok => FfiStatus::err(
                FfiErrorCode::NotReady,
                "Android owner selection readback did not match the requested engine and profile",
            ),
            _ => readback.status(),
        },
    }
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

fn profile_detail_to_record(profile: profiles::ProfileDetail) -> ProfileDetail {
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
