use super::*;

use crate::pages::app_routing::{AppRouteRule, AppRoutingMode, AppRoutingProjection};
use crate::pages::connections::ConnectionsProjection;
use crate::pages::dns::DnsProjection;
use crate::pages::doctor::{DoctorCheckState, DoctorProjection};
use crate::pages::logs::LogsProjection;
use crate::pages::profiles::ProfilesProjection;
use crate::pages::proxies::ProxiesProjection;
use crate::pages::rules::RulesProjection;
use crate::pages::settings::settings_core::SettingsProjection;
use crate::pages::sync::{SyncProjection, SyncStatus};
use crate::projection::OverviewState;
use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::dns::DnsEnhancedMode;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};

pub(super) fn snapshot_from_overview(
    overview: &OverviewProjection,
    demo_pages: bool,
) -> surface_snapshot::SurfaceSnapshot {
    let core = CoreSnapshot {
        lifecycle: match overview.state {
            OverviewState::Running => CoreLifecycle::Running,
            OverviewState::Stopped => CoreLifecycle::Stopped,
            OverviewState::Unavailable => CoreLifecycle::Failed,
        },
        generation: 1,
        session_token: None,
        revision: 1,
        proxy_mode: Some(overview.mode),
        core_version: overview.core_version.clone(),
        sampled_at_epoch_ms: i64::try_from(overview.sampled_at.as_millis()).ok(),
        failure: overview.failure.as_ref().map(|message| {
            infiltrator_contract::error::Failure::new(
                infiltrator_contract::error::ErrorCode::NotReady,
                message.clone(),
                true,
            )
        }),
        upload_bps: overview.upload_bps,
        download_bps: overview.download_bps,
        active_connections: overview.active_connections,
        memory_bytes: overview.memory_bytes,
        watchdog: Default::default(),
    };
    let overview_data = surface_snapshot::OverviewPageSnapshot {
        proxy_mode: Some(overview.mode),
        upload_bps: overview.upload_bps,
        download_bps: overview.download_bps,
        active_connections: overview.active_connections,
        memory_bytes: overview.memory_bytes,
        core_version: overview.core_version.clone(),
    };
    let pages = if demo_pages {
        demo_pages_with_overview(overview_data)
    } else {
        surface_snapshot::SurfacePages {
            overview: surface_snapshot::PageData::ready(overview_data),
            proxies: surface_snapshot::PageData::unavailable(page_not_composed()),
            profiles: surface_snapshot::PageData::unavailable(page_not_composed()),
            rules: surface_snapshot::PageData::unavailable(page_not_composed()),
            connections: surface_snapshot::PageData::unavailable(page_not_composed()),
            logs: surface_snapshot::PageData::unavailable(page_not_composed()),
            dns: surface_snapshot::PageData::unavailable(page_not_composed()),
            doctor: surface_snapshot::PageData::unavailable(page_not_composed()),
            app_routing: surface_snapshot::PageData::unavailable(page_not_composed()),
            sync: surface_snapshot::PageData::unavailable(page_not_composed()),
            settings: surface_snapshot::PageData::unavailable(page_not_composed()),
        }
    };
    surface_snapshot::SurfaceSnapshot {
        surface: SurfaceKind::BevyDesktop,
        origin: if demo_pages {
            surface_snapshot::SurfaceOrigin::Demo
        } else {
            surface_snapshot::SurfaceOrigin::Live
        },
        generation: core.generation,
        revision: core.revision,
        core,
        capabilities: CapabilitySnapshot::new(HostKind::Desktop, 1, Vec::new()),
        failure: overview.failure.as_ref().map(|message| {
            infiltrator_contract::error::Failure::new(
                infiltrator_contract::error::ErrorCode::NotReady,
                message.clone(),
                true,
            )
        }),
        pages,
        versions: infiltrator_contract::version::CoreVersionSnapshot::default(),
        controller_auth: infiltrator_contract::controller::ControllerAuthSnapshot::default(),
        service_mode: infiltrator_contract::service_mode::ServiceModeSnapshot::default(),
        port_conflicts: infiltrator_contract::port_conflict::PortConflictSnapshot::default(),
        resources: infiltrator_contract::resources::CoreResourceSnapshot {
            cpu_percent: Some(2.4),
            ..Default::default()
        },
        offline_startup: infiltrator_contract::offline_startup::OfflineStartupSnapshot::default(),
        mtu: infiltrator_contract::mtu::MtuNegotiationSnapshot::default(),
        system_proxy: if demo_pages {
            demo_system_proxy()
        } else {
            infiltrator_contract::system_proxy::SystemProxySnapshot::default()
        },
        system_proxy_recovery:
            infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot::default(),
        network_roaming: if demo_pages {
            SettingsProjection::demo().network_roaming
        } else {
            infiltrator_contract::network_roaming::NetworkRoamingSnapshot::default()
        },
        vpn: if demo_pages {
            infiltrator_contract::vpn::VpnSessionSnapshot::unsupported(
                1,
                "Android VpnService is not part of the desktop demo host",
            )
        } else {
            infiltrator_contract::vpn::VpnSessionSnapshot::default()
        },
        privileged_network:
            infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot::unsupported(
                1,
                "privileged network regression is a host-test capability",
            ),
        traffic_waveform: infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot::default(
        ),
        traffic_scale: infiltrator_contract::traffic_scale::TrafficScaleSnapshot::default(),
        traffic_topology: overview.traffic_topology.clone(),
        active_exit: overview.active_exit.clone(),
        public_ip: overview.public_ip.clone(),
        overview_layout: overview.layout.clone(),
        reconnect_mask: overview.reconnect_mask.clone(),
        viewport: overview.viewport.clone(),
        subscription_quota: overview.subscription_quota.clone(),
        yaml_ast_diff: None,
        script_sandbox: None,
        script_export: None,
        speedtest: infiltrator_contract::speedtest::SpeedtestSnapshot::default(),
    }
}

