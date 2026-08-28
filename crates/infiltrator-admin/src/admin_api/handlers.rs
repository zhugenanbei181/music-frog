use std::{
    collections::HashSet,
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::anyhow;
use axum::{
    Json,
    body::Body,
    extract::{Path as AxumPath, Query as AxumQuery, State as AxumState},
    http::{Request, StatusCode},
    middleware::Next,
    response::{
        Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use chrono::Utc;
use infiltrator_http::{HttpClient, reqwest};
use log::{info, warn};
use mihomo_api::{ConnectionsResponse, MemoryData};
use serde::Deserialize;
use tokio_stream::{
    StreamExt,
    wrappers::{BroadcastStream, UnboundedReceiverStream},
};

use infiltrator_core::{
    ProfileDetail, ProfileInfo, config as core_config, dns, fake_ip, profiles as core_profiles,
    proxy_providers, rules, settings::WebDavConfig, sniffer, subscription as core_subscription,
    tun,
};
use mihomo_config::ConfigManager;
use mihomo_version::VersionManager;

use super::events::{
    AdminEvent, EVENT_CORE_CHANGED, EVENT_DNS_CHANGED, EVENT_FAKE_IP_CHANGED,
    EVENT_PROFILES_CHANGED, EVENT_PROXY_CHANGED, EVENT_PROXY_PROVIDERS_CHANGED,
    EVENT_RULE_PROVIDERS_CHANGED, EVENT_RULES_CHANGED, EVENT_RUNTIME_CHANGED,
    EVENT_SETTINGS_CHANGED, EVENT_SNIFFER_CHANGED, EVENT_TUN_CHANGED, EVENT_WEBDAV_SYNCED,
};
use super::models::*;
use super::state::{AdminApiContext, AdminApiState, RebuildStatus};

#[derive(Deserialize)]
struct IpApiResponse {
    ip: Option<String>,
    #[serde(rename = "country_name")]
    country_name: Option<String>,
    region: Option<String>,
    city: Option<String>,
}

const DEFAULT_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
const DEFAULT_DELAY_TIMEOUT_MS: u32 = 5000;
const MIN_DELAY_TIMEOUT_MS: u32 = 100;
const MAX_DELAY_TIMEOUT_MS: u32 = 60_000;

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

pub async fn get_capabilities_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
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

pub async fn get_runtime_status_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<RuntimeStatusResponse>, ApiError> {
    let status = runtime_status_snapshot(&state.ctx).await;
    Ok(Json(status))
}

pub async fn start_runtime_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
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
    AxumState(state): AxumState<AdminApiState<C>>,
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

pub async fn get_proxies_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<RuntimeProxiesResponse>, ApiError> {
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let config = client
        .get_config()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let proxies = client
        .get_proxies()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut groups: Vec<RuntimeProxyGroupEntry> = proxies
        .into_iter()
        .filter_map(|(name, info)| {
            if !info.is_group() {
                return None;
            }
            Some(RuntimeProxyGroupEntry {
                name,
                proxy_type: info.proxy_type().to_string(),
                current: info.now().map(|item| item.to_string()),
                all: info.all().map(|items| items.to_vec()).unwrap_or_default(),
            })
        })
        .collect();
    groups.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(Json(RuntimeProxiesResponse {
        mode: normalize_proxy_mode(&config.mode),
        groups,
    }))
}

pub async fn set_proxy_mode_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<ProxyModePayload>,
) -> Result<StatusCode, ApiError> {
    let mode = normalize_proxy_mode_candidate(&payload.mode)?;
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    client
        .patch_config(serde_json::json!({ "mode": mode }))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.events.publish(AdminEvent::new(EVENT_PROXY_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn select_proxy_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<ProxySelectPayload>,
) -> Result<StatusCode, ApiError> {
    let group = payload.group.trim();
    if group.is_empty() {
        return Err(ApiError::bad_request("策略组不能为空"));
    }
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(ApiError::bad_request("代理节点不能为空"));
    }

    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    client
        .switch_proxy(group, name)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    state.events.publish(AdminEvent::new(EVENT_PROXY_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn stream_admin_events_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
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

pub async fn list_runtime_connections_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<ConnectionsResponse>, ApiError> {
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let data = client
        .get_connections()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(data))
}

pub async fn close_all_runtime_connections_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<StatusCode, ApiError> {
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    client
        .close_all_connections()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn close_runtime_connection_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let connection_id = id.trim();
    if connection_id.is_empty() {
        return Err(ApiError::bad_request("连接 ID 不能为空"));
    }
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    client
        .close_connection(connection_id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn stream_runtime_logs_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    AxumQuery(query): AxumQuery<RuntimeLogsQuery>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let level = normalize_log_level(query.level.as_deref())?;
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let receiver = client
        .stream_logs(level.as_deref())
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let stream = UnboundedReceiverStream::new(receiver).filter_map(|message| {
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
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<RuntimeTrafficSnapshotResponse>, ApiError> {
    let client = state
        .ctx
        .runtime_client()
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
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<MemoryData>, ApiError> {
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let memory = client
        .get_memory()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(memory))
}

pub async fn get_runtime_ip_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<RuntimeIpCheckResponse>, ApiError> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|e| ApiError::internal(format!("build ip client failed: {e}")))?;
    let response = client
        .get("https://ipapi.co/json/")
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("ip check request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(ApiError::internal(format!(
            "ip check failed: {}",
            response.status()
        )));
    }
    let payload: IpApiResponse = response
        .json()
        .await
        .map_err(|e| ApiError::internal(format!("decode ip response failed: {e}")))?;
    let ip = payload
        .ip
        .ok_or_else(|| ApiError::internal("ip missing from response"))?;

    Ok(Json(RuntimeIpCheckResponse {
        ip,
        country: payload.country_name,
        region: payload.region,
        city: payload.city,
    }))
}

pub async fn list_runtime_proxy_delays_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<RuntimeProxyDelayNodesResponse>, ApiError> {
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let proxies = client
        .get_proxies()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let nodes = build_runtime_proxy_delay_nodes(proxies);

    Ok(Json(RuntimeProxyDelayNodesResponse {
        nodes,
        default_test_url: DEFAULT_DELAY_TEST_URL.to_string(),
        default_timeout_ms: DEFAULT_DELAY_TIMEOUT_MS,
    }))
}

pub async fn test_runtime_proxy_delay_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<RuntimeDelayTestPayload>,
) -> Result<Json<RuntimeDelayTestResponse>, ApiError> {
    let proxy = payload.proxy.trim();
    if proxy.is_empty() {
        return Err(ApiError::bad_request("代理节点不能为空"));
    }

    let test_url = normalize_delay_test_url(payload.test_url.as_deref())?;
    let timeout_ms = normalize_delay_timeout_ms(payload.timeout_ms);
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let delay_ms = client
        .test_delay(proxy, &test_url, timeout_ms)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(RuntimeDelayTestResponse {
        proxy: proxy.to_string(),
        delay_ms,
        tested_at: Utc::now().to_rfc3339(),
        test_url,
        timeout_ms,
    }))
}

pub async fn test_all_runtime_proxy_delays_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<RuntimeDelayBatchPayload>,
) -> Result<Json<RuntimeDelayBatchResponse>, ApiError> {
    let test_url = normalize_delay_test_url(payload.test_url.as_deref())?;
    let timeout_ms = normalize_delay_timeout_ms(payload.timeout_ms);
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let proxies = client
        .get_proxies()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut results = Vec::new();
    let candidates =
        collect_delay_test_candidates(payload.proxies.as_deref(), &proxies, &mut results);

    for proxy in candidates {
        match client.test_delay(&proxy, &test_url, timeout_ms).await {
            Ok(delay_ms) => results.push(RuntimeDelayBatchResult {
                proxy,
                delay_ms: Some(delay_ms),
                tested_at: Some(Utc::now().to_rfc3339()),
                error: None,
            }),
            Err(err) => results.push(RuntimeDelayBatchResult {
                proxy,
                delay_ms: None,
                tested_at: None,
                error: Some(err.to_string()),
            }),
        }
    }

    let success_count = results
        .iter()
        .filter(|item| item.delay_ms.is_some())
        .count();
    let failed_count = results.len().saturating_sub(success_count);

    Ok(Json(RuntimeDelayBatchResponse {
        results,
        success_count,
        failed_count,
        test_url,
        timeout_ms,
    }))
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
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
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
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
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
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(ProfileActionResponse {
        profile: info,
        rebuild_scheduled: true,
    }))
}

