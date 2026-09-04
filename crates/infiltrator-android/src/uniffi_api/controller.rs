//! Live mihomo controller passthrough: proxy groups and selection, runtime
//! mode patch, traffic snapshots, connection listing/closing, and outbound
//! IP check. Every call resolves the controller through
//! [`super::support::build_controller_client`].

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::support::{build_controller_client, get_runtime, map_mihomo_error};
use crate::ffi::{FfiErrorCode, FfiStatus};

// --- Proxies API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProxyGroupSummary {
    pub name: String,
    pub group_type: String,
    pub current: Option<String>,
    pub all: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProxyGroupsResult {
    pub status: FfiStatus,
    pub groups: Vec<ProxyGroupSummary>,
}

// --- Status/Traffic API ---

#[derive(Debug, Clone, uniffi::Record)]
pub struct TrafficSnapshot {
    pub up_rate: u64,
    pub down_rate: u64,
    pub up_total: u64,
    pub down_total: u64,
    pub up_peak: u64,
    pub down_peak: u64,
    pub connections: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct TrafficResult {
    pub status: FfiStatus,
    pub snapshot: Option<TrafficSnapshot>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ConnectionRecord {
    pub id: String,
    pub host: String,
    pub process_path: String,
    pub network: String,
    pub connection_type: String,
    pub rule: String,
    pub upload: u64,
    pub download: u64,
    pub chains: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ConnectionsResult {
    pub status: FfiStatus,
    pub connections: Vec<ConnectionRecord>,
    pub upload_total: u64,
    pub download_total: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct IpCheckResult {
    pub ip: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct IpResult {
    pub status: FfiStatus,
    pub value: Option<IpCheckResult>,
}

#[derive(Deserialize)]
struct IpApiResponse {
    ip: Option<String>,
    #[serde(rename = "country_name")]
    country_name: Option<String>,
    region: Option<String>,
    city: Option<String>,
}

struct TrafficState {
    last_up: u64,
    last_down: u64,
    last_at: Instant,
    peak_up_rate: u64,
    peak_down_rate: u64,
    initialized: bool,
}

impl TrafficState {
    fn new() -> Self {
        Self {
            last_up: 0,
            last_down: 0,
            last_at: Instant::now(),
            peak_up_rate: 0,
            peak_down_rate: 0,
            initialized: false,
        }
    }
}

#[uniffi::export]
pub async fn proxies_groups() -> ProxyGroupsResult {
    get_runtime()
        .spawn(async move {
            match proxies_groups_internal().await {
                Ok(groups) => ProxyGroupsResult {
                    status: FfiStatus::ok(),
                    groups,
                },
                Err(status) => ProxyGroupsResult {
                    status,
                    groups: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| ProxyGroupsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            groups: Vec::new(),
        })
}

#[uniffi::export]
pub async fn proxy_select(group: String, server: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match proxy_select_internal(&group, &server).await {
                Ok(_) => FfiStatus::ok(),
                Err(status) => status,
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

// --- Config/Mode API ---

#[uniffi::export]
pub async fn config_patch_mode(mode: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match config_patch_mode_internal(&mode).await {
                Ok(_) => FfiStatus::ok(),
                Err(status) => status,
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn traffic_snapshot() -> TrafficResult {
    get_runtime()
        .spawn(async move {
            match traffic_snapshot_internal().await {
                Ok(snapshot) => TrafficResult {
                    status: FfiStatus::ok(),
                    snapshot: Some(snapshot),
                },
                Err(status) => TrafficResult {
                    status,
                    snapshot: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| TrafficResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            snapshot: None,
        })
}

#[uniffi::export]
pub async fn ip_check() -> IpResult {
    get_runtime()
        .spawn(async move {
            match fetch_ip_check().await {
                Ok(value) => IpResult {
                    status: FfiStatus::ok(),
                    value: Some(value),
                },
                Err(status) => IpResult {
                    status,
                    value: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| IpResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            value: None,
        })
}

#[uniffi::export]
pub async fn connections_list() -> ConnectionsResult {
    get_runtime()
        .spawn(async move {
            let client = match build_controller_client().await {
                Ok(client) => client,
                Err(status) => {
                    return ConnectionsResult {
                        status,
                        connections: Vec::new(),
                        upload_total: 0,
                        download_total: 0,
                    };
                }
            };
            match client.get_connections().await.map_err(map_mihomo_error) {
                Ok(response) => ConnectionsResult {
                    status: FfiStatus::ok(),
                    connections: response
                        .connections
                        .into_iter()
                        .map(|connection| connection_to_record(connection.into()))
                        .collect(),
                    upload_total: response.upload_total,
                    download_total: response.download_total,
                },
                Err(status) => ConnectionsResult {
                    status,
                    connections: Vec::new(),
                    upload_total: 0,
                    download_total: 0,
                },
            }
        })
        .await
        .unwrap_or_else(|e| ConnectionsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            connections: Vec::new(),
            upload_total: 0,
            download_total: 0,
        })
}

#[uniffi::export]
pub async fn connection_close(id: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let connection_id = id.trim().to_string();
            if connection_id.is_empty() {
                return FfiStatus::err(FfiErrorCode::InvalidInput, "connection id is empty");
            }
            let client = match build_controller_client().await {
                Ok(client) => client,
                Err(status) => return status,
            };
            client
                .close_connection(&connection_id)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_mihomo_error)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn connections_close_all() -> FfiStatus {
    get_runtime()
        .spawn(async move {
            let client = match build_controller_client().await {
                Ok(client) => client,
                Err(status) => return status,
            };
            client
                .close_all_connections()
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_mihomo_error)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

// --- Internal Helpers ---

async fn proxies_groups_internal() -> Result<Vec<ProxyGroupSummary>, FfiStatus> {
    let client = build_controller_client().await?;
    let proxies = client.get_proxies().await.map_err(map_mihomo_error)?;
    let mut groups: Vec<ProxyGroupSummary> = proxies
        .into_iter()
        .filter_map(|(name, info)| {
            info.all().map(|all| ProxyGroupSummary {
                name,
                group_type: info.proxy_type().to_string(),
                current: Some(info.now().unwrap_or("").to_string()),
                all: all.to_vec(),
            })
        })
        .collect();
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(groups)
}

async fn proxy_select_internal(group: &str, server: &str) -> Result<(), FfiStatus> {
    let client = build_controller_client().await?;
    client
        .switch_proxy(group, server)
        .await
        .map_err(map_mihomo_error)
}

async fn config_patch_mode_internal(mode: &str) -> Result<(), FfiStatus> {
    let client = build_controller_client().await?;
    // We create a partial config JSON to patch just the mode
    let patch = serde_json::json!({ "mode": mode });
    client.patch_config(patch).await.map_err(map_mihomo_error)
}

async fn traffic_snapshot_internal() -> Result<TrafficSnapshot, FfiStatus> {
    let client = build_controller_client().await?;
    let snapshot = client.get_connections().await.map_err(map_mihomo_error)?;
    Ok(build_traffic_snapshot(
        snapshot.upload_total,
        snapshot.download_total,
        snapshot.connections.len(),
    ))
}

fn connection_to_record(connection: infiltrator_domain::runtime::Connection) -> ConnectionRecord {
    ConnectionRecord {
        id: connection.id,
        host: connection.metadata.host,
        process_path: connection.metadata.process_path,
        network: connection.metadata.network,
        connection_type: connection.metadata.connection_type,
        rule: connection.rule,
        upload: connection.upload,
        download: connection.download,
        chains: connection.chains,
    }
}

fn build_traffic_snapshot(up_total: u64, down_total: u64, connections: usize) -> TrafficSnapshot {
    let state = traffic_state();
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let now = Instant::now();
    let elapsed = now.duration_since(guard.last_at);
    let elapsed_secs = elapsed.as_secs_f64();
    let mut up_rate = 0;
    let mut down_rate = 0;
    if guard.initialized && elapsed_secs > 0.0 {
        let up_delta = up_total.saturating_sub(guard.last_up);
        let down_delta = down_total.saturating_sub(guard.last_down);
        up_rate = ((up_delta as f64) / elapsed_secs).round() as u64;
        down_rate = ((down_delta as f64) / elapsed_secs).round() as u64;
        guard.peak_up_rate = guard.peak_up_rate.max(up_rate);
        guard.peak_down_rate = guard.peak_down_rate.max(down_rate);
    } else {
        guard.initialized = true;
    }
    guard.last_up = up_total;
    guard.last_down = down_total;
    guard.last_at = now;

    TrafficSnapshot {
        up_rate,
        down_rate,
        up_total,
        down_total,
        up_peak: guard.peak_up_rate,
        down_peak: guard.peak_down_rate,
        connections: connections as u32,
    }
}

async fn fetch_ip_check() -> Result<IpCheckResult, FfiStatus> {
    crate::tls::ensure_rustls_provider();
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|err| map_reqwest_error("build ip client", err))?;
    let resp = client
        .get("https://ipapi.co/json/")
        .send()
        .await
        .map_err(|err| map_reqwest_error("fetch ip", err))?;
    if !resp.status().is_success() {
        return Err(FfiStatus::err(
            FfiErrorCode::Network,
            format!("ip check failed: {}", resp.status()),
        ));
    }
    let body: IpApiResponse = resp
        .json()
        .await
        .map_err(|err| map_reqwest_error("decode ip response", err))?;
    let ip = body
        .ip
        .ok_or_else(|| FfiStatus::err(FfiErrorCode::InvalidState, "ip missing from response"))?;
    Ok(IpCheckResult {
        ip,
        country: body.country_name,
        region: body.region,
        city: body.city,
    })
}

fn traffic_state() -> &'static Mutex<TrafficState> {
    static TRAFFIC_STATE: OnceLock<Mutex<TrafficState>> = OnceLock::new();
    TRAFFIC_STATE.get_or_init(|| Mutex::new(TrafficState::new()))
}

fn map_reqwest_error(context: &str, err: reqwest::Error) -> FfiStatus {
    let message = format!("{context}: {err}");
    FfiStatus::err(FfiErrorCode::Network, message)
}