pub(super) fn demo_snapshot() -> surface_snapshot::SurfaceSnapshot {
    let overview = crate::projection::DemoOverviewSource::running().current();
    let core = snapshot_from_overview(&overview, true).core;
    let pages = demo_pages_with_overview(surface_snapshot::OverviewPageSnapshot {
        proxy_mode: Some(overview.mode),
        upload_bps: overview.upload_bps,
        download_bps: overview.download_bps,
        active_connections: overview.active_connections,
        memory_bytes: overview.memory_bytes,
        core_version: None,
    });
    surface_snapshot::SurfaceSnapshot {
        surface: SurfaceKind::BevyDesktop,
        origin: surface_snapshot::SurfaceOrigin::Demo,
        generation: core.generation,
        revision: core.revision,
        core,
        capabilities: CapabilitySnapshot::new(HostKind::Desktop, 1, Vec::new()),
        failure: None,
        pages,
        versions: infiltrator_contract::version::CoreVersionSnapshot::default(),
        controller_auth: infiltrator_contract::controller::ControllerAuthSnapshot::default(),
        service_mode: infiltrator_contract::service_mode::ServiceModeSnapshot::default(),
        port_conflicts: infiltrator_contract::port_conflict::PortConflictSnapshot::default(),
        resources: infiltrator_contract::resources::CoreResourceSnapshot {
            cpu_percent: Some(2.4),
            ..Default::default()
        },
        offline_startup: infiltrator_contract::offline_startup::OfflineStartupSnapshot::default(),
        mtu: infiltrator_contract::mtu::MtuNegotiationSnapshot::default(),
        system_proxy: demo_system_proxy(),
        system_proxy_recovery:
            infiltrator_contract::system_proxy::SystemProxyRecoverySnapshot::default(),
        network_roaming: SettingsProjection::demo().network_roaming,
        vpn: infiltrator_contract::vpn::VpnSessionSnapshot::unsupported(
            1,
            "Android VpnService is not part of the desktop demo host",
        ),
        privileged_network:
            infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot::unsupported(
                1,
                "privileged network regression is a host-test capability",
            ),
        traffic_waveform: infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot::default(
        ),
        traffic_scale: infiltrator_contract::traffic_scale::TrafficScaleSnapshot::default(),
        traffic_topology: overview.traffic_topology.clone(),
        active_exit: overview.active_exit.clone(),
        public_ip: overview.public_ip.clone(),
        overview_layout: overview.layout.clone(),
        reconnect_mask: overview.reconnect_mask.clone(),
        viewport: overview.viewport.clone(),
        subscription_quota: overview.subscription_quota.clone(),
        yaml_ast_diff: None,
        script_sandbox: None,
        script_export: None,
        speedtest: infiltrator_contract::speedtest::SpeedtestSnapshot::default(),
    }
}