pub async fn delete_profile_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    AxumPath(name): AxumPath<String>,
) -> Result<StatusCode, ApiError> {
    let profile_name = ensure_valid_profile_name(&name)?;
    let manager = ConfigManager::new().map_err(|e| ApiError::internal(e.to_string()))?;
    manager
        .delete_profile(&profile_name)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(StatusCode::NO_CONTENT)
}

pub async fn set_profile_subscription_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
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
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
    Ok(Json(info))
}

pub async fn clear_profile_subscription_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
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
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
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
    if core_config::validate_yaml(content).is_err() {
        return Err(ApiError::bad_request("订阅内容不是有效的 YAML"));
    }
    manager
        .save(&profile_name, content)
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

    let rebuild_scheduled = manager.get_current().await.ok().as_deref() == Some(&profile_name);
    if rebuild_scheduled {
        schedule_rebuild(&state.ctx, &state.rebuild_status, "subscription-update-now");
    }
    let profile = core_profiles::load_profile_info(&profile_name).await?;
    state
        .events
        .publish(AdminEvent::new(EVENT_PROFILES_CHANGED));
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
    state
        .events
        .publish(AdminEvent::new(EVENT_SETTINGS_CHANGED));
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
    state
        .ctx
        .open_profile_in_editor(&name)
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

