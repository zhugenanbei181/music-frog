//! Mihomo kernel (core binary) version management endpoints
//! (`/admin/api/core/*`).

use axum::{Json, http::StatusCode};
use infiltrator_application::version_application::{QuietVersionProgress, VersionApplication};
use infiltrator_contract::version::CoreReleaseChannel;

use crate::admin_api::events::{AdminEvent, EVENT_CORE_CHANGED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

use super::schedule_rebuild;

pub async fn list_core_versions_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<CoreVersionsResponse>, ApiError> {
    let application = state
        .ctx
        .version_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let versions = application
        .list_installed()
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;
    let mut list: Vec<String> = versions.iter().map(|version| version.version.clone()).collect();
    sort_versions_desc(&mut list);
    let current = versions
        .into_iter()
        .find(|version| version.is_default)
        .map(|version| version.version);
    Ok(Json(CoreVersionsResponse {
        current,
        versions: list,
    }))
}

pub async fn get_latest_stable_core_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<CoreLatestStableResponse>, ApiError> {
    let (version, release_date) = state
        .ctx
        .version_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .latest(CoreReleaseChannel::Stable)
        .await
        .map(|release| (release.version, release.release_date))
        .map_err(|failure| ApiError::internal(failure.message))?;
    Ok(Json(CoreLatestStableResponse {
        version,
        release_date,
    }))
}

pub async fn download_core_version_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<CoreDownloadPayload>,
) -> Result<Json<CoreDownloadResponse>, ApiError> {
    let version = payload.version.trim().to_string();
    if version.is_empty() {
        return Err(ApiError::bad_request("版本不能为空"));
    }
    let application = state
        .ctx
        .version_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let outcome = ensure_core_version_installed(&application, &version).await?;
    state.events.publish(AdminEvent::new(EVENT_CORE_CHANGED));
    Ok(Json(CoreDownloadResponse {
        version,
        downloaded: outcome.downloaded,
        already_installed: outcome.already_installed,
    }))
}

pub async fn update_stable_core_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<CoreUpdateStableResponse>, ApiError> {
    let release = state
        .ctx
        .version_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .latest(CoreReleaseChannel::Stable)
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;
    let version = release.version;
    let application = state
        .ctx
        .version_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    let outcome = ensure_core_version_installed(&application, &version).await?;

    state.ctx.set_use_bundled_core(false).await;
    application
        .activate(&version)
        .await
        .map_err(|failure| ApiError::bad_request(failure.message))?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "core-update-stable");
    state.ctx.refresh_core_version_info().await;
    state.events.publish(AdminEvent::new(EVENT_CORE_CHANGED));

    Ok(Json(CoreUpdateStableResponse {
        version,
        downloaded: outcome.downloaded,
        already_installed: outcome.already_installed,
        rebuild_scheduled: true,
    }))
}

pub async fn activate_core_version_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    Json(payload): Json<CoreActivatePayload>,
) -> Result<StatusCode, ApiError> {
    let version = payload.version.trim();
    if version.is_empty() {
        return Err(ApiError::bad_request("版本不能为空"));
    }
    let application = state
        .ctx
        .version_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?;
    state.ctx.set_use_bundled_core(false).await;
    application
        .activate(version)
        .await
        .map_err(|failure| ApiError::bad_request(failure.message))?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "core-activate");
    state.ctx.refresh_core_version_info().await;
    state.events.publish(AdminEvent::new(EVENT_CORE_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

struct CoreInstallOutcome {
    downloaded: bool,
    already_installed: bool,
}

async fn ensure_core_version_installed(
    application: &VersionApplication,
    version: &str,
) -> Result<CoreInstallOutcome, ApiError> {
    let installed = application
        .list_installed()
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;
    if installed.iter().any(|item| item.version == version) {
        return Ok(CoreInstallOutcome {
            downloaded: false,
            already_installed: true,
        });
    }

    if let Err(failure) = application
        .install(version.to_string(), std::sync::Arc::new(QuietVersionProgress))
        .await
    {
        let installed_after = application
            .list_installed()
            .await
            .map_err(|failure| ApiError::internal(failure.message))?;
        if installed_after.iter().any(|item| item.version == version) {
            return Ok(CoreInstallOutcome {
                downloaded: false,
                already_installed: true,
            });
        }
        return Err(ApiError::bad_request(failure.message));
    }

    Ok(CoreInstallOutcome {
        downloaded: true,
        already_installed: false,
    })
}

pub(super) fn sort_versions_desc(list: &mut [String]) {
    list.sort_by(|a, b| compare_versions_desc(a, b));
}

pub(super) fn compare_versions_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let va = parse_version(a);
    let vb = parse_version(b);
    match (va, vb) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

pub(super) fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let trimmed = version.trim().trim_start_matches('v');
    let core = trimmed.split('-').next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}