fn demo_system_proxy() -> infiltrator_contract::system_proxy::SystemProxySnapshot {
    infiltrator_contract::system_proxy::SystemProxySnapshot::from_observation(
        1,
        infiltrator_contract::system_proxy::SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        },
    )
}

fn demo_pages_with_overview(
    overview: surface_snapshot::OverviewPageSnapshot,
) -> surface_snapshot::SurfacePages {
    let proxies: surface_snapshot::ProxiesPageSnapshot = ProxiesProjection::demo().into();
    let profiles: surface_snapshot::ProfilesPageSnapshot = ProfilesProjection::demo().into();
    let rules: surface_snapshot::RulesPageSnapshot = RulesProjection::demo().into();
    let connections: surface_snapshot::ConnectionsPageSnapshot =
        ConnectionsProjection::demo().into();
    let logs: surface_snapshot::LogsPageSnapshot = LogsProjection::demo().into();
    let dns: surface_snapshot::DnsPageSnapshot = DnsProjection::demo().into();
    let doctor: surface_snapshot::DoctorPageSnapshot = DoctorProjection::demo().into();
    let app_routing: surface_snapshot::AppRoutingPageSnapshot = AppRoutingProjection::demo().into();
    let sync: surface_snapshot::SyncPageSnapshot = SyncProjection::demo().into();
    let settings: surface_snapshot::SettingsPageSnapshot = SettingsProjection::demo().into();
    surface_snapshot::SurfacePages {
        overview: surface_snapshot::PageData::ready(overview),
        proxies: surface_snapshot::PageData::ready(proxies),
        profiles: surface_snapshot::PageData::ready(profiles),
        rules: surface_snapshot::PageData::ready(rules),
        connections: surface_snapshot::PageData::ready(connections),
        logs: surface_snapshot::PageData::ready(logs),
        dns: surface_snapshot::PageData::ready(dns),
        doctor: surface_snapshot::PageData::ready(doctor),
        app_routing: surface_snapshot::PageData::ready(app_routing),
        sync: surface_snapshot::PageData::ready(sync),
        settings: surface_snapshot::PageData::ready(settings),
    }
}

fn page_not_composed() -> infiltrator_contract::error::Failure {
    infiltrator_contract::error::Failure::new(
        infiltrator_contract::error::ErrorCode::NotReady,
        "page reader is not composed for this host",
        true,
    )
}

pub(crate) fn empty_proxies() -> ProxiesProjection {
    ProxiesProjection {
        groups: Vec::new(),
        testing: false,
        active_exit: "—".to_owned(),
        custom_node: Default::default(),
    }
}

pub(crate) fn empty_profiles() -> ProfilesProjection {
    ProfilesProjection {
        profiles: Vec::new(),
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
        yaml_ast_diff: None,
        snapshot_history: None,
        apply_transaction: None,
        profile_document: None,
        profile_options: None,
        script_sandbox: None,
        script_export: None,
    }
}

pub(crate) fn empty_rules() -> RulesProjection {
    RulesProjection {
        total_rules: 0,
        default_action: "—".to_owned(),
        providers: Vec::new(),
        rules: Vec::new(),
        tracer: Default::default(),
        hit_audit: Default::default(),
        mrs_acceleration: Default::default(),
        truncated_rule_count: None,
        rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    }
}

pub(crate) fn empty_connections() -> ConnectionsProjection {
    ConnectionsProjection {
        total_connections: 0,
        total_upload_bytes: 0,
        total_download_bytes: 0,
        stream_phase: infiltrator_contract::connection::ConnectionStreamPhase::Idle,
        connections: Vec::new(),
    }
}

pub(crate) fn empty_logs() -> LogsProjection {
    LogsProjection {
        total_entries: 0,
        active_level: None,
        entries: Vec::new(),
    }
}

pub(crate) fn empty_dns() -> DnsProjection {
    DnsProjection {
        mode: DnsEnhancedMode::Unmapped,
        cache_entries: 0,
        fake_ip_range: "—".to_owned(),
        servers: Vec::new(),
        switches: infiltrator_contract::dns::DnsCoreSwitches::default(),
        filter_mode: infiltrator_contract::dns::DnsFakeIpFilterMode::default(),
        form: infiltrator_contract::dns_form::DnsWorkbenchForm::default(),
        cache_flush: infiltrator_contract::dns::DnsCacheFlushReport::default(),
        fake_ip_pool: infiltrator_contract::dns::FakeIpMappingPool::default(),
        latency: infiltrator_contract::dns_latency::DnsLatencyReport::default(),
        leak: infiltrator_contract::dns_leak::DnsLeakReport::default(),
        self_heal: infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot::default(),
        hosts: Vec::new(),
    }
}