pub async fn get_app_settings_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
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
        open_webui_on_startup: Some(settings.open_webui_on_startup),
        editor_path: settings.editor_path,
        use_bundled_core: Some(settings.use_bundled_core),
        language: Some(settings.language),
        theme: Some(settings.theme),
        webdav: Some(settings.webdav),
        autostart_enabled,
        system_proxy_enabled,
        runtime_running,
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
    if let Some(val) = payload.webdav {
        settings.webdav = val;
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

pub async fn get_dns_config_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<dns::DnsConfig>, ApiError> {
    let config = dns::load_dns_config().await?;
    Ok(Json(config))
}

pub async fn save_dns_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<dns::DnsConfigPatch>,
) -> Result<Json<dns::DnsConfig>, ApiError> {
    let config = dns::save_dns_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "dns-update");
    state.events.publish(AdminEvent::new(EVENT_DNS_CHANGED));
    Ok(Json(config))
}

pub async fn get_fake_ip_config_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<fake_ip::FakeIpConfig>, ApiError> {
    let config = fake_ip::load_fake_ip_config().await?;
    Ok(Json(config))
}

pub async fn save_fake_ip_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<fake_ip::FakeIpConfigPatch>,
) -> Result<Json<fake_ip::FakeIpConfig>, ApiError> {
    let config = fake_ip::save_fake_ip_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "fake-ip-update");
    state.events.publish(AdminEvent::new(EVENT_FAKE_IP_CHANGED));
    Ok(Json(config))
}

pub async fn flush_fake_ip_cache_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<CacheFlushResponse>, ApiError> {
    let removed = fake_ip::clear_fake_ip_cache().await?;
    Ok(Json(CacheFlushResponse { removed }))
}

pub async fn get_rule_providers_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<rules::RuleProvidersPayload>, ApiError> {
    let providers = rules::load_rule_providers().await?;
    Ok(Json(rules::RuleProvidersPayload { providers }))
}

pub async fn save_rule_providers_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<rules::RuleProvidersPayload>,
) -> Result<Json<rules::RuleProvidersPayload>, ApiError> {
    let providers = rules::save_rule_providers(payload.providers).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "rule-providers-update");
    state
        .events
        .publish(AdminEvent::new(EVENT_RULE_PROVIDERS_CHANGED));
    Ok(Json(rules::RuleProvidersPayload { providers }))
}

pub async fn get_proxy_providers_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<proxy_providers::ProxyProvidersPayload>, ApiError> {
    let providers = proxy_providers::load_proxy_providers().await?;
    Ok(Json(proxy_providers::ProxyProvidersPayload { providers }))
}

