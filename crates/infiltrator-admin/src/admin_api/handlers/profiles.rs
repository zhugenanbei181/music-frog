//! Profile / subscription management endpoints (`/admin/api/profiles*`).

use std::sync::Arc;

use anyhow::anyhow;
use axum::{Json, http::StatusCode};
use chrono::Utc;
use infiltrator_core::profiles::{ProfileDetail, ProfileInfo};
use infiltrator_http::HttpClient;
use log::info;

use crate::admin_api::events::{AdminEvent, EVENT_PROFILES_CHANGED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState, RebuildStatus};
use crate::support::app_config_manager;

use super::{schedule_core_restart, schedule_rebuild};

pub async fn list_profiles_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<Vec<ProfileInfo>>, ApiError> {
    let profiles = infiltrator_core::profiles::list_profile_infos()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(profiles))
}

pub async fn get_profile_http<C: AdminApiContext>(
    axum::extract::State(_state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<ProfileDetail>, ApiError> {
    let profile = infiltrator_core::profiles::load_profile_detail(&name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(profile))
}

pub async fn switch_profile_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<SwitchProfilePayload>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    let profile = switch_profile_internal(&state.ctx, &state.rebuild_status, &name).await?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfileActionResponse {
        profile,
        rebuild_scheduled: true,
    }))
}

pub async fn import_profile_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<ImportProfilePayload>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let profile_name = ensure_valid_profile_name(&payload.name)?;
    if payload.url.trim().is_empty() {
        return Err(ApiError::bad_request("订阅链接不能为空"));
    }
    let (profile, rebuild_scheduled) = import_profile_from_url_internal(
        &state.ctx,
        &state.rebuild_status,
        &state.http_client,
        &state.raw_http_client,
        &profile_name,
        &payload.url,
        payload.activate.unwrap_or(false),
    )
    .await?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfileActionResponse {
        profile,
        rebuild_scheduled,
    }))
}

pub async fn save_profile_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<SaveProfilePayload>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    if let Err(err) = infiltrator_core::config::validate_yaml(&payload.content) {
        return Err(ApiError::bad_request(err.to_string()));
    }

    let manager = app_config_manager()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let current_before = manager.get_current().await.ok();
    let is_current = current_before.as_deref() == Some(&name);
    let controller_before = if is_current || payload.activate.unwrap_or(false) {
        manager.get_external_controller().await.ok()
    } else {
        None
    };

    manager
        .save(&name, &payload.content)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let mut controller_url = None;
    let mut controller_changed = None;
    let activate = payload.activate.unwrap_or(false);
    let mut rebuild_scheduled = false;
    if activate {
        manager
            .set_current(&name)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        schedule_rebuild(&state.ctx, &state.rebuild_status, "save-activate");
        rebuild_scheduled = true;
        controller_url = manager.get_external_controller().await.ok();
    } else if manager.get_current().await.ok().as_deref() == Some(&name) {
        schedule_rebuild(&state.ctx, &state.rebuild_status, "save-current");
        rebuild_scheduled = true;
        controller_url = manager.get_external_controller().await.ok();
    }
    if controller_url.is_some() {
        controller_changed = Some(controller_before != controller_url);
    }

    let mut info = infiltrator_core::profiles::load_profile_info(&name).await?;
    info.controller_url = controller_url;
    info.controller_changed = controller_changed;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfileActionResponse {
        profile: info,
        rebuild_scheduled,
    }))
}

pub async fn clear_profiles_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let profile = infiltrator_core::profiles::reset_profiles_to_default()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    // All previous profiles (and their auto-update jobs) are gone.
    crate::scheduler::cancel_all_profile_jobs();
    let manager = app_config_manager()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut info = profile;
    info.controller_url = manager.get_external_controller().await.ok();
    schedule_rebuild(&state.ctx, &state.rebuild_status, "profiles-clear");
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfileActionResponse {
        profile: info,
        rebuild_scheduled: true,
    }))
}

