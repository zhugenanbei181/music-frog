use std::{
    sync::Arc,
    time::Instant,
};

use anyhow::anyhow;
use axum::{
    body::Body,
    extract::{Path as AxumPath, State as AxumState},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use chrono::Utc;
use log::{info, warn};
use reqwest::Client;

use crate::{
    config as core_config,
    editor as core_editor,
    profiles as core_profiles,
    subscription as core_subscription,
    version as core_version,
    ProfileDetail, ProfileInfo,
};
use mihomo_rs::{config::ConfigManager, version::VersionManager};

use super::models::*;
use super::state::{AdminApiContext, AdminApiState, RebuildStatus};

pub async fn list_profiles_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<Vec<ProfileInfo>>, ApiError> {
    let profiles = core_profiles::list_profile_infos()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(profiles))
}

pub async fn get_rebuild_status_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<RebuildStatusResponse>, ApiError> {
    Ok(Json(state.rebuild_status.snapshot()))
}

pub async fn get_profile_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<ProfileDetail>, ApiError> {
    let profile = core_profiles::load_profile_detail(&name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(profile))
}

pub async fn switch_profile_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<SwitchProfilePayload>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    let profile = switch_profile_internal(&state.ctx, &state.rebuild_status, &name).await?;
    Ok(Json(ProfileActionResponse {
        profile,
        rebuild_scheduled: true,
    }))
}

pub async fn import_profile_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<ImportProfilePayload>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let profile_name = ensure_valid_profile_name(&payload.name)?;
    if payload.url.trim().is_empty() {
        return Err(ApiError::bad_request(
            "订阅链接不能为空",
        ));
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
    Ok(Json(ProfileActionResponse {
        profile,
        rebuild_scheduled,
    }))
}

pub async fn save_profile_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<SaveProfilePayload>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    if let Err(err) = core_config::validate_yaml(&payload.content) {
        return Err(ApiError::bad_request(err.to_string()));
    }

    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
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

    let mut info = core_profiles::load_profile_info(&name).await?;
    info.controller_url = controller_url;
    info.controller_changed = controller_changed;
    Ok(Json(ProfileActionResponse {
        profile: info,
        rebuild_scheduled,
    }))
}

pub async fn clear_profiles_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let profile = core_profiles::reset_profiles_to_default()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let mut info = profile;
    info.controller_url = manager.get_external_controller().await.ok();
    schedule_rebuild(&state.ctx, &state.rebuild_status, "profiles-clear");
    Ok(Json(ProfileActionResponse {
        profile: info,
        rebuild_scheduled: true,
    }))
}

pub async fn delete_profile_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    manager
        .delete_profile(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_profile_subscription_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
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

    core_profiles::load_profile_info(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
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
    let info = core_profiles::load_profile_info(&profile_name).await?;
    Ok(Json(info))
}

pub async fn clear_profile_subscription_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<ProfileInfo>, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    core_profiles::load_profile_info(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
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
    let info = core_profiles::load_profile_info(&profile_name).await?;
    Ok(Json(info))
}

pub async fn update_profile_now_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<ProfileActionResponse>, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    core_profiles::load_profile_info(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let mut metadata = manager
        .get_profile_metadata(&profile_name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let url = metadata
        .subscription_url
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("未找到订阅链接"))?;

    let content =
        core_subscription::fetch_subscription_text(&state.http_client, &state.raw_http_client, url)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    let content = core_subscription::strip_utf8_bom(&content);
    if core_config::validate_yaml(&content).is_err() {
        return Err(ApiError::bad_request("订阅内容不是有效的 YAML"));
    }
    manager
        .save(&profile_name, &content)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let now = Utc::now();
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

    let rebuild_scheduled = manager
        .get_current()
        .await
        .ok()
        .as_deref()
        == Some(&profile_name);
    if rebuild_scheduled {
        schedule_rebuild(&state.ctx, &state.rebuild_status, "subscription-update-now");
    }
    let profile = core_profiles::load_profile_info(&profile_name).await?;
    Ok(Json(ProfileActionResponse {
        profile,
        rebuild_scheduled,
    }))
}

pub async fn get_editor_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.ctx.editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

pub async fn set_editor_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<EditorConfigPayload>,
) -> Result<StatusCode, ApiError> {
    let editor = payload.editor.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });
    state.ctx.set_editor_path(editor).await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pick_editor_path_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.ctx.pick_editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

