//! System/meta endpoints: capability discovery, rebuild status, and the
//! admin event stream (`/admin/api/capabilities`, `/admin/api/rebuild/status`,
//! `/admin/api/events`).

use std::{convert::Infallible, time::Duration};

use axum::{
    Json,
    response::sse::{Event, KeepAlive, Sse}};
use log::warn;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

pub async fn get_capabilities_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<AdminCapabilities>, ApiError> {
    Ok(Json(AdminCapabilities {
        schema_version: 1,
        platform: std::env::consts::OS.to_string(),
        runtime: RuntimeCapabilitySet {
            status: true,
            lifecycle: true,
        },
        proxy: ProxyCapabilitySet {
            list: true,
            mode_switch: true,
            select: true,
        },
        settings: SettingsCapabilitySet {
            autostart: state.ctx.supports_autostart_control(),
            system_proxy: state.ctx.supports_system_proxy_control(),
        },
    }))
}

pub async fn get_rebuild_status_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Result<Json<RebuildStatusResponse>, ApiError> {
    Ok(Json(state.rebuild_status.snapshot()))
}

pub async fn stream_admin_events_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(state.events.subscribe()).filter_map(|event| {
        let payload = match event {
            Ok(event) => match serde_json::to_string(&event) {
                Ok(payload) => payload,
                Err(err) => {
                    warn!("failed to serialize admin event: {err}");
                    return None;
                }
            },
            Err(err) => {
                warn!("admin event stream lagged: {err}");
                return None;
            }
        };
        Some(Ok(Event::default().data(payload)))
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    )
}