pub async fn save_proxy_providers_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<proxy_providers::ProxyProvidersPayload>,
) -> Result<Json<proxy_providers::ProxyProvidersPayload>, ApiError> {
    let providers = proxy_providers::save_proxy_providers(payload.providers).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "proxy-providers-update");
    state
        .events
        .publish(AdminEvent::new(EVENT_PROXY_PROVIDERS_CHANGED));
    Ok(Json(proxy_providers::ProxyProvidersPayload { providers }))
}

pub async fn get_sniffer_config_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = sniffer::load_sniffer_config().await?;
    Ok(Json(config))
}

pub async fn save_sniffer_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let config = sniffer::save_sniffer_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "sniffer-update");
    state.events.publish(AdminEvent::new(EVENT_SNIFFER_CHANGED));
    Ok(Json(config))
}

pub async fn get_rules_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<rules::RulesPayload>, ApiError> {
    let rules_list = rules::load_rules().await?;
    Ok(Json(rules::RulesPayload { rules: rules_list }))
}

pub async fn save_rules_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<rules::RulesPayload>,
) -> Result<Json<rules::RulesPayload>, ApiError> {
    let rules_list = rules::save_rules(payload.rules).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "rules-update");
    state.events.publish(AdminEvent::new(EVENT_RULES_CHANGED));
    Ok(Json(rules::RulesPayload { rules: rules_list }))
}

pub async fn get_tun_config_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
) -> Result<Json<tun::TunConfig>, ApiError> {
    let config = tun::load_tun_config().await?;
    Ok(Json(config))
}

pub async fn save_tun_config_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
    Json(payload): Json<tun::TunConfigPatch>,
) -> Result<Json<tun::TunConfig>, ApiError> {
    let config = tun::save_tun_config(payload).await?;
    schedule_rebuild(&state.ctx, &state.rebuild_status, "tun-update");
    state.events.publish(AdminEvent::new(EVENT_TUN_CHANGED));
    Ok(Json(config))
}

pub async fn sync_webdav_now_http<C: AdminApiContext>(
    AxumState(state): AxumState<AdminApiState<C>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let settings = state.ctx.get_app_settings().await;
    if !settings.webdav.enabled {
        return Err(ApiError::bad_request("WebDAV 同步未开启"));
    }

    // 手动触发同步逻辑
    let summary = crate::scheduler::sync::run_sync_tick(&state.ctx, &settings.webdav)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    state.events.publish(AdminEvent::new(EVENT_WEBDAV_SYNCED));

    Ok(Json(serde_json::json!({
        "success_count": summary.success_count,
        "failed_count": summary.failed_count,
        "total_actions": summary.total_actions,
    })))
}

pub async fn test_webdav_conn_http<C: AdminApiContext>(
    AxumState(_state): AxumState<AdminApiState<C>>,
    Json(payload): Json<WebDavConfig>,
) -> Result<StatusCode, ApiError> {
    use dav_client::DavClient;
    use dav_client::client::WebDavClient;

    let dav = WebDavClient::new(&payload.url, &payload.username, &payload.password)
        .map_err(|e| ApiError::bad_request(format!("无效的配置: {e}")))?;

    // 尝试 list 根目录来测试连接
    dav.list("/")
        .await
        .map_err(|e| ApiError::bad_request(format!("连接测试失败: {e}")))?;

    Ok(StatusCode::OK)
}

