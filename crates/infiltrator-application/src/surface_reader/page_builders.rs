//! Page builders for DNS, app-routing, sync and settings snapshots.
//!
//! Each builder is a pure-ish projection from already-read host facts into
//! the shared contract page model; the reader trait impl only gathers the
//! inputs and calls these.

use super::projections::missing;
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) async fn build_dns_page(
    configuration: Option<&ConfigurationApplication>,
    runtime_config: Option<&Result<infiltrator_domain::runtime::ConfigSnapshot, PortError>>,
    dns_cache: Option<&crate::dns_cache_application::DnsCacheApplication>,
    dns_latency: Option<&crate::dns_latency_application::DnsLatencyApplication>,
    dns_leak: Option<&crate::dns_leak_application::DnsLeakApplication>,
    stun_probe: Option<&crate::stun_probe_application::StunProbeApplication>,
    runtime_connections: Option<
        &Result<infiltrator_domain::runtime::ConnectionSnapshot, PortError>,
    >,
    port_conflicts: &infiltrator_contract::port_conflict::PortConflictSnapshot,
) -> surface_snapshot::PageData<surface_snapshot::DnsPageSnapshot> {
    let cache_flush = crate::dns_workbench_application::cache_flush_report(dns_cache);
    let latency = crate::dns_workbench_application::latency_report(dns_latency);
    let leak = crate::dns_workbench_application::leak_report(dns_leak);
    let stun = crate::dns_workbench_application::stun_report(stun_probe);
    let connections = runtime_connections.and_then(|result| result.as_ref().ok());
    if let Some(configuration) = configuration {
        let dns = configuration.load_dns_config().await;
        let fake_ip = configuration.load_fake_ip_config().await;
        if let (Ok(dns), Ok(fake_ip)) = (dns, fake_ip) {
            let range = dns
                .fake_ip_range
                .clone()
                .or(fake_ip.fake_ip_range)
                .unwrap_or_default();
            let mut snapshot = crate::dns_workbench_application::dns_page_snapshot(&dns, range);
            snapshot.cache_flush = cache_flush;
            snapshot.fake_ip_pool = crate::dns_workbench_application::fake_ip_pool_from_connections(
                &snapshot.fake_ip_range,
                connections,
            );
            crate::dns_workbench_application::apply_latency_report(&mut snapshot, &latency);
            snapshot.leak = leak.clone();
            snapshot.stun = stun.clone();
            snapshot.self_heal = crate::dns_self_heal_application::dns_self_heal_snapshot(
                &dns,
                crate::dns_self_heal_application::dns_listen_conflict(port_conflicts),
                &latency,
            );
            return surface_snapshot::PageData::ready(snapshot);
        }
    }
    match runtime_config {
        Some(Ok(config)) => match &config.dns {
            Some(dns) => surface_snapshot::PageData::ready(surface_snapshot::DnsPageSnapshot {
                enhanced_mode: infiltrator_contract::dns::DnsEnhancedMode::from_config_value(Some(
                    dns.enhanced_mode.as_str(),
                )),
                servers: crate::dns_workbench_application::dns_servers_from_lists(
                    dns.nameserver.clone(),
                    dns.fallback.clone(),
                ),
                cache_flush,
                latency: latency.clone(),
                leak: leak.clone(),
                stun: stun.clone(),
                // The runtime DnsSnapshot carries no fake-ip-range, so the
                // pool stays honestly unsupported on this path.
                ..surface_snapshot::DnsPageSnapshot::default()
            }),
            None => surface_snapshot::PageData::empty(surface_snapshot::DnsPageSnapshot {
                enhanced_mode: infiltrator_contract::dns::DnsEnhancedMode::Unmapped,
                cache_flush,
                latency,
                leak,
                stun,
                ..surface_snapshot::DnsPageSnapshot::default()
            }),
        },
        Some(Err(error)) => surface_snapshot::PageData::failed(Failure::new(
            ErrorCode::Network,
            error.to_string(),
            true,
        )),
        None => surface_snapshot::PageData::unavailable(missing("DNS reader")),
    }
}

pub(super) fn app_routing_page(
    config: infiltrator_domain::app_routing::AppRoutingConfig,
    uwp_loopback: infiltrator_contract::uwp::UwpLoopbackSnapshot,
) -> surface_snapshot::AppRoutingPageSnapshot {
    let mode = match config.mode {
        AppRoutingMode::ProxyAll => "proxy_all",
        AppRoutingMode::ProxySelected => "proxy_selected",
        AppRoutingMode::BypassSelected => "bypass_selected",
    };
    let mut ids = config.packages.iter().cloned().collect::<Vec<_>>();
    ids.extend(config.rules.keys().cloned());
    ids.sort();
    ids.dedup();
    let apps = ids
        .into_iter()
        .map(|id| {
            let rule = config.rules.get(&id).copied().unwrap_or(match config.mode {
                AppRoutingMode::ProxyAll => AppRoutingRule::Proxy,
                AppRoutingMode::ProxySelected => AppRoutingRule::Direct,
                AppRoutingMode::BypassSelected => AppRoutingRule::Proxy,
            });
            surface_snapshot::AppSnapshot {
                id: id.clone(),
                name: id.clone(),
                process_name: id,
                rule: match rule {
                    AppRoutingRule::Proxy => "proxy",
                    AppRoutingRule::Direct => "direct",
                    AppRoutingRule::Block => "block",
                }
                .to_owned(),
                is_system: false,
            }
        })
        .collect();
    surface_snapshot::AppRoutingPageSnapshot {
        mode: mode.to_owned(),
        include_system: false,
        apps,
        uwp_loopback,
    }
}

