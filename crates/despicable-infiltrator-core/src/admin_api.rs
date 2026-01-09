use anyhow::anyhow;
use chrono::Utc;
use axum::{
    body::Body,
    extract::{Path as AxumPath, State as AxumState},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use log::{info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use crate::{
    config as core_config,
    editor as core_editor,
    profiles as core_profiles,
    subscription as core_subscription,
    version as core_version,
    ProfileDetail, ProfileInfo,
};
use mihomo_rs::{config::ConfigManager, version::VersionManager};

#[async_trait::async_trait]
pub trait AdminApiContext: Clone + Send + Sync + 'static {
    async fn rebuild_runtime(&self) -> anyhow::Result<()>;
    async fn set_use_bundled_core(&self, enabled: bool);
    async fn refresh_core_version_info(&self);
    async fn notify_subscription_update(
        &self,
        profile: String,
        success: bool,
        message: Option<String>,
    );
    async fn editor_path(&self) -> Option<String>;
    async fn set_editor_path(&self, path: Option<String>);
    async fn pick_editor_path(&self) -> Option<String>;
}

#[derive(Default)]
struct RebuildStatus {
    in_progress: AtomicBool,
    last_error: Mutex<Option<String>>,
    last_reason: Mutex<Option<String>>,
}

impl RebuildStatus {
    fn snapshot(&self) -> RebuildStatusResponse {
        let last_error = self
            .last_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        let last_reason = self
            .last_reason
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        RebuildStatusResponse {
            in_progress: self.in_progress.load(Ordering::SeqCst),
            last_error,
            last_reason,
        }
    }

    fn mark_start(&self, reason: &str) {
        self.in_progress.store(true, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.last_reason.lock() {
            *guard = Some(reason.to_string());
        }
    }

    fn mark_success(&self) {
        self.in_progress.store(false, Ordering::SeqCst);
    }

    fn mark_error(&self, err: String) {
        self.in_progress.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_error.lock() {
            *guard = Some(err);
        }
    }
}

#[derive(Clone)]
pub struct AdminApiState<C> {
    pub ctx: C,
    pub http_client: Client,
    pub raw_http_client: Client,
    rebuild_status: Arc<RebuildStatus>,
}

impl<C: AdminApiContext> AdminApiState<C> {
    pub fn new(ctx: C) -> Self {
        let http_client = Client::builder()
            .user_agent("Mihomo-Despicable-Infiltrator")
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|err| {
                warn!("failed to build http client: {err}");
                Client::new()
            });
        let raw_http_client = Client::builder()
            .user_agent("Mihomo-Despicable-Infiltrator")
            .timeout(Duration::from_secs(30))
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .unwrap_or_else(|err| {
                warn!("failed to build raw http client: {err}");
                http_client.clone()
            });
        let rebuild_status = Arc::new(RebuildStatus::default());
        Self {
            ctx,
            http_client,
            raw_http_client,
            rebuild_status,
        }
    }
}

#[derive(Deserialize)]
pub struct SwitchProfilePayload {
    pub name: String,
}

#[derive(Deserialize)]
pub struct ImportProfilePayload {
    pub name: String,
    pub url: String,
    pub activate: Option<bool>,
}

#[derive(Deserialize)]
pub struct SaveProfilePayload {
    pub name: String,
    pub content: String,
    pub activate: Option<bool>,
}

#[derive(Deserialize)]
pub struct OpenProfilePayload {
    pub name: String,
}

#[derive(Deserialize)]
pub struct SubscriptionConfigPayload {
    pub url: String,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
}

#[derive(Deserialize)]
pub struct EditorConfigPayload {
    pub editor: Option<String>,
}

#[derive(Serialize)]
pub struct EditorConfigResponse {
    pub editor: Option<String>,
}

#[derive(Serialize)]
pub struct CoreVersionsResponse {
    pub current: Option<String>,
    pub versions: Vec<String>,
}

#[derive(Serialize)]
pub struct RebuildStatusResponse {
    pub in_progress: bool,
    pub last_error: Option<String>,
    pub last_reason: Option<String>,
}

#[derive(Serialize)]
pub struct ProfileActionResponse {
    pub profile: ProfileInfo,
    pub rebuild_scheduled: bool,
}

#[derive(Deserialize)]
pub struct CoreActivatePayload {
    pub version: String,
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::internal(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_client_error() || self.status.is_server_error() {
            warn!("admin api error: {}", self.message);
        }
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

pub fn router<C: AdminApiContext>(state: AdminApiState<C>) -> Router {
    Router::new()
        .route("/admin/api/profiles", get(list_profiles_http::<C>))
        .route(
            "/admin/api/profiles/:name",
            get(get_profile_http::<C>).delete(delete_profile_http::<C>),
        )
        .route(
            "/admin/api/profiles/:name/subscription",
            post(set_profile_subscription_http::<C>)
                .delete(clear_profile_subscription_http::<C>),
        )
        .route(
            "/admin/api/profiles/:name/update-now",
            post(update_profile_now_http::<C>),
        )
        .route("/admin/api/profiles/switch", post(switch_profile_http::<C>))
        .route("/admin/api/profiles/save", post(save_profile_http::<C>))
        .route("/admin/api/profiles/import", post(import_profile_http::<C>))
        .route("/admin/api/profiles/clear", post(clear_profiles_http::<C>))
        .route("/admin/api/profiles/open", post(open_profile_in_editor_http::<C>))
        .route(
            "/admin/api/editor",
            get(get_editor_config_http::<C>).post(set_editor_config_http::<C>),
        )
        .route(
            "/admin/api/editor/pick",
            post(pick_editor_path_http::<C>),
        )
        .route("/admin/api/rebuild/status", get(get_rebuild_status_http::<C>))
        .route("/admin/api/core/versions", get(list_core_versions_http::<C>))
        .route("/admin/api/core/activate", post(activate_core_version_http::<C>))
        .with_state(state)
        .layer(middleware::from_fn(log_admin_request))
}

async fn list_profiles_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<Vec<ProfileInfo>>, ApiError> {
    let profiles = core_profiles::list_profile_infos()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(profiles))
}

async fn get_rebuild_status_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<RebuildStatusResponse>, ApiError> {
    Ok(Json(state.rebuild_status.snapshot()))
}

async fn get_profile_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<ProfileDetail>, ApiError> {
    let profile = core_profiles::load_profile_detail(&name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(profile))
}

async fn switch_profile_http<C: AdminApiContext>(
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

async fn import_profile_http<C: AdminApiContext>(
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

async fn save_profile_http<C: AdminApiContext>(
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

async fn clear_profiles_http<C: AdminApiContext>(
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

async fn delete_profile_http<C: AdminApiContext>(
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

async fn set_profile_subscription_http<C: AdminApiContext>(
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

async fn clear_profile_subscription_http<C: AdminApiContext>(
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

async fn update_profile_now_http<C: AdminApiContext>(
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

async fn get_editor_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.ctx.editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

async fn set_editor_config_http<C: AdminApiContext>(
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

async fn pick_editor_path_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.ctx.pick_editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

async fn open_profile_in_editor_http<C: AdminApiContext>(
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

async fn list_core_versions_http<C: AdminApiContext>(
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

async fn activate_core_version_http<C: AdminApiContext>(
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

async fn log_admin_request(req: Request<Body>, next: Next) -> Response {
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