pub async fn open_profile_in_editor_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<OpenProfilePayload>,
) -> Result<StatusCode, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    let editor_path = state.ctx.editor_path().await;
    core_editor::open_profile_in_editor(editor_path, &name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_core_versions_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<CoreVersionsResponse>, ApiError> {
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let versions = vm
        .list_installed()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut list: Vec<String> = versions.into_iter().map(|v| v.version).collect();
    core_version::sort_versions_desc(&mut list);
    let current = vm.get_default().await.ok();
    Ok(Json(CoreVersionsResponse {
        current,
        versions: list,
    }))
}

pub async fn activate_core_version_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<CoreActivatePayload>,
) -> Result<StatusCode, ApiError> {
    let version = payload.version.trim();
    if version.is_empty() {
        return Err(ApiError::bad_request(
            "版本不能为空",
        ));
    }
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    state.ctx.set_use_bundled_core(false).await;
    vm.set_default(version)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "core-activate");
    state.ctx.refresh_core_version_info().await;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_app_settings_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<AppSettingsPayload>, ApiError> {
    let settings = state.ctx.get_app_settings().await;
    Ok(Json(AppSettingsPayload {
        open_webui_on_startup: Some(settings.open_webui_on_startup),
        editor_path: settings.editor_path,
        use_bundled_core: Some(settings.use_bundled_core),
        language: Some(settings.language),
    }))
}

pub async fn save_app_settings_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<AppSettingsPayload>,
) -> Result<StatusCode, ApiError> {
    let mut settings = state.ctx.get_app_settings().await;
    
    if let Some(val) = payload.open_webui_on_startup {
        settings.open_webui_on_startup = val;
    }
    if let Some(val) = payload.editor_path {
        let trimmed = val.trim().to_string();
        settings.editor_path = if trimmed.is_empty() { None } else { Some(trimmed) };
    }
    if let Some(val) = payload.use_bundled_core {
        settings.use_bundled_core = val;
    }
    if let Some(val) = payload.language {
        settings.language = val;
    }

    state.ctx.save_app_settings(settings).await.map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

fn ensure_valid_profile_name(name: &str) -> Result<String, ApiError> {
    core_profiles::sanitize_profile_name(name).map_err(|e| ApiError::bad_request(e.to_string()))
}

async fn switch_profile_internal<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    name: &str,
) -> anyhow::Result<ProfileInfo> {
    let profile_name = core_profiles::sanitize_profile_name(name)?;
    let manager = ConfigManager::new()?;
    manager.set_current(&profile_name).await?;
    schedule_rebuild(ctx, rebuild_status, "switch-profile");
    core_profiles::load_profile_info(&profile_name).await
}

async fn import_profile_from_url_internal<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    client: &Client,
    raw_client: &Client,
    name: &str,
    url: &str,
    activate: bool,
) -> anyhow::Result<(ProfileInfo, bool)> {
    let profile_name = core_profiles::sanitize_profile_name(name)?;
    let source_url = url.trim();
    if source_url.is_empty() {
        return Err(anyhow!(
            "订阅链接不能为空"
        ));
    }

    let masked_url = core_subscription::mask_subscription_url(source_url);
    info!(
        "admin import profile start: name={} url={}",
        profile_name, masked_url
    );
    let content =
        core_subscription::fetch_subscription_text(client, raw_client, source_url).await?;
    if content.trim().is_empty() {
        return Err(anyhow!(
            "订阅返回内容为空"
        ));
    }
    let content = core_subscription::strip_utf8_bom(&content);
    if core_config::validate_yaml(&content).is_err() {
        return Err(anyhow!(
            "订阅内容不是有效的 YAML"
        ));
    }

    let manager = ConfigManager::new()?;
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
    metadata.last_updated = Some(now);
    metadata.next_update = if metadata.auto_update_enabled {
        metadata
            .update_interval_hours
            .map(|hours| now + chrono::Duration::hours(hours as i64))
    } else {
        None
    };
    manager.update_profile_metadata(&profile_name, &metadata).await?;

    let mut info = core_profiles::load_profile_info(&profile_name).await?;
    if activate {
        info.controller_url = manager.get_external_controller().await.ok();
    }
    Ok((info, rebuild_scheduled))
}

pub async fn log_admin_request(req: Request<Body>, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let start = Instant::now();
    let response = next.run(req).await;
    let status = response.status();
    let elapsed = start.elapsed();
    if status.is_client_error() || status.is_server_error() {
        warn!(
            "admin api {} {}{} -> {} ({}ms)",
            method,
            path,
            query,
            status.as_u16(),
            elapsed.as_millis()
        );
    } else {
        info!(
            "admin api {} {}{} -> {} ({}ms)",
            method,
            path,
            query,
            status.as_u16(),
            elapsed.as_millis()
        );
    }
    response
}

fn schedule_rebuild<C: AdminApiContext>(
    ctx: &C,
    rebuild_status: &Arc<RebuildStatus>,
    reason: &str,
) {
    let ctx = ctx.clone();
    let reason = reason.to_string();
    let rebuild_status = Arc::clone(rebuild_status);
    info!("schedule runtime rebuild: {reason}");
    rebuild_status.mark_start(&reason);
    tokio::spawn(async move {
        if let Err(err) = ctx.rebuild_runtime().await {
            warn!("runtime rebuild failed ({reason}): {err}");
            rebuild_status.mark_error(err.to_string());
        } else {
            info!("runtime rebuild completed ({reason})");
            rebuild_status.mark_success();
        }
    });
}