pub(super) async fn build_sync_page(
    settings: Option<&Result<infiltrator_domain::settings::AppSettings, Failure>>,
    profiles: Option<&ProfileApplication>,
    snapshots: Option<&SnapshotApplication>,
) -> surface_snapshot::PageData<surface_snapshot::SyncPageSnapshot> {
    let Some(Ok(settings)) = settings else {
        return surface_snapshot::PageData::unavailable(missing("settings application"));
    };
    let mut snapshot_items = Vec::new();
    if let (Some(profiles), Some(snapshots)) = (profiles, snapshots)
        && let Ok(profile) = profiles.current_profile().await
        && let Ok(items) = snapshots.list(&profile).await
    {
        snapshot_items = items
            .into_iter()
            .map(|item| surface_snapshot::SnapshotItemSnapshot {
                id: item.path.to_string_lossy().to_string(),
                timestamp: item.timestamp.to_rfc3339(),
                device: "local".to_owned(),
                size_bytes: 0,
            })
            .collect();
    }
    surface_snapshot::PageData::ready(surface_snapshot::SyncPageSnapshot {
        status: if settings.webdav.enabled {
            "connected".to_owned()
        } else {
            "disconnected".to_owned()
        },
        server_url: settings.webdav.url.clone(),
        username: settings.webdav.username.clone(),
        last_sync: None,
        auto_sync: settings.webdav.sync_on_startup,
        conflict: None,
        snapshots: snapshot_items,
    })
}

pub(super) fn build_settings_page(
    settings: Option<&Result<infiltrator_domain::settings::AppSettings, Failure>>,
    runtime_config: Option<&Result<infiltrator_domain::runtime::ConfigSnapshot, PortError>>,
    system_proxy: &infiltrator_contract::system_proxy::SystemProxySnapshot,
    pac: infiltrator_contract::pac::PacSnapshot,
) -> surface_snapshot::PageData<surface_snapshot::SettingsPageSnapshot> {
    let settings = match settings {
        Some(Ok(settings)) => settings,
        Some(Err(error)) => {
            return surface_snapshot::PageData::failed(Failure::new(
                ErrorCode::Storage,
                error.message.clone(),
                true,
            ));
        }
        None => return surface_snapshot::PageData::unavailable(missing("settings application")),
    };
    let config = runtime_config.and_then(|result| result.as_ref().ok());
    surface_snapshot::PageData::ready(surface_snapshot::SettingsPageSnapshot {
        autostart: false,
        system_proxy: system_proxy.is_enabled(),
        mixed_port: config.map_or(0, |value| value.mixed_port),
        allow_lan: config.is_some_and(|value| value.allow_lan),
        lan_bind_address: config.map_or_else(
            || infiltrator_contract::lan::DEFAULT_BIND_ADDRESS.to_owned(),
            |value| value.bind_address.clone(),
        ),
        lan_security: config.map_or_else(
            infiltrator_contract::lan::LanSecuritySnapshot::default,
            |value| {
                infiltrator_contract::lan::LanSecuritySnapshot::new(
                    0,
                    value.lan_allowed_ips.clone(),
                    value.lan_disallowed_ips.clone(),
                    value.skip_auth_prefixes.clone(),
                    value.authentication_enabled,
                    value.authentication_user_count,
                    value.authentication_username.clone(),
                )
            },
        ),
        ipv6_routing: config.map_or_else(
            infiltrator_contract::ipv6::Ipv6RoutingSnapshot::default,
            |value| {
                infiltrator_contract::ipv6::Ipv6RoutingSnapshot::new(
                    0,
                    value.ipv6,
                    value.tun.as_ref().is_some_and(|tun| tun.enable),
                )
            },
        ),
        pac,
        tun_enabled: config
            .and_then(|value| value.tun.as_ref())
            .is_some_and(|tun| tun.enable),
        tun_stack: config
            .and_then(|value| value.tun.as_ref())
            .map_or_else(String::new, |tun| tun.stack.clone()),
        tun_auto_route: config
            .and_then(|value| value.tun.as_ref())
            .is_some_and(|tun| tun.auto_route),
        tun_strict_route: config
            .and_then(|value| value.tun.as_ref())
            .is_some_and(|tun| tun.strict_route),
        controller_port: 0,
        log_level: config
            .map(|value| value.log_level.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "info".to_owned()),
        core_channel: settings.core_channel.clone(),
        mini_hud: settings.mini_hud,
    })
}
