//! VPN/TUN surface: tun2proxy lifecycle (`start_vpn`/`stop_vpn`), live TUN
//! status via the Android bridge, and the persisted VPN TUN settings
//! (MTU/routes/stack plus DNS servers patched into the DNS config).

#[cfg(target_os = "android")]
use serde_yaml_ng::Value;

use mihomo_platform::android_bridge::get_android_bridge;

use infiltrator_domain::{dns, tun};
use infiltrator_application::vpn_application::VpnServiceApplication;
use infiltrator_contract::vpn::VpnSessionState;
#[cfg(target_os = "android")]
use infiltrator_contract::vpn::{VpnConfiguration, VpnRoute, VpnStartRequest};

#[cfg(target_os = "android")]
use crate::host_support::build_config_manager;
use crate::host_support::{
    build_configuration_application, get_runtime, map_application_failure, map_mihomo_error,
    normalize_optional_string,
};
use crate::ffi::{FfiErrorCode, FfiStatus};
use crate::vpn_service::AndroidVpnServicePort;

#[uniffi::export]
pub fn start_vpn(fd: i32) -> FfiStatus {
    log::info!("Rust received VPN File Descriptor: {}", fd);

    #[cfg(target_os = "android")]
    {
        if fd <= 0 {
            return FfiStatus::err(
                FfiErrorCode::InvalidInput,
                "VpnService supplied an invalid tunnel file descriptor",
            );
        }
        let proxy_endpoint = resolve_proxy_url().unwrap_or_else(default_proxy_url);
        let request = match get_runtime().block_on(build_vpn_start_request(fd, proxy_endpoint)) {
            Ok(request) => request,
            Err(status) => return status,
        };
        let application = VpnServiceApplication::new(std::sync::Arc::new(
            AndroidVpnServicePort::shared(),
        ));
        return match get_runtime().block_on(application.start(request)) {
            Ok(snapshot) if snapshot.is_running() => FfiStatus::ok(),
            Ok(snapshot) => FfiStatus::err(
                FfiErrorCode::NotReady,
                format!("VPN start did not reach foreground Running: {:?}", snapshot.state),
            ),
            Err(failure) => map_application_failure(failure),
        };
    }

    #[cfg(not(target_os = "android"))]
    FfiStatus::err(
        FfiErrorCode::NotSupported,
        "Android VpnService is unavailable on this target",
    )
}

/// Prepare the native Android `VpnService.Builder` before it calls
/// `Builder.establish()`. The later `start_vpn(fd)` callback only owns the
/// established TUN descriptor and starts tun2proxy.
#[uniffi::export]
pub fn prepare_vpn() -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        let proxy_endpoint = resolve_proxy_url().unwrap_or_else(default_proxy_url);
        let configuration = match get_runtime().block_on(build_vpn_configuration(proxy_endpoint)) {
            Ok(configuration) => configuration,
            Err(status) => return status,
        };
        let application = VpnServiceApplication::new(std::sync::Arc::new(
            AndroidVpnServicePort::shared(),
        ));
        return match get_runtime().block_on(application.prepare(configuration)) {
            Ok(snapshot) if snapshot.foreground => FfiStatus::ok(),
            Ok(snapshot) => FfiStatus::err(
                FfiErrorCode::NotReady,
                format!(
                    "VPN Builder configuration is not foreground-protected: {:?}",
                    snapshot.state
                ),
            ),
            Err(failure) => map_application_failure(failure),
        };
    }

    #[cfg(not(target_os = "android"))]
    {
        FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android VpnService is unavailable on this target",
        )
    }
}

#[uniffi::export]
pub fn stop_vpn() -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        let application = VpnServiceApplication::new(std::sync::Arc::new(
            AndroidVpnServicePort::shared(),
        ));
        return match get_runtime().block_on(application.stop()) {
            Ok(snapshot)
                if matches!(
                    snapshot.state,
                    VpnSessionState::Stopped | VpnSessionState::Revoked
                ) => FfiStatus::ok(),
            Ok(snapshot) => FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("VPN stop did not settle: {:?}", snapshot.state),
            ),
            Err(failure) => map_application_failure(failure),
        };
    }

    #[cfg(not(target_os = "android"))]
    {
        FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android VpnService is unavailable on this target",
        )
    }
}