async fn runtime_status_snapshot<C: AdminApiContext>(ctx: &C) -> RuntimeStatusResponse {
    let running = ctx.runtime_running().await;
    let controller = ctx.runtime_controller_url().await;
    let mode = if running {
        match ctx.runtime_client().await {
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

fn normalize_proxy_mode(mode: &str) -> String {
    let trimmed = mode.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        "rule".to_string()
    } else {
        trimmed
    }
}

fn normalize_proxy_mode_candidate(mode: &str) -> Result<String, ApiError> {
    let normalized = normalize_proxy_mode(mode);
    if !matches!(
        normalized.as_str(),
        "rule" | "global" | "direct" | "script"
    ) {
        return Err(ApiError::bad_request(
            "代理模式仅支持 rule/global/direct/script",
        ));
    }
    Ok(normalized)
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

fn normalize_delay_test_url(test_url: Option<&str>) -> Result<String, ApiError> {
    let candidate = test_url.unwrap_or(DEFAULT_DELAY_TEST_URL).trim();
    if candidate.is_empty() {
        return Err(ApiError::bad_request("测速地址不能为空"));
    }
    Ok(candidate.to_string())
}

fn normalize_delay_timeout_ms(timeout_ms: Option<u32>) -> u32 {
    timeout_ms
        .unwrap_or(DEFAULT_DELAY_TIMEOUT_MS)
        .clamp(MIN_DELAY_TIMEOUT_MS, MAX_DELAY_TIMEOUT_MS)
}

fn build_runtime_proxy_delay_nodes(
    proxies: std::collections::HashMap<String, mihomo_api::Proxy>,
) -> Vec<RuntimeProxyDelayNode> {
    let mut nodes: Vec<RuntimeProxyDelayNode> = proxies
        .into_iter()
        .filter_map(|(name, info)| {
            if info.is_group() {
                return None;
            }
            let latest = info.history().last();
            Some(RuntimeProxyDelayNode {
                name,
                proxy_type: info.proxy_type().to_string(),
                delay_ms: latest.map(|item| item.delay),
                tested_at: latest.map(|item| item.time.clone()),
            })
        })
        .collect();
    nodes.sort_by(|left, right| left.name.cmp(&right.name));
    nodes
}

fn collect_delay_test_candidates(
    requested: Option<&[String]>,
    proxies: &std::collections::HashMap<String, mihomo_api::Proxy>,
    results: &mut Vec<RuntimeDelayBatchResult>,
) -> Vec<String> {
    match requested {
        Some(requested_list) => {
            let mut candidates = Vec::new();
            let mut seen = HashSet::new();
            for raw_name in requested_list {
                let trimmed = raw_name.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let name = trimmed.to_string();
                if !seen.insert(name.clone()) {
                    continue;
                }
                let Some(info) = proxies.get(trimmed) else {
                    results.push(RuntimeDelayBatchResult {
                        proxy: name,
                        delay_ms: None,
                        tested_at: None,
                        error: Some("节点不存在".to_string()),
                    });
                    continue;
                };
                if info.is_group() {
                    results.push(RuntimeDelayBatchResult {
                        proxy: name,
                        delay_ms: None,
                        tested_at: None,
                        error: Some("不支持策略组测速，请选择具体节点".to_string()),
                    });
                    continue;
                }
                candidates.push(name);
            }
            candidates
        }
        None => {
            let mut candidates: Vec<String> = proxies
                .iter()
                .filter_map(|(name, info)| {
                    if info.is_group() {
                        return None;
                    }
                    Some(name.clone())
                })
                .collect();
            candidates.sort();
            candidates
        }
    }
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
    client: &HttpClient,
    raw_client: &HttpClient,
    name: &str,
    url: &str,
    activate: bool,
) -> anyhow::Result<(ProfileInfo, bool)> {
    let profile_name = core_profiles::sanitize_profile_name(name)?;
    let source_url = url.trim();
    if source_url.is_empty() {
        return Err(anyhow!("订阅链接不能为空"));
    }

    let masked_url = core_subscription::mask_subscription_url(source_url);
    info!(
        "admin import profile start: name={} url={}",
        profile_name, masked_url
    );
    let content =
        core_subscription::fetch_subscription_text(client, raw_client, source_url).await?;
    if content.trim().is_empty() {
        return Err(anyhow!("订阅返回内容为空"));
    }
    let content = core_subscription::strip_utf8_bom(&content);
    if core_config::validate_yaml(content).is_err() {
        return Err(anyhow!("订阅内容不是有效的 YAML"));
    }

    let manager = ConfigManager::new()?;
    manager.save(&profile_name, content).await?;

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
    manager
        .update_profile_metadata(&profile_name, &metadata)
        .await?;

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

fn sort_versions_desc(list: &mut [String]) {
    list.sort_by(|a, b| compare_versions_desc(a, b));
}

fn compare_versions_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let va = parse_version(a);
    let vb = parse_version(b);
    match (va, vb) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let trimmed = version.trim().trim_start_matches('v');
    let core = trimmed.split('-').next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

#[cfg(test)]
#[path = "handlers_test.rs"]
mod handlers_test;
