//! App preferences endpoints: editor integration, application settings, and
//! WebDAV backup sync (`/admin/api/editor*`, `/admin/api/settings`,
//! `/admin/api/profiles/open`, `/admin/api/webdav/*`).

use axum::{Json, http::StatusCode};
use infiltrator_domain::settings::WebDavConfig;

use crate::admin_api::events::{AdminEvent, EVENT_SETTINGS_CHANGED, EVENT_WEBDAV_SYNCED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

use super::profiles::ensure_valid_profile_name;

pub async fn get_editor_config_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.ctx.editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

pub async fn set_editor_config_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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
    state
        .events
        .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pick_editor_path_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<EditorConfigResponse>, ApiError> {
    let editor = state.ctx.pick_editor_path().await;
    Ok(Json(EditorConfigResponse { editor }))
}

pub async fn open_profile_in_editor_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<OpenProfilePayload>,
) -> Result<StatusCode, ApiError> {
    let name = ensure_valid_profile_name(&payload.name)?;
    let _ = state
        .ctx
        .open_profile_in_editor(&name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_app_settings_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<AppSettingsPayload>, ApiError> {
    let settings = state.ctx.get_app_settings().await;
    let autostart_enabled = if state.ctx.supports_autostart_control() {
        Some(state.ctx.autostart_enabled().await)
    } else {
        None
    };
    let system_proxy_enabled = if state.ctx.supports_system_proxy_control() {
        Some(state.ctx.system_proxy_enabled().await)
    } else {
        None
    };
    let runtime_running = Some(state.ctx.runtime_running().await);
    Ok(Json(AppSettingsPayload {
        editor_path: settings.editor_path,
        use_bundled_core: Some(settings.use_bundled_core),
        language: Some(settings.language),
        theme: Some(settings.theme),
        notifications_enabled: Some(settings.notifications_enabled),
        webdav: Some(settings.webdav),
        configs_dir: settings.configs_dir,
        autostart_enabled,
        system_proxy_enabled,
        runtime_running,
    }))
}

pub async fn save_app_settings_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<AppSettingsPayload>,
) -> Result<StatusCode, ApiError> {
    let mut settings = state.ctx.get_app_settings().await;

    if let Some(val) = payload.editor_path {
        let trimmed = val.trim().to_string();
        settings.editor_path = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }
    if let Some(val) = payload.use_bundled_core {
        settings.use_bundled_core = val;
    }
    if let Some(val) = payload.language {
        settings.language = val;
    }
    if let Some(val) = payload.theme {
        settings.theme = val;
    }
    if let Some(val) = payload.notifications_enabled {
        settings.notifications_enabled = val;
    }
    if let Some(val) = payload.webdav {
        // 密码永不落盘：只有客户端显式带非空 password 时才路由进 keyring；
        // 缺省/空 = 保持既有条目不动（GET 不回传明文，普通回传自然为空）。
        if !val.password.is_empty() {
            state
                .ctx
                .set_webdav_password(&val.password)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        }
        settings.webdav = val;
        settings.webdav.password = String::new();
    }
    if let Some(val) = payload.configs_dir {
        let trimmed = val.trim().to_string();
        settings.configs_dir = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        };
    }

    state
        .ctx
        .save_app_settings(settings)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if let Some(enabled) = payload.autostart_enabled {
        if !state.ctx.supports_autostart_control() {
            return Err(ApiError::bad_request("当前平台不支持开机自启控制"));
        }
        state
            .ctx
            .set_autostart_enabled(enabled)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    if let Some(enabled) = payload.system_proxy_enabled {
        if !state.ctx.supports_system_proxy_control() {
            return Err(ApiError::bad_request("当前平台不支持系统代理控制"));
        }
        state
            .ctx
            .set_system_proxy_enabled(enabled)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    state
        .events
        .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn sync_webdav_now_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let settings = state.ctx.get_app_settings().await;
    if !settings.webdav.enabled {
        return Err(ApiError::bad_request("WebDAV 同步未开启"));
    }

    let report = state
        .ctx
        .sync_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .sync(settings.webdav, settings.configs_dir)
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;

    state.events.publish(AdminEvent::new(EVENT_WEBDAV_SYNCED));

    Ok(Json(serde_json::json!({
        "success_count": report.success_count,
        "failed_count": report.failed_count,
        "total_actions": report.total_actions,
    })))
}

pub async fn test_webdav_conn_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<WebDavConfig>,
) -> Result<StatusCode, ApiError> {
    // GET 不再回传明文密码，客户端回传的 password 常为空：此时回退 keyring
    // 里保存的凭据，保持「测试连接」对已配置账号可用。
    state
        .ctx
        .sync_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
        .test(payload)
        .await
        .map_err(|failure| ApiError::bad_request(failure.message))?;

    Ok(StatusCode::OK)
}
