//! Runtime lifecycle, connections, and streaming telemetry endpoints
//! (`/admin/api/runtime/*` except proxies/delay).

use crate::admin_api::events::{AdminEvent, EVENT_RUNTIME_CHANGED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState, CoreOwnerAction};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{Json, extract};
use futures_util::StreamExt;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::routing_intent::RoutingIntentSnapshot;
use infiltrator_domain::runtime::ConnectionsResponse;
use infiltrator_ports::runtime_stream::{RuntimeStreamEvent, RuntimeStreamTerminal};
use log::warn;
use std::convert::Infallible;
use std::time::Duration;

pub async fn get_runtime_status_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn start_runtime_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    state
        .ctx
        .apply_core_owner_action(CoreOwnerAction::StartCore)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn stop_runtime_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    state
        .ctx
        .apply_core_owner_action(CoreOwnerAction::StopCore)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.events.publish(AdminEvent::new(EVENT_RUNTIME_CHANGED));
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn list_runtime_connections_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<ConnectionsResponse>, ApiError> {
    let application = state
        .ctx
        .connection_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let observation = application
        .snapshot()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let snapshot = observation.snapshot();
    let data = ConnectionsResponse {
        download_total: snapshot.download_total,
        upload_total: snapshot.upload_total,
        connections: snapshot.connections.clone(),
    };
    Ok(Json(data))
}

pub async fn close_all_runtime_connections_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<StatusCode, ApiError> {
    let application = state
        .ctx
        .connection_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    application
        .close_all()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn close_runtime_connection_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    extract::Path(id): extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    let connection_id = id.trim();
    if connection_id.is_empty() {
        return Err(ApiError::bad_request("连接 ID 不能为空"));
    }
    let application = state
        .ctx
        .connection_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    application
        .close(connection_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn stream_runtime_logs_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
    extract::Query(query): extract::Query<RuntimeLogsQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let level = normalize_log_level(query.level.as_deref())?;
    let application = state
        .ctx
        .runtime_query_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let receiver = application
        .logs(level)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let stream = receiver.filter_map(|event| async move {
        let (event_name, payload) = match event.event {
            RuntimeStreamEvent::Connecting => ("connecting", String::new()),
            RuntimeStreamEvent::Connected => ("connected", String::new()),
            RuntimeStreamEvent::Item(message) => {
                match serde_json::to_string(&RuntimeLogEvent { message }) {
                    Ok(payload) => ("message", payload),
                    Err(err) => {
                        warn!("failed to serialize runtime log event: {err}");
                        return None;
                    }
                }
            }
            RuntimeStreamEvent::Reconnecting(failure) => ("reconnecting", failure.to_string()),
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Cancelled) => {
                ("cancelled", String::new())
            }
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Failed(failure)) => {
                ("failed", failure.to_string())
            }
            RuntimeStreamEvent::Terminal(RuntimeStreamTerminal::Unsupported(failure)) => {
                ("unsupported", failure.to_string())
            }
        };
        Some(Ok(Event::default().event(event_name).data(payload)))
    });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}

pub async fn get_runtime_traffic_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeTrafficSnapshotResponse>, ApiError> {
    let application = state
        .ctx
        .connection_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let observation = application
        .snapshot()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let connections = observation.snapshot();
    let snapshot = state.traffic_snapshot(
        connections.upload_total,
        connections.download_total,
        connections.connections.len(),
    );
    Ok(Json(snapshot))
}

pub async fn get_runtime_memory_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<CoreResourceSnapshot>, ApiError> {
    let application = state
        .ctx
        .resource_application()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let snapshot = application
        .observe()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(snapshot))
}

pub async fn get_runtime_ip_http<C: AdminApiContext>(
    extract::State(state): extract::State<AdminApiState<C>>,
) -> Result<Json<RuntimeIpCheckResponse>, ApiError> {
    let snapshot = state
        .ctx
        .network_application()
        .await
        .map_err(|error| ApiError::internal(error.to_string()))?
        .probe_public_ip_direct()
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
    let routing_intent = if running {
        ctx.routing_intent_snapshot()
            .await
            .ok()
            .map(|(_, snapshot)| snapshot)
            .unwrap_or_default()
    } else {
        RoutingIntentSnapshot::default()
    };
    RuntimeStatusResponse {
        running,
        controller,
        routing_intent,
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