pub(crate) fn empty_doctor() -> DoctorProjection {
    DoctorProjection {
        overall_healthy: false,
        last_run: "—".to_owned(),
        checks: Vec::new(),
        watchdog: Default::default(),
    }
}

pub(crate) fn empty_app_routing() -> AppRoutingProjection {
    AppRoutingProjection {
        mode: AppRoutingMode::ProxyAll,
        include_system: false,
        apps: Vec::new(),
        uwp_loopback: Default::default(),
    }
}

pub(crate) fn empty_sync() -> SyncProjection {
    SyncProjection {
        status: SyncStatus::Disconnected,
        server_url: String::new(),
        username: String::new(),
        last_sync: None,
        auto_sync: false,
        conflict: None,
        snapshots: Vec::new(),
    }
}

pub(crate) fn empty_settings() -> SettingsProjection {
    SettingsProjection {
        autostart: false,
        system_proxy: false,
        system_proxy_snapshot: Default::default(),
        system_proxy_recovery: Default::default(),
        mixed_port: 0,
        allow_lan: false,
        lan_bind_address: infiltrator_contract::lan::DEFAULT_BIND_ADDRESS.to_owned(),
        lan_security: Default::default(),
        ipv6_routing: Default::default(),
        pac: Default::default(),
        network_roaming: Default::default(),
        vpn: Default::default(),
        privileged_network: Default::default(),
        tun_enabled: false,
        tun_stack: String::new(),
        tun_auto_route: false,
        tun_strict_route: false,
        controller_port: 0,
        log_level: String::new(),
        core_channel: String::new(),
        core_versions: Default::default(),
        core_integrity: Default::default(),
        controller_auth: Default::default(),
        service_mode: Default::default(),
        port_conflicts: Default::default(),
        core_resources: Default::default(),
        offline_startup: Default::default(),
        mtu: Default::default(),
        mini_hud: Default::default(),
    }
}

impl From<ProxiesProjection> for surface_snapshot::ProxiesPageSnapshot {
    fn from(value: ProxiesProjection) -> Self {
        Self {
            custom_node: value.custom_node.clone(),
            groups: value
                .groups
                .into_iter()
                .map(|group| surface_snapshot::ProxyGroupSnapshot {
                    name: group.name,
                    group_type: group.group_type.clone(),
                    classification:
                        infiltrator_contract::proxies::ProxyGroupClassification::from_str_loose(
                            &group.group_type,
                        ),
                    current: group.current,
                    expanded: group.expanded,
                    proxies: group
                        .proxies
                        .into_iter()
                        .map(|proxy| surface_snapshot::ProxyNodeSnapshot {
                            name: proxy.name,
                            node_type: proxy.node_type,
                            delay_ms: proxy.delay_ms,
                            selected: proxy.selected,
                            favorite: proxy.favorite,
                            features: proxy.features,
                        })
                        .collect(),
                })
                .collect(),
            testing: value.testing,
            active_exit: value.active_exit,
            filter_alive: Default::default(),
            sort_order: Default::default(),
            compact_view: false,
        }
    }
}

impl From<ProfilesProjection> for surface_snapshot::ProfilesPageSnapshot {
    fn from(value: ProfilesProjection) -> Self {
        Self {
            profiles: value
                .profiles
                .into_iter()
                .map(|profile| surface_snapshot::ProfileSnapshot {
                    id: profile.id,
                    name: profile.name,
                    url: profile.url,
                    updated_at: profile.updated_at,
                    upload_bytes: profile.upload_bytes,
                    download_bytes: profile.download_bytes,
                    total_bytes: profile.total_bytes,
                    is_active: profile.is_active,
                    user_agent: profile.user_agent,
                    insecure_skip_verify: profile.insecure_skip_verify,
                    etag: profile.etag,
                    last_modified: profile.last_modified,
                    has_backup: profile.has_backup,
                    cron_expression: profile.cron_expression,
                    auto_update_enabled: profile.auto_update_enabled,
                    update_interval_hours: profile.update_interval_hours,
                    next_update: profile.next_update,
                    auto_reload_core: profile.auto_reload_core,
                    filter: profile.filter,
                    write_protection: profile.write_protection,
                })
                .collect(),
            auto_update_interval_hours: value.auto_update_interval_hours,
            updating: value.updating,
            snapshot_history: value.snapshot_history,
            apply_transaction: value.apply_transaction,
            profile_document: value.profile_document,
            profile_options: value.profile_options,
            aggregation: value.aggregation,
            aggregation_templates: value.aggregation_templates,
            aggregation_templates_available: value.aggregation_templates_available,
        }
    }
}