pub async fn delete_profile_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    let manager = app_config_manager()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    manager
        .delete_profile(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    // The profile is gone: its periodic update job must not fire again.
    crate::scheduler::cancel_profile_update_job(&profile_name);
    // Best-effort sidecar cleanup so a recreated profile of the same name
    // does not inherit the deleted one's filter/mixin. The sidecar lives in
    // the (possibly redirected) configs dir, not under home.
    if let Ok(configs_dir) = crate::support::app_configs_dir().await {
        infiltrator_core::profile_options_io::delete_options(&configs_dir, &profile_name).await;
    }
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_profile_subscription_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(payload): Json<SubscriptionConfigPayload>,
) -> Result<Json<ProfileInfo>, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    let url = payload.url.trim();
    if url.is_empty() {
        return Err(ApiError::bad_request("订阅链接不能为空"));
    }
    if payload.auto_update_enabled && payload.update_interval_hours.unwrap_or(0) == 0 {
        return Err(ApiError::bad_request("更新间隔不能为空"));
    }

    infiltrator_core::profiles::load_profile_info(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let manager = app_config_manager()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut metadata = manager
        .get_profile_metadata(&profile_name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    metadata.subscription_url = Some(url.to_string());
    metadata.auto_update_enabled = payload.auto_update_enabled;
    metadata.update_interval_hours = payload.update_interval_hours;
    if payload.auto_update_enabled {
        if let Some(hours) = payload.update_interval_hours {
            metadata.next_update = Some(Utc::now() + chrono::Duration::hours(hours as i64));
        }
    } else {
        metadata.next_update = None;
    }
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    // Keep the per-profile periodic job in lockstep with the new metadata
    // (spawn on enable / interval change, cancel on disable).
    crate::scheduler::sync_profile_job(
        &state.ctx,
        &profile_name,
        metadata.auto_update_enabled,
        metadata.subscription_url.as_deref(),
        metadata.update_interval_hours,
    );
    let info = infiltrator_core::profiles::load_profile_info(&profile_name).await?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(info))
}

pub async fn clear_profile_subscription_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<ProfileInfo>, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    infiltrator_core::profiles::load_profile_info(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let manager = app_config_manager()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut metadata = manager
        .get_profile_metadata(&profile_name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    metadata.subscription_url = None;
    metadata.auto_update_enabled = false;
    metadata.update_interval_hours = None;
    metadata.last_updated = None;
    metadata.next_update = None;
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    // Subscription (and auto-update with it) is gone: drop the periodic job.
    crate::scheduler::sync_profile_job(&state.ctx, &profile_name, false, None, None);
    let info = infiltrator_core::profiles::load_profile_info(&profile_name).await?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(info))
}

pub async fn update_profile_now_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    infiltrator_core::profiles::load_profile_info(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let manager = app_config_manager()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut metadata = manager
        .get_profile_metadata(&profile_name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let url = metadata
        .subscription_url
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("未找到订阅链接"))?;

    let checked_url = infiltrator_core::subscription::CheckedSubscriptionUrl::parse(url)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let (content, userinfo) = infiltrator_core::subscription::fetch_subscription_with_info(
        &state.http_client,
        &state.raw_http_client,
        &checked_url,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    let content = infiltrator_core::subscription::strip_utf8_bom(&content);
    let (content, _report) =
        infiltrator_core::profile_options_io::apply_saved_options_for(&profile_name, content)
            .await
            .map_err(|e| ApiError::bad_request(format!("应用配置选项失败: {e}")))?;
    if infiltrator_core::config::validate_yaml(&content).is_err() {
        return Err(ApiError::bad_request("订阅内容不是有效的 YAML"));
    }
    manager
        .save(&profile_name, &content)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let now = Utc::now();
    if let Some(info) = userinfo {
        metadata.traffic_upload = info.upload;
        metadata.traffic_download = info.download;
        metadata.traffic_total = info.total;
        metadata.expire_at = info.expire;
    }
    metadata.last_updated = Some(now);
    metadata.next_update = if metadata.auto_update_enabled {
        metadata
            .update_interval_hours
            .map(|hours| now + chrono::Duration::hours(hours as i64))
    } else {
        None
    };
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let rebuild_scheduled = manager.get_current().await.ok().as_deref() == Some(&profile_name);
    if rebuild_scheduled {
        schedule_rebuild(&state.ctx, &state.rebuild_status, "subscription-update-now");
    }
    let profile = infiltrator_core::profiles::load_profile_info(&profile_name).await?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfileActionResponse {
        profile,
        rebuild_scheduled,
    }))
}

