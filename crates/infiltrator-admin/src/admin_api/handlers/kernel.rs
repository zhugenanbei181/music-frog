//! Mihomo kernel (core binary) version management endpoints
//! (`/admin/api/core/*`).

use axum::{Json, extract::State as AxumState, http::StatusCode};
use mihomo_version::manager::VersionManager;

use crate::admin_api::events::{AdminEvent, EVENT_CORE_CHANGED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

use super::schedule_rebuild;

pub async fn list_core_versions_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<CoreVersionsResponse>, ApiError> {
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let versions = vm
        .list_installed()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut list: Vec<String> = versions.into_iter().map(|v| v.version).collect();
    sort_versions_desc(&mut list);
    let current = vm.get_default().await.ok();
    Ok(Json(CoreVersionsResponse {
        current,
        versions: list,
    }))
}

pub async fn get_latest_stable_core_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<CoreLatestStableResponse>, ApiError> {
    let (version, release_date) = state
        .ctx
        .latest_stable_core()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(CoreLatestStableResponse {
        version,
        release_date,
    }))
}

pub async fn download_core_version_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<CoreDownloadPayload>,
) -> Result<Json<CoreDownloadResponse>, ApiError> {
    let version = payload.version.trim().to_string();
    if version.is_empty() {
        return Err(ApiError::bad_request("版本不能为空"));
    }
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let outcome = ensure_core_version_installed(&vm, &version).await?;
    state.events.publish(AdminEvent::new(EVENT_CORE_CHANGED));
    Ok(Json(CoreDownloadResponse {
        version,
        downloaded: outcome.downloaded,
        already_installed: outcome.already_installed,
    }))
}

pub async fn update_stable_core_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<CoreUpdateStableResponse>, ApiError> {
    let (version, _release_date) = state
        .ctx
        .latest_stable_core()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    let outcome = ensure_core_version_installed(&vm, &version).await?;

    state.ctx.set_use_bundled_core(false).await;
    vm.set_default(&version)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
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
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<CoreActivatePayload>,
) -> Result<StatusCode, ApiError> {
    let version = payload.version.trim();
    if version.is_empty() {
        return Err(ApiError::bad_request("版本不能为空"));
    }
    let vm = VersionManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    state.ctx.set_use_bundled_core(false).await;
    vm.set_default(version)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
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
    vm: &VersionManager,
    version: &str,
) -> Result<CoreInstallOutcome, ApiError> {
    let installed = vm
        .list_installed()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if installed.iter().any(|item| item.version == version) {
        return Ok(CoreInstallOutcome {
            downloaded: false,
            already_installed: true,
        });
    }

    if let Err(err) = vm.install_with_progress(version, |_| {}).await {
        let installed_after = vm
            .list_installed()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        if installed_after.iter().any(|item| item.version == version) {
            return Ok(CoreInstallOutcome {
                downloaded: false,
                already_installed: true,
            });
        }
        return Err(ApiError::bad_request(err.to_string()));
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
