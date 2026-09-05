//! Android controller surface. Concrete Mihomo HTTP details stay in the
//! composition helpers; this module only maps application results into FFI
//! records.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use infiltrator_contract::command::ProxyMode;

use crate::host_support::{
    build_connection_application, build_proxy_application, build_runtime_query_application,
    get_runtime, map_application_failure, network_application,
};
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
            let application = match build_connection_application().await {
                Ok(application) => application,
                Err(status) => {
                    return ConnectionsResult {
                        status,
                        connections: Vec::new(),
                        upload_total: 0,
                        download_total: 0,
                    };
                }
            };
            match application.snapshot().await.map_err(map_application_failure) {
                Ok(response) => ConnectionsResult {
                    status: FfiStatus::ok(),
                    connections: response
                        .connections
                        .into_iter()
                        .map(connection_to_record)
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
            let application = match build_connection_application().await {
                Ok(application) => application,
                Err(status) => return status,
            };
            application
                .close(&connection_id)
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_application_failure)
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
            let application = match build_connection_application().await {
                Ok(application) => application,
                Err(status) => return status,
            };
            application
                .close_all()
                .await
                .map(|_| FfiStatus::ok())
                .unwrap_or_else(map_application_failure)
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

// --- Internal Helpers ---

async fn proxies_groups_internal() -> Result<Vec<ProxyGroupSummary>, FfiStatus> {
    let application = build_proxy_application().await?;
    application
        .list_groups()
        .await
        .map(|groups| {
            groups
                .into_iter()
                .map(|group| ProxyGroupSummary {
                    name: group.name,
                    group_type: "group".to_string(),
                    current: Some(group.now),
                    all: group.all,
                })
                .collect()
        })
        .map_err(map_application_failure)
}

async fn proxy_select_internal(group: &str, server: &str) -> Result<(), FfiStatus> {
    build_proxy_application()
        .await?
        .switch(group, server)
        .await
        .map_err(map_application_failure)
}

async fn config_patch_mode_internal(mode: &str) -> Result<(), FfiStatus> {
    let mode = ProxyMode::from_wire(mode).ok_or_else(|| {
        FfiStatus::err(FfiErrorCode::InvalidInput, format!("unsupported proxy mode: {mode}"))
    })?;
    build_runtime_query_application()
        .await?
        .set_proxy_mode(mode)
        .await
        .map_err(map_application_failure)
}

async fn traffic_snapshot_internal() -> Result<TrafficSnapshot, FfiStatus> {
    let snapshot = build_connection_application()
        .await?
        .snapshot()
        .await
        .map_err(map_application_failure)?;
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
    let snapshot = network_application()
        .probe_public_ip(None)
        .await
        .map_err(map_application_failure)?;
    Ok(IpCheckResult {
        ip: snapshot.ip,
        country: snapshot.country,
        region: snapshot.region,
        city: snapshot.city,
    })
}

fn traffic_state() -> &'static Mutex<TrafficState> {
    static TRAFFIC_STATE: OnceLock<Mutex<TrafficState>> = OnceLock::new();
    TRAFFIC_STATE.get_or_init(|| Mutex::new(TrafficState::new()))
}