impl From<RulesProjection> for surface_snapshot::RulesPageSnapshot {
    fn from(value: RulesProjection) -> Self {
        Self {
            total_rules: value.total_rules,
            default_action: value.default_action,
            providers: value
                .providers
                .into_iter()
                .map(|provider| surface_snapshot::RuleProviderSnapshot {
                    name: provider.name,
                    rule_count: provider.rule_count,
                    behavior: provider.behavior,
                    updated_at: provider.updated_at,
                    source_url: provider.source_url,
                    refresh_interval_secs: provider.refresh_interval_secs,
                    cache_fingerprint: provider.cache_fingerprint,
                })
                .collect(),
            rules: value
                .rules
                .into_iter()
                .map(|rule| surface_snapshot::RuleSnapshot {
                    id: rule.id,
                    rule_type: rule.rule_type,
                    payload: rule.payload,
                    proxy: rule.proxy,
                    hit_count: rule.hit_count,
                    ..Default::default()
                })
                .collect(),
            tracer: value.tracer,
            mrs_acceleration: value.mrs_acceleration,
            total_hits: 0,
            rule_publish_limit: value.rule_publish_limit,
            provider_cache: value.provider_cache,
            etag_support: value.etag_support,
            json_documents: value.json_documents,
        }
    }
}

impl From<ConnectionsProjection> for surface_snapshot::ConnectionsPageSnapshot {
    fn from(value: ConnectionsProjection) -> Self {
        Self {
            total_connections: value.total_connections,
            total_upload_bytes: value.total_upload_bytes,
            total_download_bytes: value.total_download_bytes,
            connections: value
                .connections
                .into_iter()
                .map(|connection| surface_snapshot::ConnectionSnapshot {
                    id: connection.id,
                    host: connection.host,
                    process: connection.process,
                    rule: connection.rule,
                    rule_payload: connection.rule_payload,
                    chain: connection.chain,
                    chains: connection.chains,
                    network: connection.network,
                    source_ip: connection.source_ip,
                    source_port: connection.source_port,
                    destination_ip: connection.destination_ip,
                    destination_port: connection.destination_port,
                    destination_geo_ip: connection.destination_geo_ip,
                    destination_ip_asn: connection.destination_ip_asn,
                    upload_bps: connection.upload_bps,
                    download_bps: connection.download_bps,
                    upload_total: connection.upload_total,
                    download_total: connection.download_total,
                })
                .collect(),
        }
    }
}

impl From<LogsProjection> for surface_snapshot::LogsPageSnapshot {
    fn from(value: LogsProjection) -> Self {
        Self {
            total_entries: value.total_entries,
            active_level: value
                .active_level
                .map(|level| level.label().to_ascii_lowercase()),
            entries: value
                .entries
                .into_iter()
                .map(|entry| surface_snapshot::LogSnapshot {
                    timestamp: entry.timestamp,
                    level: entry.level.label().to_ascii_lowercase(),
                    tag: entry.tag,
                    message: entry.message,
                })
                .collect(),
        }
    }
}

impl From<DnsProjection> for surface_snapshot::DnsPageSnapshot {
    fn from(value: DnsProjection) -> Self {
        Self {
            enhanced_mode: value.mode,
            cache_entries: value.cache_entries,
            fake_ip_range: value.fake_ip_range,
            switches: value.switches,
            filter_mode: value.filter_mode,
            servers: value
                .servers
                .into_iter()
                .map(|server| surface_snapshot::DnsServerSnapshot {
                    address: server.address,
                    protocol: server.protocol,
                    latency_ms: server.latency_ms,
                    is_fallback: server.is_fallback,
                    tags: server.tags,
                })
                .collect(),
            default_nameserver: infiltrator_contract::dns::parse_server_list(
                &value.form.bootstrap_nameserver,
            ),
            fallback_policy: value.form.fallback_policy.policy(),
            fake_ip_filter: infiltrator_contract::dns::parse_server_list(
                &value.form.fake_ip_filter,
            ),
            proxy_server_nameserver: infiltrator_contract::dns::parse_server_list(
                &value.form.proxy_server_nameserver,
            ),
            direct_nameserver: infiltrator_contract::dns::parse_server_list(
                &value.form.direct_nameserver,
            ),
            cache_flush: value.cache_flush,
            fake_ip_pool: value.fake_ip_pool,
            latency: value.latency,
            leak: value.leak,
            self_heal: value.self_heal,
            hosts: value.hosts,
        }
    }
}

