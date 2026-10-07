//! Page builders for DNS, app-routing, sync and settings snapshots.
//!
//! Each builder is a pure-ish projection from already-read host facts into
//! the shared contract page model; the reader trait impl only gathers the
//! inputs and calls these.

use super::projections::missing;
use super::*;
use crate::dns_latency_application::DnsLatencyApplication;
use crate::dns_self_heal_application::{dns_listen_conflict, dns_self_heal_snapshot};
use crate::dns_workbench_application::{
    apply_latency_report, dns_page_snapshot, dns_servers_from_lists, fake_ip_pool_from_connections,
    latency_report, stun_report,
};
use crate::stun_probe_application::StunProbeApplication;
use infiltrator_contract::dns::DnsEnhancedMode;
use infiltrator_contract::pac::PacSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::runtime_control::RuntimeControlSnapshot;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{SnapshotItemSnapshot, SyncPageSnapshot};
use infiltrator_contract::system_proxy::SystemProxySnapshot;
use infiltrator_contract::uwp::UwpLoopbackSnapshot;
use infiltrator_domain::app_routing::AppRoutingConfig;
use infiltrator_domain::runtime::{ConfigSnapshot, ConnectionSnapshot};
use infiltrator_domain::settings::AppSettings;

#[cfg(test)]
#[path = "settings_observation_tests.rs"]
mod settings_observation_tests;

#[allow(clippy::too_many_arguments)]
pub(super) async fn build_dns_page(
    configuration: Option<&ConfigurationApplication>,
    runtime_config: Option<&Result<ConfigSnapshot, PortError>>,
    dns_latency: Option<&DnsLatencyApplication>,
    stun_probe: Option<&StunProbeApplication>,
    runtime_connections: Option<&Result<ConnectionSnapshot, PortError>>,
    port_conflicts: &PortConflictSnapshot,
) -> surface_snapshot::PageData<surface_snapshot::DnsPageSnapshot> {
    let latency = latency_report(dns_latency);
    let stun = stun_report(stun_probe);
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
            let mut snapshot = dns_page_snapshot(&dns, range);
            snapshot.fake_ip_pool =
                fake_ip_pool_from_connections(&snapshot.fake_ip_range, connections);
            apply_latency_report(&mut snapshot, &latency);
            snapshot.stun = stun.clone();
            snapshot.self_heal =
                dns_self_heal_snapshot(&dns, dns_listen_conflict(port_conflicts), &latency);
            return surface_snapshot::PageData::ready(snapshot);
        }
    }
    match runtime_config {
        Some(Ok(config)) => match &config.dns {
            Some(dns) => surface_snapshot::PageData::ready(surface_snapshot::DnsPageSnapshot {
                enhanced_mode: DnsEnhancedMode::from_config_value(Some(dns.enhanced_mode.as_str())),
                servers: dns_servers_from_lists(dns.nameserver.clone(), dns.fallback.clone()),
                latency: latency.clone(),
                stun: stun.clone(),
                // The runtime DnsSnapshot carries no fake-ip-range, so the
                // pool stays honestly unsupported on this path.
                ..surface_snapshot::DnsPageSnapshot::default()
            }),
            None => surface_snapshot::PageData::empty(surface_snapshot::DnsPageSnapshot {
                enhanced_mode: DnsEnhancedMode::Unmapped,
                latency,
                stun,
                ..surface_snapshot::DnsPageSnapshot::default()
            }),
        },
        Some(Err(error)) => surface_snapshot::PageData::failed(Failure::from(error.clone())),
        None => surface_snapshot::PageData::unavailable(missing("DNS reader")),
    }
}

pub(super) fn app_routing_page(
    config: AppRoutingConfig,
    uwp_loopback: UwpLoopbackSnapshot,
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
    settings: Option<&Result<AppSettings, Failure>>,
    profiles: Option<&ProfileApplication>,
    snapshots: Option<&SnapshotApplication>,
) -> surface_snapshot::PageData<SyncPageSnapshot> {
    let settings = match settings {
        Some(Ok(settings)) => settings,
        Some(Err(failure)) => return surface_snapshot::PageData::failed(failure.clone()),
        None => return surface_snapshot::PageData::unavailable(missing("settings application")),
    };
    let mut snapshot_items = Vec::new();
    let history_status = if let (Some(profiles), Some(snapshots)) = (profiles, snapshots) {
        match profiles.current_profile().await {
            Ok(profile) => match snapshots.list(&profile).await {
                Ok(items) => {
                    snapshot_items = items
                        .into_iter()
                        .map(|item| SnapshotItemSnapshot {
                            profile: item.profile.clone(),
                            id: item.path.to_string_lossy().to_string(),
                            timestamp: item.timestamp.to_rfc3339(),
                            device: "local".into(),
                            size_bytes: None,
                        })
                        .collect();
                    if snapshot_items.is_empty() {
                        PageStatus::Empty
                    } else {
                        PageStatus::Ready
                    }
                }
                Err(failure) => PageStatus::Failed { failure },
            },
            Err(failure) => PageStatus::Failed { failure },
        }
    } else {
        PageStatus::Unavailable {
            failure: missing("snapshot history application"),
        }
    };
    surface_snapshot::PageData::ready(SyncPageSnapshot {
        status: if settings.webdav.enabled {
            SyncStatus::Configured
        } else {
            SyncStatus::Disconnected
        },
        server_url: settings.webdav.url.clone(),
        username: settings.webdav.username.clone(),
        last_sync: None,
        auto_sync: settings.webdav.sync_on_startup,
        conflict: None,
        snapshots: snapshot_items,
        history_status,
    })
}

pub(super) fn build_settings_page(
    settings: Option<&Result<AppSettings, Failure>>,
    observed: &RuntimeControlSnapshot,
    system_proxy: &SystemProxySnapshot,
    pac: PacSnapshot,
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
    surface_snapshot::PageData::ready(surface_snapshot::SettingsPageSnapshot {
        close_to_tray: Some(settings.close_to_tray),
        notifications_enabled: Some(settings.notifications_enabled),
        language: settings.language.clone(),
        autostart: false,
        system_proxy: system_proxy.is_enabled(),
        mixed_port: observed.mixed_port,
        allow_lan: observed.allow_lan,
        lan_bind_address: observed.lan_bind_address.clone(),
        lan_security: observed.lan_security.clone(),
        ipv6_routing: observed.ipv6_routing,
        pac,
        tun_enabled: observed.tun_enabled,
        tun_stack: observed.tun_stack.clone(),
        tun_auto_route: observed.tun_auto_route,
        tun_strict_route: observed.tun_strict_route,
        controller_port: None,
        log_level: observed.log_level.clone(),
        core_channel: settings.core_channel.clone(),
        mini_hud: settings.mini_hud,
    })
}
