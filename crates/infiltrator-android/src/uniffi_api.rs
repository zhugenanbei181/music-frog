use crate::ffi::{FfiBoolResult, FfiErrorCode, FfiStatus};
use dav_client::client::WebDavClient;
use dav_client::DavClient;
use infiltrator_core::app_routing::{
    load_app_routing, save_app_routing, set_routing_mode as core_set_routing_mode,
    toggle_package as core_toggle_package, AppRoutingConfig as CoreAppRoutingConfig,
    AppRoutingMode as CoreAppRoutingMode,
};
use infiltrator_core::dns::{
    load_dns_config, save_dns_config, DnsConfig as CoreDnsConfig,
    DnsConfigPatch as CoreDnsConfigPatch,
};
use infiltrator_core::fake_ip::{
    clear_fake_ip_cache, load_fake_ip_config, save_fake_ip_config,
    FakeIpConfig as CoreFakeIpConfig, FakeIpConfigPatch as CoreFakeIpConfigPatch,
};
use infiltrator_core::profiles::{
    create_profile_from_url, list_profile_infos, select_profile as core_select_profile,
    update_profile as core_update_profile, ProfileInfo,
};
use infiltrator_core::rules::{
    load_rule_providers, load_rules, save_rule_providers, save_rules,
    RuleEntry as CoreRuleEntry, RuleProviders as CoreRuleProviders,
};
use infiltrator_core::settings::{
    load_settings, save_settings, settings_path, AppSettings,
    WebDavConfig as CoreWebDavConfig,
};
use infiltrator_core::tun::{
    load_tun_config, save_tun_config, TunConfig as CoreTunConfig,
    TunConfigPatch as CoreTunConfigPatch,
};
use mihomo_config::ConfigManager;
use mihomo_api::{MihomoClient, MihomoError};
use mihomo_platform::{clear_android_bridge, get_android_bridge, get_home_dir};
use serde::Deserialize;
use serde_json::Value as JsonValue;
#[cfg(target_os = "android")]
use serde_yaml::Value;
use state_store::StateStore;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use sync_engine::{executor::SyncExecutor, SyncPlanner};
use tokio::runtime::Runtime;
use std::{collections::BTreeMap, path::PathBuf};

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
    // Use proxy settings from the active profile if available.
    #[cfg(target_os = "android")]
    {
        let proxy_url = resolve_proxy_url().unwrap_or_else(default_proxy_url);
        let proxy = match tun2proxy::ArgProxy::try_from(proxy_url.as_str()) {
            Ok(proxy) => proxy,
            Err(err) => {
                return FfiStatus::err(FfiErrorCode::InvalidInput, err.to_string());
            }
        };

        let mut args = tun2proxy::Args::default();
        args.proxy = proxy;
        args.tun_fd = Some(fd);
        args.close_fd_on_drop = Some(true);
        let mtu = get_runtime()
            .block_on(load_tun_config())
            .ok()
            .and_then(|config| config.mtu)
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(1500);

        let proxy_url_clone = proxy_url.clone();
        std::thread::spawn(move || {
            log::info!("Starting tun2proxy for FD {} to {}", fd, proxy_url_clone);
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

#[uniffi::export]
pub fn stop_vpn() -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        let exit_code = tun2proxy::mobile_stop();
        if exit_code == 0 {
            return FfiStatus::ok();
        }
        return FfiStatus::err(FfiErrorCode::NotReady, "tun2proxy not running");
    }

    #[cfg(not(target_os = "android"))]
    {
        log::warn!("stop_vpn called on non-Android target");
        FfiStatus::ok()
    }
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

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnTunSettings {
    pub mtu: Option<u32>,
    pub auto_route: Option<bool>,
    pub strict_route: Option<bool>,
    pub dns_servers: Vec<String>,
    pub ipv6: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnTunSettingsPatch {
    pub mtu: Option<u32>,
    pub auto_route: Option<bool>,
    pub strict_route: Option<bool>,
    pub dns_servers: Option<Vec<String>>,
    pub ipv6: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnTunSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<VpnTunSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsSettings {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub enhanced_mode: Option<String>,
    pub nameserver: Vec<String>,
    pub default_nameserver: Vec<String>,
    pub fallback: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsSettingsPatch {
    pub enable: Option<bool>,
    pub ipv6: Option<bool>,
    pub enhanced_mode: Option<String>,
    pub nameserver: Option<Vec<String>>,
    pub default_nameserver: Option<Vec<String>>,
    pub fallback: Option<Vec<String>>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct DnsSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<DnsSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FakeIpSettings {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Vec<String>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FakeIpSettingsPatch {
    pub fake_ip_range: Option<String>,
    pub fake_ip_filter: Option<Vec<String>>,
    pub store_fake_ip: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FakeIpSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<FakeIpSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RuleEntryRecord {
    pub rule: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RulesResult {
    pub status: FfiStatus,
    pub rules: Vec<RuleEntryRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RuleProvidersResult {
    pub status: FfiStatus,
    pub json: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSettings {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub sync_interval_mins: u32,
    pub sync_on_startup: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<WebDavSettings>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebDavSyncResult {
    pub status: FfiStatus,
    pub success_count: u32,
    pub failed_count: u32,
    pub total_actions: u32,
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

// --- Log Buffer ---

#[derive(Debug, Clone, uniffi::Enum)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Silent,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LogsResult {
    pub status: FfiStatus,
    pub entries: Vec<LogEntry>,
}

struct LogBuffer {
    entries: Vec<LogEntry>,
    max_size: usize,
    is_streaming: bool,
}

impl LogBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_size),
            max_size,
            is_streaming: false,
        }
    }

    fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    fn get_entries(&self, limit: usize) -> Vec<LogEntry> {
        let start = if self.entries.len() > limit {
            self.entries.len() - limit
        } else {
            0
        };
        self.entries[start..].to_vec()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

fn log_buffer() -> &'static Mutex<LogBuffer> {
    static LOG_BUFFER: OnceLock<Mutex<LogBuffer>> = OnceLock::new();
    LOG_BUFFER.get_or_init(|| Mutex::new(LogBuffer::new(500)))
}

fn parse_log_level(level: &str) -> LogLevel {
    match level.to_lowercase().as_str() {
        "debug" => LogLevel::Debug,
        "info" => LogLevel::Info,
        "warning" | "warn" => LogLevel::Warning,
        "error" => LogLevel::Error,
        "silent" => LogLevel::Silent,
        _ => LogLevel::Info,
    }
}

#[uniffi::export]
pub async fn logs_start_streaming() -> FfiStatus {
    get_runtime()
        .spawn(async move {
            // Check if already streaming
            {
                let buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                if buffer.is_streaming {
                    return FfiStatus::ok();
                }
            }
            
            // Set streaming flag
            {
                let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                buffer.is_streaming = true;
            }

            let client = match build_controller_client().await {
                Ok(c) => c,
                Err(e) => {
                    let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                    buffer.is_streaming = false;
                    return e;
                }
            };

            let rx = match client.stream_logs(Some("info")).await {
                Ok(rx) => rx,
                Err(e) => {
                    let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                    buffer.is_streaming = false;
                    return map_mihomo_error(e);
                }
            };

            tokio::spawn(async move {
                let mut rx = rx;
                while let Some(line) = rx.recv().await {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&line) {
                        let level_str = parsed
                            .get("level")
                            .and_then(|v| v.as_str())
                            .unwrap_or("info");
                        let msg = parsed
                            .get("payload")
                            .or_else(|| parsed.get("message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or(&line)
                            .to_string();
                        let entry = LogEntry {
                            level: parse_log_level(level_str),
                            message: msg,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                        };
                        let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                        buffer.push(entry);
                    }
                }
                let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
                buffer.is_streaming = false;
            });

            FfiStatus::ok()
        })
        .await
        .unwrap_or_else(|e| FfiStatus::err(FfiErrorCode::Unknown, format!("runtime error: {}", e)))
}

#[uniffi::export]
pub fn logs_get(limit: u32) -> LogsResult {
    let buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
    LogsResult {
        status: FfiStatus::ok(),
        entries: buffer.get_entries(limit as usize),
    }
}

#[uniffi::export]
pub fn logs_clear() -> FfiStatus {
    let mut buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
    buffer.clear();
    FfiStatus::ok()
}

#[uniffi::export]
pub fn logs_is_streaming() -> bool {
    let buffer = log_buffer().lock().unwrap_or_else(|p| p.into_inner());
    buffer.is_streaming
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
                    if let Some(bridge) = get_android_bridge()
                        && let Ok(true) = bridge.core_is_running().await {
                            let _ = bridge.core_stop().await;
                            let _ = bridge.core_start().await;
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

#[uniffi::export]
pub async fn vpn_tun_settings() -> VpnTunSettingsResult {
    get_runtime()
        .spawn(async move {
            match load_vpn_tun_settings().await {
                Ok(settings) => VpnTunSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => VpnTunSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| VpnTunSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn vpn_tun_settings_save(patch: VpnTunSettingsPatch) -> VpnTunSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_vpn_tun_settings(patch).await {
                Ok(settings) => VpnTunSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => VpnTunSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| VpnTunSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

// --- DNS API ---

#[uniffi::export]
pub async fn dns_settings() -> DnsSettingsResult {
    get_runtime()
        .spawn(async move {
            match load_dns_settings().await {
                Ok(settings) => DnsSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => DnsSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| DnsSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn dns_settings_save(patch: DnsSettingsPatch) -> DnsSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_dns_settings(patch).await {
                Ok(settings) => DnsSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => DnsSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| DnsSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

// --- Fake-IP API ---

#[uniffi::export]
pub async fn fake_ip_settings() -> FakeIpSettingsResult {
    get_runtime()
        .spawn(async move {
            match load_fake_ip_settings().await {
                Ok(settings) => FakeIpSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => FakeIpSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| FakeIpSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn fake_ip_settings_save(patch: FakeIpSettingsPatch) -> FakeIpSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_fake_ip_settings(patch).await {
                Ok(settings) => FakeIpSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => FakeIpSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| FakeIpSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn fake_ip_cache_clear() -> FfiBoolResult {
    get_runtime()
        .spawn(async move {
            match clear_fake_ip_cache().await.map_err(map_anyhow_error) {
                Ok(removed) => FfiBoolResult::ok(removed),
                Err(status) => FfiBoolResult {
                    status,
                    value: false,
                },
            }
        })
        .await
        .unwrap_or_else(|e| {
            FfiBoolResult::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

// --- Rules API ---

#[uniffi::export]
pub async fn rules_list() -> RulesResult {
    get_runtime()
        .spawn(async move {
            match load_rules().await.map_err(map_anyhow_error) {
                Ok(rules) => RulesResult {
                    status: FfiStatus::ok(),
                    rules: rules.into_iter().map(core_rule_to_record).collect(),
                },
                Err(status) => RulesResult {
                    status,
                    rules: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RulesResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            rules: Vec::new(),
        })
}

#[uniffi::export]
pub async fn rules_save(rules: Vec<RuleEntryRecord>) -> RulesResult {
    get_runtime()
        .spawn(async move {
            let core_rules: Vec<CoreRuleEntry> =
                rules.iter().map(record_to_core_rule).collect();
            match save_rules(core_rules).await.map_err(map_anyhow_error) {
                Ok(rules) => RulesResult {
                    status: FfiStatus::ok(),
                    rules: rules.into_iter().map(core_rule_to_record).collect(),
                },
                Err(status) => RulesResult {
                    status,
                    rules: Vec::new(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RulesResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            rules: Vec::new(),
        })
}

#[uniffi::export]
pub async fn rule_providers() -> RuleProvidersResult {
    get_runtime()
        .spawn(async move {
            match load_rule_providers().await.map_err(map_anyhow_error) {
                Ok(providers) => RuleProvidersResult {
                    status: FfiStatus::ok(),
                    json: rule_providers_to_json(&providers),
                },
                Err(status) => RuleProvidersResult {
                    status,
                    json: "{}".to_string(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RuleProvidersResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            json: "{}".to_string(),
        })
}

#[uniffi::export]
pub async fn rule_providers_save(json: String) -> RuleProvidersResult {
    get_runtime()
        .spawn(async move {
            let providers = match parse_rule_providers_json(&json) {
                Ok(value) => value,
                Err(status) => {
                    return RuleProvidersResult {
                        status,
                        json: "{}".to_string(),
                    }
                }
            };
            match save_rule_providers(providers).await.map_err(map_anyhow_error) {
                Ok(providers) => RuleProvidersResult {
                    status: FfiStatus::ok(),
                    json: rule_providers_to_json(&providers),
                },
                Err(status) => RuleProvidersResult {
                    status,
                    json: "{}".to_string(),
                },
            }
        })
        .await
        .unwrap_or_else(|e| RuleProvidersResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            json: "{}".to_string(),
        })
}

// --- WebDAV API ---

#[uniffi::export]
pub async fn webdav_settings() -> WebDavSettingsResult {
    get_runtime()
        .spawn(async move {
            match load_webdav_settings().await {
                Ok(settings) => WebDavSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => WebDavSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn webdav_settings_save(settings: WebDavSettings) -> WebDavSettingsResult {
    get_runtime()
        .spawn(async move {
            match save_webdav_settings(settings).await {
                Ok(settings) => WebDavSettingsResult {
                    status: FfiStatus::ok(),
                    settings: Some(settings),
                },
                Err(status) => WebDavSettingsResult {
                    status,
                    settings: None,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSettingsResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            settings: None,
        })
}

#[uniffi::export]
pub async fn webdav_test(settings: WebDavSettings) -> FfiStatus {
    get_runtime()
        .spawn(async move { test_webdav_settings(settings).await })
        .await
        .unwrap_or_else(|e| {
            FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e))
        })
}

#[uniffi::export]
pub async fn webdav_sync_now() -> WebDavSyncResult {
    get_runtime()
        .spawn(async move {
            match sync_webdav_now().await {
                Ok(summary) => WebDavSyncResult {
                    status: FfiStatus::ok(),
                    success_count: summary.success_count as u32,
                    failed_count: summary.failed_count as u32,
                    total_actions: summary.total_actions as u32,
                },
                Err(status) => WebDavSyncResult {
                    status,
                    success_count: 0,
                    failed_count: 0,
                    total_actions: 0,
                },
            }
        })
        .await
        .unwrap_or_else(|e| WebDavSyncResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {}", e)),
            success_count: 0,
            failed_count: 0,
            total_actions: 0,
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

// Android-only helpers for tun2proxy URL resolution
#[cfg(target_os = "android")]
fn resolve_proxy_url() -> Option<String> {
    get_runtime().block_on(async {
        let manager = ConfigManager::new().map_err(map_mihomo_error)?;
        let profile = manager
            .get_current()
            .await
            .map_err(map_mihomo_error)?;
        let content = manager
            .load(&profile)
            .await
            .map_err(map_mihomo_error)?;
        let doc: Value = serde_yaml::from_str(&content)
            .map_err(|err| FfiStatus::err(FfiErrorCode::InvalidState, err.to_string()))?;
        Ok::<Option<String>, FfiStatus>(build_proxy_url(&doc))
    })
    .ok()
    .flatten()
}

#[cfg(target_os = "android")]
fn build_proxy_url(doc: &Value) -> Option<String> {
    let candidates = [
        ("mixed-port", "socks5"),
        ("socks-port", "socks5"),
        ("port", "http"),
    ];
    for (key, scheme) in candidates {
        if let Some(value) = doc.get(key) {
            if let Some(port) = port_from_value(value) {
                return Some(format!("{}://127.0.0.1:{}", scheme, port));
            }
        }
    }
    None
}

#[cfg(target_os = "android")]
fn port_from_value(value: &Value) -> Option<u16> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .filter(|v| *v > 0),
        Value::String(raw) => raw
            .trim()
            .parse::<u16>()
            .ok()
            .filter(|v| *v > 0),
        _ => None,
    }
}

#[cfg(target_os = "android")]
fn default_proxy_url() -> String {
    "socks5://127.0.0.1:7891".to_string()
}

async fn load_vpn_tun_settings() -> Result<VpnTunSettings, FfiStatus> {
    let tun_config = load_tun_config().await.map_err(map_anyhow_error)?;
    let dns_config = load_dns_config().await.map_err(map_anyhow_error)?;
    Ok(build_vpn_tun_settings(tun_config, dns_config))
}

async fn save_vpn_tun_settings(
    patch: VpnTunSettingsPatch,
) -> Result<VpnTunSettings, FfiStatus> {
    let (tun_patch, has_tun) = build_tun_patch(&patch);
    let has_dns = patch.dns_servers.is_some() || patch.ipv6.is_some();

    let tun_config = if has_tun {
        save_tun_config(tun_patch).await.map_err(map_anyhow_error)?
    } else {
        load_tun_config().await.map_err(map_anyhow_error)?
    };
    let dns_config = if has_dns {
        let current = load_dns_config().await.map_err(map_anyhow_error)?;
        let dns_patch = build_dns_patch(&patch, &current);
        save_dns_config(dns_patch).await.map_err(map_anyhow_error)?
    } else {
        load_dns_config().await.map_err(map_anyhow_error)?
    };

    Ok(build_vpn_tun_settings(tun_config, dns_config))
}

fn build_vpn_tun_settings(
    tun_config: CoreTunConfig,
    dns_config: CoreDnsConfig,
) -> VpnTunSettings {
    let dns_servers = dns_config
        .nameserver
        .or(dns_config.default_nameserver)
        .unwrap_or_default();
    VpnTunSettings {
        mtu: tun_config.mtu,
        auto_route: tun_config.auto_route,
        strict_route: tun_config.strict_route,
        dns_servers,
        ipv6: dns_config.ipv6,
    }
}

fn build_tun_patch(patch: &VpnTunSettingsPatch) -> (CoreTunConfigPatch, bool) {
    let mut core_patch = CoreTunConfigPatch::default();
    let mut has_patch = false;
    if let Some(value) = patch.mtu {
        core_patch.mtu = Some(value);
        has_patch = true;
    }
    if let Some(value) = patch.auto_route {
        core_patch.auto_route = Some(value);
        has_patch = true;
    }
    if let Some(value) = patch.strict_route {
        core_patch.strict_route = Some(value);
        has_patch = true;
    }
    (core_patch, has_patch)
}

fn build_dns_patch(
    patch: &VpnTunSettingsPatch,
    current: &CoreDnsConfig,
) -> CoreDnsConfigPatch {
    let mut core_patch = CoreDnsConfigPatch::default();
    if let Some(value) = patch.ipv6 {
        core_patch.ipv6 = Some(value);
    }
    if let Some(value) = patch.dns_servers.clone() {
        if current.nameserver.is_some() {
            core_patch.nameserver = Some(value);
        } else {
            core_patch.default_nameserver = Some(value);
        }
    }
    core_patch
}

async fn load_dns_settings() -> Result<DnsSettings, FfiStatus> {
    let config = load_dns_config().await.map_err(map_anyhow_error)?;
    Ok(build_dns_settings(config))
}

async fn save_dns_settings(patch: DnsSettingsPatch) -> Result<DnsSettings, FfiStatus> {
    let core_patch = build_dns_settings_patch(patch);
    let config = save_dns_config(core_patch).await.map_err(map_anyhow_error)?;
    Ok(build_dns_settings(config))
}

fn build_dns_settings(config: CoreDnsConfig) -> DnsSettings {
    DnsSettings {
        enable: config.enable,
        ipv6: config.ipv6,
        enhanced_mode: config.enhanced_mode,
        nameserver: config.nameserver.unwrap_or_default(),
        default_nameserver: config.default_nameserver.unwrap_or_default(),
        fallback: config.fallback.unwrap_or_default(),
    }
}

fn build_dns_settings_patch(patch: DnsSettingsPatch) -> CoreDnsConfigPatch {
    CoreDnsConfigPatch {
        enable: patch.enable,
        ipv6: patch.ipv6,
        enhanced_mode: normalize_optional_string(patch.enhanced_mode),
        nameserver: sanitize_list(patch.nameserver),
        default_nameserver: sanitize_list(patch.default_nameserver),
        fallback: sanitize_list(patch.fallback),
        ..CoreDnsConfigPatch::default()
    }
}

async fn load_fake_ip_settings() -> Result<FakeIpSettings, FfiStatus> {
    let config = load_fake_ip_config().await.map_err(map_anyhow_error)?;
    Ok(build_fake_ip_settings(config))
}

async fn save_fake_ip_settings(patch: FakeIpSettingsPatch) -> Result<FakeIpSettings, FfiStatus> {
    let core_patch = build_fake_ip_settings_patch(patch);
    let config = save_fake_ip_config(core_patch).await.map_err(map_anyhow_error)?;
    Ok(build_fake_ip_settings(config))
}

fn build_fake_ip_settings(config: CoreFakeIpConfig) -> FakeIpSettings {
    FakeIpSettings {
        fake_ip_range: config.fake_ip_range,
        fake_ip_filter: config.fake_ip_filter.unwrap_or_default(),
        store_fake_ip: config.store_fake_ip,
    }
}

fn build_fake_ip_settings_patch(patch: FakeIpSettingsPatch) -> CoreFakeIpConfigPatch {
    CoreFakeIpConfigPatch {
        fake_ip_range: normalize_optional_string(patch.fake_ip_range),
        fake_ip_filter: sanitize_list(patch.fake_ip_filter),
        store_fake_ip: patch.store_fake_ip,
    }
}

fn core_rule_to_record(entry: CoreRuleEntry) -> RuleEntryRecord {
    RuleEntryRecord {
        rule: entry.rule,
        enabled: entry.enabled,
    }
}

fn record_to_core_rule(entry: &RuleEntryRecord) -> CoreRuleEntry {
    CoreRuleEntry {
        rule: entry.rule.trim().to_string(),
        enabled: entry.enabled,
    }
}

fn rule_providers_to_json(providers: &CoreRuleProviders) -> String {
    let value = JsonValue::Object(
        providers
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

fn parse_rule_providers_json(value: &str) -> Result<CoreRuleProviders, FfiStatus> {
    let parsed: JsonValue = serde_json::from_str(value).map_err(|err| {
        FfiStatus::err(FfiErrorCode::InvalidInput, format!("invalid JSON: {err}"))
    })?;
    let object = parsed.as_object().ok_or_else(|| {
        FfiStatus::err(
            FfiErrorCode::InvalidInput,
            "rule providers JSON must be an object",
        )
    })?;
    let mut providers: BTreeMap<String, JsonValue> = BTreeMap::new();
    for (key, value) in object {
        providers.insert(key.clone(), value.clone());
    }
    Ok(providers)
}

async fn load_webdav_settings() -> Result<WebDavSettings, FfiStatus> {
    let (settings, _) = load_app_settings().await?;
    Ok(webdav_settings_from_core(&settings.webdav))
}

async fn save_webdav_settings(settings: WebDavSettings) -> Result<WebDavSettings, FfiStatus> {
    let (mut app_settings, path) = load_app_settings().await?;
    app_settings.webdav = webdav_settings_to_core(settings);
    save_settings(&path, &app_settings)
        .await
        .map_err(map_anyhow_error)?;
    Ok(webdav_settings_from_core(&app_settings.webdav))
}

async fn test_webdav_settings(settings: WebDavSettings) -> FfiStatus {
    crate::tls::ensure_rustls_provider();
    let config = webdav_settings_to_core(settings);
    if let Err(status) = validate_webdav_config(&config) {
        return status;
    }
    let dav = match WebDavClient::new(&config.url, &config.username, &config.password) {
        Ok(client) => client,
        Err(err) => {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("invalid WebDAV config: {err}"),
            )
        }
    };
    match dav.list("/").await {
        Ok(_) => FfiStatus::ok(),
        Err(err) => FfiStatus::err(
            FfiErrorCode::Network,
            format!("connection test failed: {err}"),
        ),
    }
}

#[derive(Debug, Default)]
struct WebDavSyncSummary {
    success_count: usize,
    failed_count: usize,
    total_actions: usize,
}

async fn sync_webdav_now() -> Result<WebDavSyncSummary, FfiStatus> {
    crate::tls::ensure_rustls_provider();
    let (settings, _) = load_app_settings().await?;
    if !settings.webdav.enabled {
        return Err(FfiStatus::err(FfiErrorCode::NotReady, "WebDAV is disabled"));
    }
    run_webdav_sync(&settings.webdav).await
}

async fn run_webdav_sync(config: &CoreWebDavConfig) -> Result<WebDavSyncSummary, FfiStatus> {
    validate_webdav_config(config)?;
    let dav = WebDavClient::new(&config.url, &config.username, &config.password)
        .map_err(|err| {
            FfiStatus::err(
                FfiErrorCode::InvalidInput,
                format!("invalid WebDAV config: {err}"),
            )
        })?;

    let home = get_home_dir().map_err(map_mihomo_error)?;
    let local_root = home.join("configs");
    if !local_root.exists() {
        tokio::fs::create_dir_all(&local_root)
            .await
            .map_err(|e| FfiStatus::err(FfiErrorCode::Io, e.to_string()))?;
    }
    let db_path = home.join("sync_state.db").to_string_lossy().to_string();
    let store = StateStore::new(&db_path)
        .await
        .map_err(map_anyhow_error)?;

    let planner = SyncPlanner::new(local_root, "/".to_string(), &dav, &store);
    let actions = planner
        .build_plan()
        .await
        .map_err(map_anyhow_error)?;

    if actions.is_empty() {
        return Ok(WebDavSyncSummary::default());
    }

    let executor = SyncExecutor::new(&dav, &store);
    let total_actions = actions.len();
    let mut success_count = 0usize;
    let mut failed_count = 0usize;
    for action in actions {
        match executor.execute(action).await {
            Ok(()) => success_count = success_count.saturating_add(1),
            Err(_) => failed_count = failed_count.saturating_add(1),
        }
    }

    Ok(WebDavSyncSummary {
        success_count,
        failed_count,
        total_actions,
    })
}

async fn load_app_settings() -> Result<(AppSettings, PathBuf), FfiStatus> {
    let base = get_home_dir().map_err(map_mihomo_error)?;
    let path = settings_path(&base)
        .map_err(|err| FfiStatus::err(FfiErrorCode::InvalidState, err.to_string()))?;
    let settings = load_settings(&path).await.map_err(map_anyhow_error)?;
    Ok((settings, path))
}

fn webdav_settings_from_core(config: &CoreWebDavConfig) -> WebDavSettings {
    WebDavSettings {
        enabled: config.enabled,
        url: config.url.clone(),
        username: config.username.clone(),
        password: config.password.clone(),
        sync_interval_mins: config.sync_interval_mins,
        sync_on_startup: config.sync_on_startup,
    }
}

fn webdav_settings_to_core(settings: WebDavSettings) -> CoreWebDavConfig {
    CoreWebDavConfig {
        enabled: settings.enabled,
        url: settings.url.trim().to_string(),
        username: settings.username.trim().to_string(),
        password: settings.password,
        sync_interval_mins: settings.sync_interval_mins,
        sync_on_startup: settings.sync_on_startup,
    }
}

fn validate_webdav_config(config: &CoreWebDavConfig) -> Result<(), FfiStatus> {
    if config.url.trim().is_empty() {
        return Err(FfiStatus::err(
            FfiErrorCode::InvalidInput,
            "WebDAV URL is empty",
        ));
    }
    Ok(())
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn sanitize_list(value: Option<Vec<String>>) -> Option<Vec<String>> {
    value.map(|items| {
        items
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect()
    })
}

// --- App Routing API ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AppRoutingMode {
    ProxyAll,
    ProxySelected,
    BypassSelected,
}

impl From<CoreAppRoutingMode> for AppRoutingMode {
    fn from(mode: CoreAppRoutingMode) -> Self {
        match mode {
            CoreAppRoutingMode::ProxyAll => AppRoutingMode::ProxyAll,
            CoreAppRoutingMode::ProxySelected => AppRoutingMode::ProxySelected,
            CoreAppRoutingMode::BypassSelected => AppRoutingMode::BypassSelected,
        }
    }
}

impl From<AppRoutingMode> for CoreAppRoutingMode {
    fn from(mode: AppRoutingMode) -> Self {
        match mode {
            AppRoutingMode::ProxyAll => CoreAppRoutingMode::ProxyAll,
            AppRoutingMode::ProxySelected => CoreAppRoutingMode::ProxySelected,
            AppRoutingMode::BypassSelected => CoreAppRoutingMode::BypassSelected,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingConfig {
    pub mode: AppRoutingMode,
    pub packages: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingResult {
    pub status: FfiStatus,
    pub config: Option<AppRoutingConfig>,
}

#[uniffi::export]
pub fn app_routing_load() -> AppRoutingResult {
    match load_app_routing() {
        Ok(config) => AppRoutingResult {
            status: FfiStatus::ok(),
            config: Some(AppRoutingConfig {
                mode: config.mode.into(),
                packages: config.packages.into_iter().collect(),
            }),
        },
        Err(e) => AppRoutingResult {
            status: FfiStatus::err(FfiErrorCode::Io, e.to_string()),
            config: None,
        },
    }
}

#[uniffi::export]
pub fn app_routing_save(mode: AppRoutingMode, packages: Vec<String>) -> FfiStatus {
    let config = CoreAppRoutingConfig {
        mode: mode.into(),
        packages: packages.into_iter().collect(),
    };
    match save_app_routing(&config) {
        Ok(_) => FfiStatus::ok(),
        Err(e) => FfiStatus::err(FfiErrorCode::Io, e.to_string()),
    }
}

#[uniffi::export]
pub fn app_routing_set_mode(mode: AppRoutingMode) -> FfiStatus {
    match core_set_routing_mode(mode.into()) {
        Ok(_) => FfiStatus::ok(),
        Err(e) => FfiStatus::err(FfiErrorCode::Io, e.to_string()),
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AppRoutingToggleResult {
    pub status: FfiStatus,
    pub is_selected: bool,
}

#[uniffi::export]
pub fn app_routing_toggle_package(package: String) -> AppRoutingToggleResult {
    match core_toggle_package(&package) {
        Ok(is_selected) => AppRoutingToggleResult {
            status: FfiStatus::ok(),
            is_selected,
        },
        Err(e) => AppRoutingToggleResult {
            status: FfiStatus::err(FfiErrorCode::Io, e.to_string()),
            is_selected: false,
        },
    }
}

#[uniffi::export]
pub fn app_routing_get_allowed_packages() -> Vec<String> {
    match load_app_routing() {
        Ok(config) => config.get_allowed_packages().unwrap_or_default(),
        Err(_) => Vec::new(),
    }
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
    let manager = ConfigManager::new().map_err(map_mihomo_error)?;
    let controller_url = match manager.get_external_controller().await {
        Ok(url) => url,
        Err(err) => {
            if let Some(bridge) = get_android_bridge()
                && let Some(url) = bridge.core_controller_url() {
                    return MihomoClient::new(&url, None).map_err(map_mihomo_error);
                }
            return Err(map_mihomo_error(err));
        }
    };
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
