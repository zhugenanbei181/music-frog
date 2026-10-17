use super::*;
#[cfg(target_os = "android")]
use crate::uniffi_api::settings::read_dns_settings_local;

#[cfg(target_os = "android")]
pub(super) async fn start_prepared_vpn(request: VpnStartRequest) -> FfiStatus {
    let expected_core_scope = request.prepared.core_scope.clone();
    let application = VpnServiceApplication::new(Arc::new(AndroidVpnServicePort::shared()));
    match application.start(request).await {
        Ok(snapshot) if snapshot.is_running() => match active_core_scope().await {
            Ok(current_scope) if current_scope == expected_core_scope => FfiStatus::ok(),
            Ok(_) => FfiStatus::err(
                FfiErrorCode::InvalidState,
                "Core identity, generation, or session changed while the VPN worker started",
            ),
            Err(status) => status,
        },
        Ok(snapshot) => FfiStatus::err(
            FfiErrorCode::NotReady,
            format!(
                "VPN data-plane start receipt/readback did not reach foreground Running: {:?}",
                snapshot.state
            ),
        ),
        Err(failure) => map_application_failure(failure),
    }
}

// Android-only helpers for tun2proxy URL resolution
#[cfg(target_os = "android")]
pub(super) async fn resolve_proxy_url() -> Result<Option<String>, FfiStatus> {
    let configuration = build_configuration_application().await?;
    configuration
        .load_proxy_endpoint()
        .await
        .map_err(map_application_failure)
}

#[cfg(target_os = "android")]
pub(super) fn default_proxy_url() -> String {
    "socks5://127.0.0.1:7891".to_string()
}

#[cfg(target_os = "android")]
pub(super) async fn build_vpn_configuration(
    proxy_endpoint: String,
) -> Result<VpnConfiguration, FfiStatus> {
    let tun_settings = load_vpn_tun_settings().await?;
    let dns_result = read_dns_settings_local().await;
    let dns_snapshot = match (dns_result.status.code, dns_result.settings) {
        (FfiErrorCode::Ok, Some(settings)) => settings,
        (FfiErrorCode::Ok, None) => {
            return Err(FfiStatus::err(
                FfiErrorCode::NotReady,
                "DNS settings owner returned no settings readback",
            ));
        }
        (_, _) => return Err(dns_result.status),
    };
    let mtu = tun_settings.mtu.unwrap_or(1500);
    let ipv6 = dns_snapshot.ipv6.unwrap_or(true);
    let dns_servers = if dns_snapshot.nameserver_configured {
        dns_snapshot.nameserver
    } else {
        dns_snapshot.default_nameserver
    }
    .into_iter()
    .filter(|server| server.parse::<IpAddr>().is_ok())
    .collect::<Vec<_>>();
    let route_plan = VpnRouteConfig {
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
            .chain(
                route_plan
                    .excluded_routes
                    .into_iter()
                    .map(|route| VpnRoute {
                        address: route.ip,
                        prefix: route.prefix,
                        exclude: true,
                    }),
            )
            .collect(),
        dns_servers,
        ipv6,
        foreground_requested: true,
    })
}

pub(super) async fn load_vpn_tun_settings() -> Result<VpnTunSettings, FfiStatus> {
    let application = build_configuration_application().await?;
    let tun_config = application
        .load_tun_config()
        .await
        .map_err(map_application_failure)?;
    Ok(build_vpn_tun_settings(tun_config))
}

pub(super) async fn save_vpn_tun_settings(
    patch: VpnTunSettingsPatch,
) -> Result<VpnTunSettings, FfiStatus> {
    let tun_patch = build_tun_patch(&patch);

    let application = build_configuration_application().await?;
    let tun_config = application
        .save_tun_config(tun_patch)
        .await
        .map_err(map_application_failure)?;
    Ok(build_vpn_tun_settings(tun_config))
}

pub(super) fn build_vpn_tun_settings(tun_config: tun::TunConfig) -> VpnTunSettings {
    VpnTunSettings {
        mtu: tun_config.mtu,
        auto_route: tun_config.auto_route,
        strict_route: tun_config.strict_route,
        stack: tun_config.stack,
        auto_detect_interface: tun_config.auto_detect_interface,
    }
}
