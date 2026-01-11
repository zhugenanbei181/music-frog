use crate::ffi::{FfiErrorCode, FfiStatus};
use infiltrator_core::profiles::{
    create_profile_from_url, list_profile_infos, select_profile as core_select_profile,
    update_profile as core_update_profile, ProfileInfo,
};
use mihomo_api::{MihomoClient, MihomoError};
use mihomo_platform::{clear_android_bridge, get_android_bridge};
use serde::Deserialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

fn get_runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("failed to create tokio runtime"))
}

#[uniffi::export]
pub fn ping() -> String {
    "ok".to_string()
}

#[uniffi::export]
pub fn bridge_ready() -> bool {
    get_android_bridge().is_some()
}

#[uniffi::export]
pub fn bridge_shutdown() -> FfiStatus {
    clear_android_bridge();
    FfiStatus::ok()
}

#[uniffi::export]
pub fn start_vpn(fd: i32) -> FfiStatus {
    log::info!("Rust received VPN File Descriptor: {}", fd);

    // Launch tun2proxy in a background thread.
    // We assume the SOCKS5 proxy is at 127.0.0.1:7891 (default in MihomoHost).
    #[cfg(target_os = "android")]
    {
        let proxy_url = "socks5://127.0.0.1:7891";
        let proxy = match tun2proxy::ArgProxy::try_from(proxy_url) {
            Ok(proxy) => proxy,
            Err(err) => {
                return FfiStatus::err(FfiErrorCode::InvalidInput, err.to_string());
            }
        };

        let mut args = tun2proxy::Args::default();
        args.proxy = proxy;
        args.tun_fd = Some(fd);
        args.close_fd_on_drop = Some(true);
        let mtu = 1500;

        std::thread::spawn(move || {
            log::info!("Starting tun2proxy for FD {} to {}", fd, proxy_url);
            let exit_code = tun2proxy::mobile_run(args, mtu, false);
            if exit_code != 0 {
                log::error!("tun2proxy exited with code {}", exit_code);
            }
        });
    }

    #[cfg(not(target_os = "android"))]
    log::warn!("start_vpn called on non-Android target");

    FfiStatus::ok()
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfileSummary {
    pub name: String,
    pub active: bool,
    pub subscription_url: Option<String>,
    pub auto_update_enabled: bool,
    pub update_interval_hours: Option<u32>,
    pub last_updated: Option<String>,
    pub next_update: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfilesResult {
    pub status: FfiStatus,
    pub profiles: Vec<ProfileSummary>,
}

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

#[derive(Debug, Clone, uniffi::Record)]
pub struct TunStatusResult {
    pub status: FfiStatus,
    pub enabled: bool,
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

// --- Profiles API ---

#[uniffi::export]
pub async fn profiles_list() -> ProfilesResult {
    get_runtime()
        .spawn(async move {
            match list_profile_infos().await.map_err(map_anyhow_error) {
                Ok(profiles) => ProfilesResult {
                    status: FfiStatus::ok(),
                    profiles: profiles.into_iter().map(profile_to_summary).collect(),
                },
                Err(status) => ProfilesResult {
                    status,
                    profiles: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| ProfilesResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            profiles: Vec::new(),
        })
}

#[uniffi::export]
pub async fn profile_create(name: String, url: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match create_profile_from_url(&name, &url).await {
                Ok(_) => FfiStatus::ok(),
                Err(err) => map_anyhow_error(err),
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_select(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match core_select_profile(&name).await {
                Ok(_) => {
                    // After switching profiles, we should restart the core if it's running
                    // to apply the new config.
                    if let Some(bridge) = get_android_bridge() {
                        if let Ok(true) = bridge.core_is_running().await {
                            let _ = bridge.core_stop().await;
                            let _ = bridge.core_start().await;
                        }
                    }
                    FfiStatus::ok()
                }
                Err(err) => map_anyhow_error(err),
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn profile_update(name: String) -> FfiStatus {
    get_runtime()
        .spawn(async move {
            match core_update_profile(&name).await {
                Ok(_) => FfiStatus::ok(),
                Err(err) => map_anyhow_error(err),
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

// --- Proxies API ---

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

// --- Status/Traffic API ---

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
                Err(status) => IpResult { status, value: None },
            }
        })
        .await
        .unwrap_or_else(|e| IpResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            value: None,
        })
}

#[uniffi::export]
pub async fn tun_status() -> TunStatusResult {
    get_runtime()
        .spawn(async move {
            match tun_status_internal().await {
                Ok(enabled) => TunStatusResult {
                    status: FfiStatus::ok(),
                    enabled,
                },
                Err(status) => TunStatusResult {
                    status,
                    enabled: false,
                },
            }
        })
        .await
        .unwrap_or_else(|e| TunStatusResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            enabled: false,
        })
}

// --- Internal Helpers ---

async fn proxies_groups_internal() -> Result<Vec<ProxyGroupSummary>, FfiStatus> {
    let client = build_controller_client().await?;
    let proxies = client.get_proxies().await.map_err(map_mihomo_error)?;
    let mut groups: Vec<ProxyGroupSummary> = proxies
        .into_iter()
        .filter_map(|(name, info)| {
            info.all.map(|all| ProxyGroupSummary {
                name,
                group_type: info.proxy_type,
                current: info.now,
                all,
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
    client
        .patch_config(patch)
        .await
        .map_err(map_mihomo_error)
}

async fn traffic_snapshot_internal() -> Result<TrafficSnapshot, FfiStatus> {
    let client = build_controller_client().await?;
    let snapshot = client
        .get_connections()
        .await
        .map_err(map_mihomo_error)?;
    Ok(build_traffic_snapshot(
        snapshot.upload_total,
        snapshot.download_total,
        snapshot.connections.len(),
    ))
}

async fn tun_status_internal() -> Result<bool, FfiStatus> {
    let bridge = get_android_bridge().ok_or_else(|| {
        FfiStatus::err(FfiErrorCode::NotReady, "android bridge not ready")
    })?;
    let enabled = bridge
        .tun_is_enabled()
        .await
        .map_err(map_mihomo_error)?;
    Ok(enabled)
}

fn profile_to_summary(profile: ProfileInfo) -> ProfileSummary {
    ProfileSummary {
        name: profile.name,
        active: profile.active,
        subscription_url: profile.subscription_url,
        auto_update_enabled: profile.auto_update_enabled,
        update_interval_hours: profile.update_interval_hours,
        last_updated: profile.last_updated.map(|value| value.to_rfc3339()),
        next_update: profile.next_update.map(|value| value.to_rfc3339()),
    }
}

fn build_traffic_snapshot(
    up_total: u64,
    down_total: u64,
    connections: usize,
) -> TrafficSnapshot {
    let state = traffic_state();
    let mut guard = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
    let ip = body.ip.ok_or_else(|| {
        FfiStatus::err(FfiErrorCode::InvalidState, "ip missing from response")
    })?;
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

async fn build_controller_client() -> Result<MihomoClient, FfiStatus> {
    let bridge = get_android_bridge().ok_or_else(|| {
        FfiStatus::err(FfiErrorCode::NotReady, "android bridge not ready")
    })?;
    let controller_url = bridge.core_controller_url().ok_or_else(|| {
        FfiStatus::err(FfiErrorCode::NotReady, "controller url not ready")
    })?;
    MihomoClient::new(&controller_url, None).map_err(map_mihomo_error)
}

fn map_anyhow_error(err: anyhow::Error) -> FfiStatus {
    if let Some(source) = err.downcast_ref::<MihomoError>() {
        return map_mihomo_error_ref(source);
    }
    FfiStatus::err(FfiErrorCode::Unknown, err.to_string())
}

fn map_mihomo_error(err: MihomoError) -> FfiStatus {
    map_mihomo_error_ref(&err)
}

fn map_mihomo_error_ref(err: &MihomoError) -> FfiStatus {
    match err {
        MihomoError::Http(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Io(_) => FfiStatus::err(FfiErrorCode::Io, err.to_string()),
        MihomoError::Json(_)
        | MihomoError::Yaml(_)
        | MihomoError::YamlEmit(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::UrlParse(_) => FfiStatus::err(FfiErrorCode::InvalidInput, err.to_string()),
        MihomoError::WebSocket(_) => FfiStatus::err(FfiErrorCode::Network, err.to_string()),
        MihomoError::Config(_) | MihomoError::Service(_) | MihomoError::Version(_) => {
            FfiStatus::err(FfiErrorCode::InvalidState, err.to_string())
        }
        MihomoError::Proxy(_) | MihomoError::NotFound(_) => {
            FfiStatus::err(FfiErrorCode::NotReady, err.to_string())
        }
    }
}

fn map_reqwest_error(context: &str, err: reqwest::Error) -> FfiStatus {
    let message = format!("{context}: {err}");
    FfiStatus::err(FfiErrorCode::Network, message)
}