#[uniffi::export]
pub fn revoke_vpn() -> FfiStatus {
    #[cfg(target_os = "android")]
    {
        let application = VpnServiceApplication::new(std::sync::Arc::new(
            AndroidVpnServicePort::shared(),
        ));
        return match get_runtime().block_on(application.revoke()) {
            Ok(snapshot) if snapshot.state == VpnSessionState::Revoked => FfiStatus::ok(),
            Ok(snapshot) => FfiStatus::err(
                FfiErrorCode::InvalidState,
                format!("VPN revoke did not settle: {:?}", snapshot.state),
            ),
            Err(failure) => map_application_failure(failure),
        };
    }

    #[cfg(not(target_os = "android"))]
    {
        FfiStatus::err(
            FfiErrorCode::NotSupported,
            "Android VpnService is unavailable on this target",
        )
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

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnSessionSnapshot {
    pub state: String,
    pub foreground: bool,
    pub mtu: Option<u32>,
    pub route_count: u32,
    pub dns_servers: Vec<String>,
    pub ipv6: bool,
    pub revision: u64,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct VpnSessionResult {
    pub status: FfiStatus,
    pub snapshot: Option<VpnSessionSnapshot>,
}

#[uniffi::export]
pub async fn vpn_session_status() -> VpnSessionResult {
    get_runtime()
        .spawn(async move {
            let application = VpnServiceApplication::new(std::sync::Arc::new(
                AndroidVpnServicePort::shared(),
            ));
            let snapshot = application.snapshot().await;
            let status = match &snapshot.state {
                VpnSessionState::Unsupported { reason } => {
                    FfiStatus::err(FfiErrorCode::NotSupported, reason.clone())
                }
                VpnSessionState::Failed { failure } => map_application_failure(failure.clone()),
                _ => FfiStatus::ok(),
            };
            VpnSessionResult {
                status,
                snapshot: Some(map_vpn_snapshot(snapshot)),
            }
        })
        .await
        .unwrap_or_else(|error| VpnSessionResult {
            status: FfiStatus::err(FfiErrorCode::Unknown, format!("runtime join error: {error}")),
            snapshot: None,
        })
}

fn map_vpn_snapshot(
    snapshot: infiltrator_contract::vpn::VpnSessionSnapshot,
) -> VpnSessionSnapshot {
    VpnSessionSnapshot {
        state: match snapshot.state {
            VpnSessionState::Idle => "idle",
            VpnSessionState::PermissionRequired => "permission_required",
            VpnSessionState::Starting => "starting",
            VpnSessionState::Running => "running",
            VpnSessionState::Stopping => "stopping",
            VpnSessionState::Stopped => "stopped",
            VpnSessionState::Revoked => "revoked",
            VpnSessionState::Unsupported { .. } => "unsupported",
            VpnSessionState::Failed { .. } => "failed",
        }
        .to_owned(),
        foreground: snapshot.foreground,
        mtu: snapshot.mtu,
        route_count: u32::try_from(snapshot.route_count).unwrap_or(u32::MAX),
        dns_servers: snapshot.dns_servers,
        ipv6: snapshot.ipv6,
        revision: snapshot.revision,
    }
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

#[cfg(target_os = "android")]
async fn build_vpn_start_request(
    fd: i32,
    proxy_endpoint: String,
) -> Result<VpnStartRequest, FfiStatus> {
    let configuration = build_vpn_configuration(proxy_endpoint).await?;
    Ok(VpnStartRequest {
        tun_fd: fd,
        proxy_endpoint: configuration.proxy_endpoint,
        mtu: configuration.mtu,
        routes: configuration.routes,
        dns_servers: configuration.dns_servers,
        ipv6: configuration.ipv6,
        foreground_requested: configuration.foreground_requested,
    })
}

#[cfg(target_os = "android")]
async fn build_vpn_configuration(
    proxy_endpoint: String,
) -> Result<VpnConfiguration, FfiStatus> {
    let settings = load_vpn_tun_settings().await?;
    let mtu = settings.mtu.unwrap_or(1500);
    let ipv6 = settings.ipv6.unwrap_or(true);
    let dns_servers = settings
        .dns_servers
        .into_iter()
        .filter(|server| server.parse::<std::net::IpAddr>().is_ok())
        .collect::<Vec<_>>();
    let route_plan = crate::vpn_route::VpnRouteConfig {
        bypass_lan: false,
        bypass_china: false,
        custom_dns: dns_servers.clone(),
        mtu,
    }
    .build_plan();
    Ok(VpnConfiguration {
        proxy_endpoint,
        mtu: route_plan.mtu,
        routes: route_plan
            .routes
            .into_iter()
            .map(|route| VpnRoute {
                address: route.ip,
                prefix: route.prefix,
                exclude: false,
            })
            .chain(route_plan.excluded_routes.into_iter().map(|route| VpnRoute {
                address: route.ip,
                prefix: route.prefix,
                exclude: true,
            }))
            .collect(),
        dns_servers,
        ipv6,
        foreground_requested: true,
    })
}

async fn load_vpn_tun_settings() -> Result<VpnTunSettings, FfiStatus> {
    let application = build_configuration_application().await?;
    let tun_config = application
        .load_tun_config()
        .await
        .map_err(map_application_failure)?;
    let dns_config = application
        .load_dns_config()
        .await
        .map_err(map_application_failure)?;
    Ok(build_vpn_tun_settings(tun_config, dns_config))
}

async fn save_vpn_tun_settings(patch: VpnTunSettingsPatch) -> Result<VpnTunSettings, FfiStatus> {
    let (tun_patch, has_tun) = build_tun_patch(&patch);
    let has_dns = patch.dns_servers.is_some() || patch.ipv6.is_some();

    let application = build_configuration_application().await?;
    let tun_config = if has_tun {
        application
            .save_tun_config(tun_patch)
            .await
            .map_err(map_application_failure)?
    } else {
        application
            .load_tun_config()
            .await
            .map_err(map_application_failure)?
    };
    let dns_config = if has_dns {
        let current = application
            .load_dns_config()
            .await
            .map_err(map_application_failure)?;
        let dns_patch = build_dns_patch(&patch, &current);
        application
            .save_dns_config(dns_patch)
            .await
            .map_err(map_application_failure)?
    } else {
        application
            .load_dns_config()
            .await
            .map_err(map_application_failure)?
    };

    Ok(build_vpn_tun_settings(tun_config, dns_config))
}

fn build_vpn_tun_settings(
    tun_config: tun::TunConfig,
    dns_config: dns::DnsConfig,
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
) -> (tun::TunConfigPatch, bool) {
    let mut core_patch = tun::TunConfigPatch::default();
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
    current: &dns::DnsConfig,
) -> dns::DnsConfigPatch {
    let mut core_patch = dns::DnsConfigPatch::default();
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
