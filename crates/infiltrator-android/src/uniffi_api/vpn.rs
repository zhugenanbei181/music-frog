//! VPN/TUN surface: tun2proxy lifecycle (`start_vpn`/`stop_vpn`), live TUN
//! status via the Android bridge, and the persisted VPN TUN settings
//! (MTU/routes/stack plus DNS servers patched into the DNS config).

#[cfg(target_os = "android")]
use serde_yaml_ng::Value;

use mihomo_platform::android_bridge::get_android_bridge;

use infiltrator_core::dns::{load_dns_config, save_dns_config};
use infiltrator_core::tun::{load_tun_config, save_tun_config};

#[cfg(target_os = "android")]
use super::support::build_config_manager;
use super::support::{get_runtime, map_anyhow_error, map_mihomo_error, normalize_optional_string};
use crate::ffi::{FfiErrorCode, FfiStatus};

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
    pub stack: Option<String>,
    pub auto_detect_interface: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnTunSettingsPatch {
    pub mtu: Option<u32>,
    pub auto_route: Option<bool>,
    pub strict_route: Option<bool>,
    pub dns_servers: Option<Vec<String>>,
    pub ipv6: Option<bool>,
    pub stack: Option<String>,
    pub auto_detect_interface: Option<bool>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnTunSettingsResult {
    pub status: FfiStatus,
    pub settings: Option<VpnTunSettings>,
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

async fn tun_status_internal() -> Result<bool, FfiStatus> {
    let bridge = get_android_bridge()
        .ok_or_else(|| FfiStatus::err(FfiErrorCode::NotReady, "android bridge not ready"))?;
    let enabled = bridge.tun_is_enabled().await.map_err(map_mihomo_error)?;
    Ok(enabled)
}

// Android-only helpers for tun2proxy URL resolution
#[cfg(target_os = "android")]
fn resolve_proxy_url() -> Option<String> {
    get_runtime()
        .block_on(async {
            let manager = build_config_manager().await?;
            let profile = manager.get_current().await.map_err(map_mihomo_error)?;
            let content = manager.load(&profile).await.map_err(map_mihomo_error)?;
            let doc: Value = serde_yaml_ng::from_str(&content)
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
        Value::String(raw) => raw.trim().parse::<u16>().ok().filter(|v| *v > 0),
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

async fn save_vpn_tun_settings(patch: VpnTunSettingsPatch) -> Result<VpnTunSettings, FfiStatus> {
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
    tun_config: infiltrator_core::tun::TunConfig,
    dns_config: infiltrator_core::dns::DnsConfig,
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
        stack: tun_config.stack,
        auto_detect_interface: tun_config.auto_detect_interface,
    }
}

pub(super) fn build_tun_patch(
    patch: &VpnTunSettingsPatch,
) -> (infiltrator_core::tun::TunConfigPatch, bool) {
    let mut core_patch = infiltrator_core::tun::TunConfigPatch::default();
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
    if let Some(value) = normalize_optional_string(patch.stack.clone()) {
        core_patch.stack = Some(value);
        has_patch = true;
    }
    if let Some(value) = patch.auto_detect_interface {
        core_patch.auto_detect_interface = Some(value);
        has_patch = true;
    }
    (core_patch, has_patch)
}

fn build_dns_patch(
    patch: &VpnTunSettingsPatch,
    current: &infiltrator_core::dns::DnsConfig,
) -> infiltrator_core::dns::DnsConfigPatch {
    let mut core_patch = infiltrator_core::dns::DnsConfigPatch::default();
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