pub async fn update_all_profiles_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<ProfilesUpdateAllResponse>, ApiError> {
    let summary = crate::scheduler::subscription::update_all_subscriptions(
        &state.ctx,
        &state.http_client,
        &state.raw_http_client,
    )
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfilesUpdateAllResponse::from(summary)))
}

pub(crate) fn ensure_valid_profile_name(name: &str) -> Result<String, ApiError> {
    infiltrator_core::profiles::sanitize_profile_name(name)
        .map_err(|e| ApiError::bad_request(e.to_string()))
}

async fn switch_profile_internal<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    name: &str,
) -> anyhow::Result<ProfileInfo> {
    let profile_name = infiltrator_core::profiles::sanitize_profile_name(name)?;
    let manager = app_config_manager().await?;
    manager.set_current(&profile_name).await?;
    // Config-level change only: the running core just needs a restart, not
    // a full runtime re-bootstrap (which stays reserved for version swaps).
    schedule_core_restart(ctx, rebuild_status, "switch-profile");
    infiltrator_core::profiles::load_profile_info(&profile_name).await
}

async fn import_profile_from_url_internal<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    client: &HttpClient,
    raw_client: &HttpClient,
    name: &str,
    url: &str,
    activate: bool,
) -> anyhow::Result<(ProfileInfo, bool)> {
    let profile_name = infiltrator_core::profiles::sanitize_profile_name(name)?;
    let source_url = url.trim();
    if source_url.is_empty() {
        return Err(anyhow!("订阅链接不能为空"));
    }

    let masked_url = infiltrator_core::subscription::mask_subscription_url(source_url);
    info!(
        "admin import profile start: name={} url={}",
        profile_name, masked_url
    );
    let checked_url = infiltrator_core::subscription::CheckedSubscriptionUrl::parse(source_url)?;
    let (content, userinfo) = infiltrator_core::subscription::fetch_subscription_with_info(
        client,
        raw_client,
        &checked_url,
    )
    .await?;
    if content.trim().is_empty() {
        return Err(anyhow!("订阅返回内容为空"));
    }
    let content = infiltrator_core::subscription::strip_utf8_bom(&content);
    let (content, _report) =
        infiltrator_core::profile_options_io::apply_saved_options_for(&profile_name, content)
            .await
            .map_err(|e| anyhow!("应用配置选项失败: {e}"))?;
    if infiltrator_core::config::validate_yaml(&content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }

    let manager = app_config_manager().await?;
    manager.save(&profile_name, &content).await?;

    let mut rebuild_scheduled = false;
    if activate {
        manager.set_current(&profile_name).await?;
        schedule_rebuild(ctx, rebuild_status, "import-activate");
        rebuild_scheduled = true;
    }

    let now = Utc::now();
    let mut metadata = manager.get_profile_metadata(&profile_name).await?;
    metadata.subscription_url = Some(source_url.to_string());
    if let Some(info) = userinfo {
        metadata.traffic_upload = info.upload;
        metadata.traffic_download = info.download;
        metadata.traffic_total = info.total;
        metadata.expire_at = info.expire;
    }
    metadata.last_updated = Some(now);
    metadata.next_update = if metadata.auto_update_enabled {
        metadata
            .update_interval_hours
            .map(|hours| now + chrono::Duration::hours(hours as i64))
    } else {
        None
    };
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await?;
    // Re-import may have (re)enabled auto-update on an existing profile.
    crate::scheduler::sync_profile_job(
        ctx,
        &profile_name,
        metadata.auto_update_enabled,
        metadata.subscription_url.as_deref(),
        metadata.update_interval_hours,
    );

    let mut info = infiltrator_core::profiles::load_profile_info(&profile_name).await?;
    if activate {
        info.controller_url = manager.get_external_controller().await.ok();
    }
    Ok((info, rebuild_scheduled))
}
