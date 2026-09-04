//! Runtime proxy inspection/control and delay testing endpoints
//! (`/admin/api/proxies`, `/admin/api/proxy/*`, `/admin/api/runtime/proxies`,
//! `/admin/api/runtime/delay/*`).

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use axum::{Json, http::StatusCode, response::{IntoResponse, Response}};
use chrono::Utc;
use infiltrator_core::flow_control::BatchDelayTester;
use tokio::sync::watch;

use crate::admin_api::events::{AdminEvent, EVENT_PROXY_CHANGED};
use crate::admin_api::models::*;
use crate::admin_api::state::{AdminApiContext, AdminApiState};

pub(crate) const DEFAULT_DELAY_TEST_URL: &str = "http://www.gstatic.com/generate_204";
pub(crate) const DEFAULT_DELAY_TIMEOUT_MS: u32 = 5000;
const MIN_DELAY_TIMEOUT_MS: u32 = 100;
const MAX_DELAY_TIMEOUT_MS: u32 = 60_000;

pub async fn get_proxies_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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

pub async fn list_runtime_proxy_delays_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
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

    let tester = BatchDelayTester::new(
        30,
        test_url.clone(),
        Duration::from_millis(timeout_ms as u64),
    );
    let (_cancel_tx, cancel_rx) = watch::channel(false);
    let client_arc = Arc::new(client);

    let outcomes = tester
        .test_proxies(
            candidates,
            move |proxy, url| {
                let client = client_arc.clone();
                async move {
                    client
                        .test_delay(&proxy, &url, timeout_ms)
                        .await
                        .map(|d| d as u64)
                        .map_err(|e| e.to_string())
                }
            },
            cancel_rx,
        )
        .await;

    for outcome in outcomes {
        match outcome.result {
            Ok(res) => results.push(RuntimeDelayBatchResult {
                proxy: outcome.proxy_name,
                delay_ms: Some(res.latency_ms as u32),
                tested_at: Some(Utc::now().to_rfc3339()),
                error: None,
            }),
            Err(err) => results.push(RuntimeDelayBatchResult {
                proxy: outcome.proxy_name,
                delay_ms: None,
                tested_at: None,
                error: Some(format!("{:?}", err)),
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

pub async fn test_proxies_delay_http<C: AdminApiContext>(
    axum::extract::State(state): axum::extract::State<AdminApiState<C>>,
    payload: Option<Json<ProxyDelayPayload>>,
) -> Result<Response, ApiError> {
    let payload = payload.map(|Json(p)| p).unwrap_or_default();
    let test_url = normalize_delay_test_url(payload.test_url.as_deref())?;
    let timeout_ms = normalize_delay_timeout_ms(payload.timeout_ms);
    let client = state
        .ctx
        .runtime_client()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    // Check if single proxy test is requested
    let single_candidate = payload
        .proxy
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty() && *p != "all");
    if let Some(proxy_name) = single_candidate
        && payload.proxies.is_none()
        && payload.all != Some(true)
    {
        let delay_ms = client
            .test_delay(proxy_name, &test_url, timeout_ms)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        return Ok((
            StatusCode::OK,
            Json(RuntimeDelayTestResponse {
                proxy: proxy_name.to_string(),
                delay_ms,
                tested_at: Utc::now().to_rfc3339(),
                test_url,
                timeout_ms,
            }),
        )
            .into_response());
    }

    let proxies = client
        .get_proxies()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut results = Vec::new();
    let candidates =
        collect_delay_test_candidates(payload.proxies.as_deref(), &proxies, &mut results);

    let tester = BatchDelayTester::new(
        30,
        test_url.clone(),
        Duration::from_millis(timeout_ms as u64),
    );
    let (_cancel_tx, cancel_rx) = watch::channel(false);
    let client_arc = Arc::new(client);

    let outcomes = tester
        .test_proxies(
            candidates,
            move |proxy, url| {
                let client = client_arc.clone();
                async move {
                    client
                        .test_delay(&proxy, &url, timeout_ms)
                        .await
                        .map(|d| d as u64)
                        .map_err(|e| e.to_string())
                }
            },
            cancel_rx,
        )
        .await;

    for outcome in outcomes {
        match outcome.result {
            Ok(res) => results.push(RuntimeDelayBatchResult {
                proxy: outcome.proxy_name,
                delay_ms: Some(res.latency_ms as u32),
                tested_at: Some(Utc::now().to_rfc3339()),
                error: None,
            }),
            Err(err) => results.push(RuntimeDelayBatchResult {
                proxy: outcome.proxy_name,
                delay_ms: None,
                tested_at: None,
                error: Some(format!("{:?}", err)),
            }),
        }
    }

    let success_count = results
        .iter()
        .filter(|item| item.delay_ms.is_some())
        .count();
    let failed_count = results.len().saturating_sub(success_count);

    Ok((
        StatusCode::OK,
        Json(RuntimeDelayBatchResponse {
            results,
            success_count,
            failed_count,
            test_url,
            timeout_ms,
        }),
    )
        .into_response())
}

pub(super) fn normalize_proxy_mode(mode: &str) -> String {
    let trimmed = mode.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        "rule".to_string()
    } else {
        trimmed
    }
}

pub(crate) fn normalize_proxy_mode_candidate(mode: &str) -> Result<String, ApiError> {
    let normalized = normalize_proxy_mode(mode);
    if !matches!(normalized.as_str(), "rule" | "global" | "direct" | "script") {
        return Err(ApiError::bad_request(
            "代理模式仅支持 rule/global/direct/script",
        ));
    }
    Ok(normalized)
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
    proxies: std::collections::HashMap<String, infiltrator_domain::proxy::Proxy>,
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

pub(crate) fn collect_delay_test_candidates(
    requested: Option<&[String]>,
    proxies: &std::collections::HashMap<String, infiltrator_domain::proxy::Proxy>,
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