impl From<DoctorProjection> for surface_snapshot::DoctorPageSnapshot {
    fn from(value: DoctorProjection) -> Self {
        Self {
            overall_healthy: value.overall_healthy,
            last_run: value.last_run,
            checks: value
                .checks
                .into_iter()
                .map(|check| surface_snapshot::DoctorCheckSnapshot {
                    id: check.id,
                    name: check.name,
                    category: check.category,
                    state: match check.state {
                        DoctorCheckState::Pass => "pass".to_owned(),
                        DoctorCheckState::Warning => "warning".to_owned(),
                        DoctorCheckState::Fail => "fail".to_owned(),
                    },
                    detail: check.detail,
                    fix_available: check.fix_available,
                })
                .collect(),
        }
    }
}

impl From<AppRoutingProjection> for surface_snapshot::AppRoutingPageSnapshot {
    fn from(value: AppRoutingProjection) -> Self {
        Self {
            mode: match value.mode {
                AppRoutingMode::ProxyAll => "proxy_all".to_owned(),
                AppRoutingMode::BypassList => "proxy_selected".to_owned(),
                AppRoutingMode::ProxyList => "bypass_selected".to_owned(),
            },
            include_system: value.include_system,
            uwp_loopback: value.uwp_loopback,
            apps: value
                .apps
                .into_iter()
                .map(|app| surface_snapshot::AppSnapshot {
                    id: app.id,
                    name: app.name,
                    process_name: app.process_name,
                    rule: match app.rule {
                        AppRouteRule::Proxy => "proxy".to_owned(),
                        AppRouteRule::Direct => "direct".to_owned(),
                        AppRouteRule::Block => "block".to_owned(),
                    },
                    is_system: app.is_system,
                })
                .collect(),
        }
    }
}

impl From<SyncProjection> for surface_snapshot::SyncPageSnapshot {
    fn from(value: SyncProjection) -> Self {
        Self {
            status: match value.status {
                SyncStatus::Connected => "connected".to_owned(),
                SyncStatus::Disconnected => "disconnected".to_owned(),
                SyncStatus::Syncing => "syncing".to_owned(),
                SyncStatus::Conflict => "conflict".to_owned(),
                SyncStatus::Error => "error".to_owned(),
            },
            server_url: value.server_url,
            username: value.username,
            last_sync: value.last_sync,
            auto_sync: value.auto_sync,
            conflict: value
                .conflict
                .map(|conflict| surface_snapshot::SyncConflictSnapshot {
                    remote_device: conflict.remote_device,
                    conflict_time: conflict.conflict_time,
                    conflicting_keys: conflict
                        .conflicting_keys
                        .into_iter()
                        .map(|key| (key.key, key.local_value, key.remote_value))
                        .collect(),
                }),
            snapshots: value
                .snapshots
                .into_iter()
                .map(|item| surface_snapshot::SnapshotItemSnapshot {
                    id: item.id,
                    timestamp: item.timestamp,
                    device: item.device,
                    size_bytes: item.size_bytes,
                })
                .collect(),
        }
    }
}

impl From<SettingsProjection> for surface_snapshot::SettingsPageSnapshot {
    fn from(value: SettingsProjection) -> Self {
        Self {
            autostart: value.autostart,
            system_proxy: value.system_proxy,
            mixed_port: value.mixed_port,
            allow_lan: value.allow_lan,
            lan_bind_address: value.lan_bind_address,
            lan_security: value.lan_security,
            ipv6_routing: value.ipv6_routing,
            pac: value.pac,
            tun_enabled: value.tun_enabled,
            tun_stack: value.tun_stack,
            tun_auto_route: value.tun_auto_route,
            tun_strict_route: value.tun_strict_route,
            controller_port: value.controller_port,
            log_level: value.log_level,
            core_channel: value.core_channel,
            mini_hud: value.mini_hud,
        }
    }
}
