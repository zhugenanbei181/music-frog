//! Runtime lifecycle, connections, and streaming telemetry endpoints
//! (`/admin/api/runtime/*` except proxies/delay).

use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use log::warn;
use infiltrator_domain::runtime::{ConnectionsResponse, MemoryData};
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use futures_util::StreamExt;

use crate::admin_api::events::{AdminEvent, EVENT_RUNTIME_CHANGED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

use super::proxies::normalize_proxy_mode;

pub async fn get_runtime_status_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn start_runtime_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    state
        .ctx
        .rebuild_runtime()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn stop_runtime_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    state
        .ctx
        .stop_runtime()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn list_runtime_connections_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<ConnectionsResponse>, ApiError> {
    let client = state
        .ctx
        .runtime_gateway()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let snapshot = client
        .get_connections()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data = ConnectionsResponse {
        download_total: snapshot.download_total,
        upload_total: snapshot.upload_total,
        connections: snapshot.connections,
    };
    Ok(Json(data))
}

pub async fn close_all_runtime_connections_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<StatusCode, ApiError> {
    let client = state
        .ctx
        .runtime_gateway()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    client
        .close_all_connections()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn close_runtime_connection_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    let connection_id = id.trim();
    if connection_id.is_empty() {
        return Err(ApiError::bad_request("连接 ID 不能为空"));
    }
    let client = state
        .ctx
        .runtime_gateway()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    client
        .close_connection(connection_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn stream_runtime_logs_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    axum::extract::Query(query): axum::extract::Query<RuntimeLogsQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let level = normalize_log_level(query.level.as_deref())?;
    let client = state
        .ctx
        .runtime_gateway()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let receiver = client
        .stream_logs(level)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let stream = receiver.filter_map(|event| async move {
        let RuntimeStreamEvent::Item(message) = event else {
            return None;
        };
        let payload = match serde_json::to_string(&RuntimeLogEvent { message }) {
            Ok(payload) => payload,
            Err(err) => {
                warn!("failed to serialize runtime log event: {err}");
                return None;
            }
        };
        Some(Ok(Event::default().data(payload)))
    });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}

pub async fn get_runtime_traffic_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeTrafficSnapshotResponse>, ApiError> {
    let client = state
        .ctx
        .runtime_gateway()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let connections = client
        .get_connections()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let snapshot = state.traffic_snapshot(
        connections.upload_total,
        connections.download_total,
        connections.connections.len(),
    );
    Ok(Json(snapshot))
}

pub async fn get_runtime_memory_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<MemoryData>, ApiError> {
    let client = state
        .ctx
        .runtime_gateway()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let memory = client
        .get_memory()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(memory))
}

pub async fn get_runtime_ip_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeIpCheckResponse>, ApiError> {
    let snapshot = state
        .ctx
        .network_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .probe_public_ip(None)
        .await
        .map_err(|failure| ApiError::internal(failure.message))?;

    Ok(Json(RuntimeIpCheckResponse {
        ip: snapshot.ip,
        country: snapshot.country,
        region: snapshot.region,
        city: snapshot.city,
    }))
}

async fn runtime_status_snapshot<C: AdminApiContext>(ctx: &C) -> RuntimeStatusResponse {
    let running = ctx.runtime_running().await;
    let controller = ctx.runtime_controller_url().await;
    let mode = if running {
        match ctx.runtime_gateway().await {
            Ok(client) => client
                .get_config()
                .await
                .ok()
                .map(|cfg| normalize_proxy_mode(&cfg.mode)),
            Err(_) => None,
        }
    } else {
        None
    };
    RuntimeStatusResponse {
        running,
        controller,
        mode,
    }
}

fn normalize_log_level(level: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(level) = level else {
        return Ok(None);
    };
    let trimmed = level.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let mut normalized = trimmed.to_ascii_lowercase();
    if normalized == "warn" {
        normalized = "warning".to_string();
    }
    if matches!(
        normalized.as_str(),
        "debug" | "info" | "warning" | "error" | "silent"
    ) {
        return Ok(Some(normalized));
    }

    Err(ApiError::bad_request(
        "日志级别仅支持 debug/info/warning/error/silent",
    ))
}
