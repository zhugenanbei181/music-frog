//! Page builders for DNS, app-routing, sync and settings snapshots.
//!
//! Each builder is a pure-ish projection from already-read host facts into
//! the shared contract page model; the reader trait impl only gathers the
//! inputs and calls these.

use super::projections::missing;
use super::*;
use infiltrator_application::dns_latency_application::DnsLatencyApplication;
use infiltrator_application::dns_self_heal_application::{
    dns_listen_conflict, dns_self_heal_snapshot,
};
use infiltrator_application::dns_workbench_application::{
    apply_latency_report, dns_page_snapshot, dns_servers_from_lists, fake_ip_pool_from_connections,
    latency_report, stun_report,
};
use infiltrator_application::runtime_configuration_application::RuntimeConfigurationReadout;
use infiltrator_application::stun_probe_application::StunProbeApplication;
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::dns::DnsEnhancedMode;
use infiltrator_contract::pac::PacSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::runtime_control::RuntimeControlSnapshot;
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{SnapshotItemSnapshot, SyncPageSnapshot};
use infiltrator_contract::system_proxy::SystemProxySnapshot;
use infiltrator_contract::tray_status::TrayIconColorMode;
use infiltrator_contract::uwp::UwpLoopbackSnapshot;
use infiltrator_domain::app_routing::AppRoutingConfig;
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::connection::ConnectionObservationSnapshot;
use infiltrator_ports::runtime_configuration::ObservedValue;
use lurker_core_contract::page::{PageData, PageStatus};

#[cfg(test)]
#[path = "settings_observation_tests.rs"]
mod settings_observation_tests;

#[allow(clippy::too_many_arguments)]
pub(super) async fn build_dns_page(
    configuration: Option<&ConfigurationApplication>,
    runtime_config: Option<&Result<RuntimeConfigurationReadout, Failure>>,
    dns_latency: Option<&DnsLatencyApplication>,
    stun_probe: Option<&StunProbeApplication>,
    runtime_connections: Option<&Result<ConnectionObservationSnapshot, PortError>>,
    port_conflicts: &PortConflictSnapshot,
) -> PageData<surface_snapshot::DnsPageSnapshot> {
    let latency = latency_report(dns_latency);
    let stun = stun_report(stun_probe);
    let connections = runtime_connections
        .and_then(|result| result.as_ref().ok())
        .map(ConnectionObservationSnapshot::snapshot);
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
            return PageData::ready(snapshot);
        }
    }
    match runtime_config {
        Some(Ok(config)) => {
            let nameserver = match &config.dns.value.nameserver {
                ObservedValue::Reported(nameserver) => nameserver.clone(),
                ObservedValue::Unreported => {
                    return PageData::empty(surface_snapshot::DnsPageSnapshot {
                        enhanced_mode: DnsEnhancedMode::Unmapped,
                        latency,
                        stun,
                        ..surface_snapshot::DnsPageSnapshot::default()
                    });
                }
            };
            let fallback = match &config.dns.value.fallback {
                ObservedValue::Reported(fallback) => fallback.clone(),
                ObservedValue::Unreported => Vec::new(),
            };
            let enhanced_mode = match &config.dns.value.enhanced_mode {
                ObservedValue::Reported(mode) => Some(mode.as_str()),
                ObservedValue::Unreported => None,
            };
            PageData::ready(surface_snapshot::DnsPageSnapshot {
                enhanced_mode: DnsEnhancedMode::from_config_value(enhanced_mode),
                servers: dns_servers_from_lists(nameserver, fallback),
                latency: latency.clone(),
                stun: stun.clone(),
                ..surface_snapshot::DnsPageSnapshot::default()
            })
        }
        Some(Err(failure)) => PageData::failed(failure.clone()),
        None => PageData::unavailable(missing("DNS reader")),
    }
}

pub(super) fn app_routing_page(
    config: AppRoutingConfig,
    uwp_loopback: UwpLoopbackSnapshot,
) -> surface_snapshot::AppRoutingPageSnapshot {
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
                rule: rule.into(),
                is_system: false,
            }
        })
        .collect();
    surface_snapshot::AppRoutingPageSnapshot {
        mode: config.mode.into(),
        include_system: false,
        apps,
        uwp_loopback,
    }
}

pub(super) async fn build_sync_page(
    settings: Option<&Result<AppSettings, Failure>>,
    profiles: Option<&ProfileApplication>,
    snapshots: Option<&SnapshotApplication>,
) -> PageData<SyncPageSnapshot> {
    let settings = match settings {
        Some(Ok(settings)) => settings,
        Some(Err(failure)) => return PageData::failed(failure.clone()),
        None => return PageData::unavailable(missing("settings application")),
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
    PageData::ready(SyncPageSnapshot {
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
) -> PageData<surface_snapshot::SettingsPageSnapshot> {
    let settings = match settings {
        Some(Ok(settings)) => settings,
        Some(Err(error)) => {
            return PageData::failed(Failure::new(
                ErrorCode::Storage,
                error.message.clone(),
                true,
            ));
        }
        None => return PageData::unavailable(missing("settings application")),
    };
    PageData::ready(surface_snapshot::SettingsPageSnapshot {
        close_to_tray: Some(settings.close_to_tray),
        notifications_enabled: Some(settings.notifications_enabled),
        language: settings.language.clone(),
        theme: settings.theme.clone(),
        autostart: false,
        system_proxy: system_proxy.is_enabled(),
        // The LAN sharing configuration is one owner readback: the page
        // publishes it whole or not at all, so a consumer can tell "sharing
        // is off" from "the runtime configuration was never observed".
        lan_sharing: observed.lan_sharing.clone(),
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
        refresh_current_profile_on_start: settings.refresh_current_profile_on_start,
        custom_accent_color: settings.custom_accent_color.clone(),
        tray_icon_color_mode: if settings.tray_icon_color_mode == "monochrome" {
            TrayIconColorMode::Monochrome
        } else {
            TrayIconColorMode::Colored
        },
        sidebar_expansion: settings.sidebar_expansion,
        connection_view: settings.connection_view.clone(),
    })
}

/// The profiles page, assembled from the profile application's own answers.
///
/// The reader gathers the facts; this builder owns the page's shape, so the
/// reader does not carry a second copy of the profile page's rules.
#[allow(clippy::too_many_arguments)]
pub(super) async fn build_profiles_page(
    profiles_app: Option<&ProfileApplication>,
    profile_result: Option<Result<Vec<ProfileInfo>, Failure>>,
    apply_transaction: Option<ApplyTransactionSnapshot>,
    snapshot_history: Option<SnapshotHistorySnapshot>,
) -> PageData<surface_snapshot::ProfilesPageSnapshot> {
    if profiles_app.is_some() {
        // DUAL-08-13: the template library sidecar. A store without one
        // answers with a typed unsupported, published as `available =
        // false` rather than as a fake empty library.
        let (aggregation_templates, aggregation_templates_available) = match profiles_app {
            Some(profiles) => match profiles.load_aggregation_templates().await {
                Ok(templates) => (templates, true),
                Err(_) => (Vec::new(), false),
            },
            None => (Vec::new(), false),
        };
        let named_overrides = match profiles_app {
            Some(profiles) => profiles
                .load_named_override_catalog()
                .await
                .unwrap_or_default(),
            None => NamedOverrideCatalog::default(),
        };
        match profile_result {
            Some(Ok(items)) if items.is_empty() => {
                PageData::empty(surface_snapshot::ProfilesPageSnapshot {
                    profiles: Vec::new(),
                    auto_update_interval_hours: 0,
                    updating: false,
                    aggregation: last_aggregation_report(),
                    aggregation_templates,
                    aggregation_templates_available,
                    snapshot_history: snapshot_history.clone(),
                    apply_transaction: apply_transaction.clone(),
                    named_overrides,
                })
            }
            Some(Ok(items)) => {
                let mut snapshots = Vec::with_capacity(items.len());
                // DUAL-07-14: the page-level interval is the shortest fixed
                // interval actually scheduled; cron-only profiles are not
                // counted, so the header can stay honest.
                let auto_update_interval_hours = items
                    .iter()
                    .filter(|item| item.auto_update_enabled)
                    .filter_map(|item| item.update_interval_hours)
                    .min()
                    .unwrap_or(0);
                for item in items {
                    // DUAL-07-08: surface the stored node-keyword filter so
                    // both surfaces can prefill the editor.
                    let (filter, filter_source) = match profiles_app {
                        Some(profiles) => match ProfileId::new(item.name.clone()) {
                            Ok(profile_id) => match profiles.load_workspace(&profile_id).await {
                                Ok(workspace) => match profiles.inspect_profile_options(
                                    &workspace.document.identity,
                                    workspace.sidecar_bytes.as_deref(),
                                ) {
                                    Ok(options) => (options.filter, Ok(workspace.identity())),
                                    Err(error) => (Default::default(), Err(error)),
                                },
                                Err(failure) => (Default::default(), Err(failure)),
                            },
                            Err(error) => (
                                Default::default(),
                                Err(Failure::new(
                                    ErrorCode::InvalidInput,
                                    error.to_string(),
                                    false,
                                )),
                            ),
                        },
                        None => (
                            Default::default(),
                            Err(Failure::unsupported(
                                "profile workspace reader is unavailable",
                            )),
                        ),
                    };
                    // DUAL-09-12: the same classification the write guard
                    // enforces, derived from the real subscription source.
                    let write_protection = ProfileWriteProtection::from_subscription_url(
                        item.subscription_url.as_deref().unwrap_or_default(),
                    );
                    snapshots.push(surface_snapshot::ProfileSnapshot {
                        id: item.name.clone(),
                        name: item.name,
                        url: item.subscription_url.unwrap_or_default(),
                        updated_at: item
                            .last_updated
                            .map(|value| value.to_rfc3339())
                            .unwrap_or_default(),
                        upload_bytes: item.traffic_upload,
                        download_bytes: item.traffic_download,
                        total_bytes: item.traffic_total,
                        is_active: item.active,
                        user_agent: item.user_agent.unwrap_or_default(),
                        insecure_skip_verify: item.insecure_skip_verify,
                        etag: item.etag,
                        last_modified: item.last_modified,
                        has_backup: item.has_backup,
                        cron_expression: item.cron_expression,
                        auto_update_enabled: item.auto_update_enabled,
                        update_interval_hours: item.update_interval_hours,
                        next_update: item.next_update.map(|value| value.to_rfc3339()),
                        auto_reload_core: item.auto_reload_core,
                        filter,
                        filter_source,
                        write_protection,
                    });
                }
                PageData::ready(surface_snapshot::ProfilesPageSnapshot {
                    profiles: snapshots,
                    auto_update_interval_hours,
                    updating: false,
                    aggregation: last_aggregation_report(),
                    aggregation_templates,
                    aggregation_templates_available,
                    // DUAL-09-06/11/14: the shared history, the host apply
                    // transaction and loaded editor document are separate observations.
                    // History comes from this product instance and active profile.
                    snapshot_history: snapshot_history.clone(),
                    apply_transaction: apply_transaction.clone(),
                    named_overrides,
                })
            }
            Some(Err(failure)) => PageData::failed(failure),
            None => PageData::unavailable(missing("profile application")),
        }
    } else {
        PageData::unavailable(missing("profile application"))
    }
}
