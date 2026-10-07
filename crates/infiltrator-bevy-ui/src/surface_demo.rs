use super::*;
use crate::pages::app_routing::AppRoutingProjection;
use crate::pages::connections::ConnectionsProjection;
use crate::pages::dns::DnsProjection;
use crate::pages::doctor::DoctorProjection;
use crate::pages::logs::LogsProjection;
use crate::pages::profiles::ProfilesProjection;
use crate::pages::proxies::ProxiesProjection;
use crate::pages::rules::RulesProjection;
use crate::pages::settings::settings_core::SettingsProjection;
use crate::pages::sync::SyncProjection;
use crate::projection::DemoOverviewSource;
use infiltrator_application::shell_readout_application::ShellReadoutApplication;
use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::controller::ControllerAuthSnapshot;
use infiltrator_contract::dns::{
    DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, FakeIpMappingPool, parse_server_list,
};
use infiltrator_contract::dns_cache::{DnsCacheFlushReport, DnsCacheSnapshot};
use infiltrator_contract::dns_form::DnsWorkbenchForm;
use infiltrator_contract::dns_hosts::DnsHostsProfile;
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_query::DnsQuerySnapshot;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::network_roaming::NetworkRoamingSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::proxies::ProxyGroupClassification;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::rule_provider_snapshot::RuleProviderSnapshot;
use infiltrator_contract::rule_snapshot::RuleSnapshot;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::stun_probe::StunProbeReport;
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_contract::sync_snapshot::{
    SnapshotItemSnapshot, SyncConflictSnapshot, SyncPageSnapshot,
};
use infiltrator_contract::system_proxy::{
    SystemProxyObservation, SystemProxyRecoverySnapshot, SystemProxySnapshot,
};
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_contract::vpn::VpnSessionSnapshot;
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;

pub(crate) fn snapshot_from_overview(
    overview: &OverviewProjection,
    demo_pages: bool,
) -> surface_snapshot::SurfaceSnapshot {
    let core = CoreSnapshot {
        lifecycle: overview.lifecycle.clone(),
        generation: 1,
        session_token: None,
        revision: 1,
        proxy_mode: overview.proxy_mode.current,
        core_version: overview.core_version.clone(),
        sampled_at_epoch_ms: i64::try_from(overview.sampled_at.as_millis()).ok(),
        failure: overview
            .failure
            .as_ref()
            .map(|message| Failure::new(ErrorCode::NotReady, message.clone(), true)),
        upload_bps: overview.upload_bps,
        download_bps: overview.download_bps,
        active_connections: overview.active_connections,
        memory_bytes: overview.memory_bytes,
        watchdog: Default::default(),
    };
    let overview_data = surface_snapshot::OverviewPageSnapshot {
        proxy_mode: overview.proxy_mode.current,
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
    let mut snapshot = surface_snapshot::SurfaceSnapshot {
        shell_readout: Default::default(),
        runtime_control: RuntimeControlSnapshot {
            status: if demo_pages {
                RuntimeControlStatus::Ready
            } else {
                RuntimeControlStatus::Unobserved
            },
            mode: overview.proxy_mode.current,
            script_available: demo_pages.then_some(true),
            tun_enabled: demo_pages.then_some(false),
            ..Default::default()
        },
        probe_settings: Default::default(),
        language_settings: Default::default(),
        dns_cache: DnsCacheSnapshot::default(),
        rule_trace: Default::default(),
        dns_query: DnsQuerySnapshot::unavailable(),
        dns_hosts: if demo_pages {
            surface_snapshot::PageData::ready(DnsHostsProfile {
                profile: "demo".into(),
                entries: DnsProjection::demo().hosts,
                legacy_entries: Vec::new(),
            })
        } else {
            surface_snapshot::unobserved_hosts()
        },
        dns_leak: if demo_pages {
            DnsProjection::demo().leak
        } else {
            DnsLeakReport::default()
        },
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
        failure: overview
            .failure
            .as_ref()
            .map(|message| Failure::new(ErrorCode::NotReady, message.clone(), true)),
        pages,
        versions: CoreVersionSnapshot::default(),
        controller_auth: ControllerAuthSnapshot::default(),
        service_mode: ServiceModeSnapshot::default(),
        port_conflicts: PortConflictSnapshot::default(),
        resources: CoreResourceSnapshot {
            cpu_percent: Some(2.4),
            ..Default::default()
        },
        offline_startup: OfflineStartupSnapshot::default(),
        mtu: MtuNegotiationSnapshot::default(),
        system_proxy: if demo_pages {
            demo_system_proxy()
        } else {
            SystemProxySnapshot::default()
        },
        system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
        network_roaming: if demo_pages {
            SettingsProjection::demo().network_roaming
        } else {
            NetworkRoamingSnapshot::default()
        },
        vpn: if demo_pages {
            VpnSessionSnapshot::unsupported(
                1,
                "Android VpnService is not part of the desktop demo host",
            )
        } else {
            VpnSessionSnapshot::default()
        },
        privileged_network: PrivilegedNetworkSnapshot::unsupported(
            1,
            "privileged network regression is a host-test capability",
        ),
        traffic_waveform: overview.traffic_waveform.clone(),
        traffic_scale: overview.traffic_scale.clone(),
        traffic_topology: overview.traffic_topology.clone(),
        active_exit: overview.active_exit.clone(),
        public_ip: overview.public_ip.clone(),
        overview_layout: overview.layout.clone(),
        reconnect_mask: overview.reconnect_mask.clone(),
        viewport: overview.viewport.clone(),
        subscription_quota: overview.subscription_quota.clone(),
        profile_editor: Default::default(),
        yaml_ast_diff: None,
        script_sandbox: None,
        script_export: None,
        speedtest: overview.speedtest.clone(),
    };
    fill_demo_runtime_fields(&mut snapshot);
    snapshot.shell_readout = ShellReadoutApplication::default().project(&snapshot);
    snapshot.shell_readout.upload_bps = overview.readout.upload_bps.clone();
    snapshot.shell_readout.download_bps = overview.readout.download_bps.clone();
    snapshot.shell_readout.rate_failure = overview.readout.rate_failure.clone();
    snapshot
}

pub(super) fn demo_snapshot() -> surface_snapshot::SurfaceSnapshot {
    let overview = DemoOverviewSource::running().current();
    let core = snapshot_from_overview(&overview, true).core;
    let pages = demo_pages_with_overview(surface_snapshot::OverviewPageSnapshot {
        proxy_mode: overview.proxy_mode.current,
        upload_bps: overview.upload_bps,
        download_bps: overview.download_bps,
        active_connections: overview.active_connections,
        memory_bytes: overview.memory_bytes,
        core_version: None,
    });
    let mut snapshot = surface_snapshot::SurfaceSnapshot {
        shell_readout: Default::default(),
        runtime_control: RuntimeControlSnapshot {
            status: RuntimeControlStatus::Ready,
            mode: overview.proxy_mode.current,
            script_available: Some(true),
            tun_enabled: Some(false),
            ..Default::default()
        },
        probe_settings: Default::default(),
        language_settings: Default::default(),
        dns_cache: DnsCacheSnapshot::default(),
        rule_trace: Default::default(),
        dns_query: DnsQuerySnapshot::unavailable(),
        dns_leak: DnsProjection::demo().leak,
        dns_hosts: surface_snapshot::PageData::ready(DnsHostsProfile {
            profile: "demo".into(),
            entries: DnsProjection::demo().hosts,
            legacy_entries: Vec::new(),
        }),
        surface: SurfaceKind::BevyDesktop,
        origin: surface_snapshot::SurfaceOrigin::Demo,
        generation: core.generation,
        revision: core.revision,
        core,
        capabilities: CapabilitySnapshot::new(HostKind::Desktop, 1, Vec::new()),
        failure: None,
        pages,
        versions: CoreVersionSnapshot::default(),
        controller_auth: ControllerAuthSnapshot::default(),
        service_mode: ServiceModeSnapshot::default(),
        port_conflicts: PortConflictSnapshot::default(),
        resources: CoreResourceSnapshot {
            cpu_percent: Some(2.4),
            ..Default::default()
        },
        offline_startup: OfflineStartupSnapshot::default(),
        mtu: MtuNegotiationSnapshot::default(),
        system_proxy: demo_system_proxy(),
        system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
        network_roaming: SettingsProjection::demo().network_roaming,
        vpn: VpnSessionSnapshot::unsupported(
            1,
            "Android VpnService is not part of the desktop demo host",
        ),
        privileged_network: PrivilegedNetworkSnapshot::unsupported(
            1,
            "privileged network regression is a host-test capability",
        ),
        traffic_waveform: TrafficWaveformSnapshot::default(),
        traffic_scale: TrafficScaleSnapshot::default(),
        traffic_topology: overview.traffic_topology.clone(),
        active_exit: overview.active_exit.clone(),
        public_ip: overview.public_ip.clone(),
        overview_layout: overview.layout.clone(),
        reconnect_mask: overview.reconnect_mask.clone(),
        viewport: overview.viewport.clone(),
        subscription_quota: overview.subscription_quota.clone(),
        profile_editor: Default::default(),
        yaml_ast_diff: None,
        script_sandbox: None,
        script_export: None,
        speedtest: overview.speedtest.clone(),
    };
    fill_demo_runtime_fields(&mut snapshot);
    snapshot.shell_readout = ShellReadoutApplication::default().project(&snapshot);
    snapshot
}

fn demo_system_proxy() -> SystemProxySnapshot {
    SystemProxySnapshot::from_observation(
        1,
        SystemProxyObservation {
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
    let sync: SyncPageSnapshot = SyncProjection::demo().into();
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

fn page_not_composed() -> Failure {
    Failure::new(
        ErrorCode::NotReady,
        "page reader is not composed for this host",
        true,
    )
}

pub(crate) fn empty_proxies() -> ProxiesProjection {
    ProxiesProjection {
        name_runs: Default::default(),
        search_query: String::new(),
        groups: Vec::new(),
        testing: false,
        filter_alive: false,
        compact_view: false,
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
        editor_read: Default::default(),
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
        rule_publish_limit: RULE_PUBLISH_LIMIT,
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
        stream_phase: ConnectionStreamPhase::Idle,
        connections: Vec::new(),
    }
}

pub(crate) fn empty_logs() -> LogsProjection {
    LogsProjection {
        status: surface_snapshot::PageStatus::Loading,
        generation: 0,
        session_token: None,
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
        switches: DnsCoreSwitches::default(),
        filter_mode: DnsFakeIpFilterMode::default(),
        form: DnsWorkbenchForm::default(),
        cache_flush: DnsCacheFlushReport::default(),
        fake_ip_pool: FakeIpMappingPool::default(),
        latency: DnsLatencyReport::default(),
        leak: DnsLeakReport::default(),
        stun: StunProbeReport::default(),
        self_heal: DnsSelfHealSnapshot::default(),
        hosts: Vec::new(),
    }
}

pub(crate) fn empty_doctor() -> DoctorProjection {
    DoctorProjection {
        report_finished_at: None,
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
        status: SyncStatus::Unknown,
        history_status: PageStatus::Loading,
        server_url: String::new(),
        username: String::new(),
        last_sync: None,
        auto_sync: false,
        conflict: None,
        snapshots: Vec::new(),
    }
}

impl From<ProxiesProjection> for surface_snapshot::ProxiesPageSnapshot {
    fn from(value: ProxiesProjection) -> Self {
        use infiltrator_application::proxy_inspection_projection::inspect_sparse_node;
        let mut page = Self {
            name_runs: value.name_runs,
            search_query: value.search_query,
            custom_node: value.custom_node.clone(),
            node_details: Vec::new(),
            groups: value
                .groups
                .into_iter()
                .map(|group| surface_snapshot::ProxyGroupSnapshot {
                    name: group.name,
                    group_type: group.group_type.clone(),
                    classification: ProxyGroupClassification::from_str_loose(&group.group_type),
                    current: group.current,
                    expanded: group.expanded,
                    proxies: group
                        .proxies
                        .into_iter()
                        .map(|proxy| surface_snapshot::ProxyNodeSnapshot {
                            name: proxy.name,
                            node_type: proxy.node_type,
                            delay_ms: proxy.delay_ms,
                            alive: Some(proxy.delay_ms.is_some_and(|delay| delay > 0)),
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
        };
        page.node_details = page
            .groups
            .iter()
            .flat_map(|group| &group.proxies)
            .map(inspect_sparse_node)
            .collect();
        page
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
                    filter_source: profile.filter_source,
                    write_protection: profile.write_protection,
                })
                .collect(),
            auto_update_interval_hours: value.auto_update_interval_hours,
            updating: value.updating,
            snapshot_history: value.snapshot_history,
            apply_transaction: value.apply_transaction,
            aggregation: value.aggregation,
            aggregation_templates: value.aggregation_templates,
            aggregation_templates_available: value.aggregation_templates_available,
        }
    }
}

impl From<RulesProjection> for surface_snapshot::RulesPageSnapshot {
    fn from(value: RulesProjection) -> Self {
        Self {
            document: None,
            total_rules: value.total_rules,
            default_action: value.default_action,
            providers: value
                .providers
                .into_iter()
                .map(|provider| RuleProviderSnapshot {
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
                .map(|rule| RuleSnapshot {
                    raw: rule.raw,
                    source_ip: rule.source_ip,
                    no_resolve: rule.no_resolve,
                    failure: rule.failure,
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
            hit_audit: value.hit_audit,
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
            connections: value.connections,
        }
    }
}

impl From<LogsProjection> for surface_snapshot::LogsPageSnapshot {
    fn from(value: LogsProjection) -> Self {
        Self {
            stream: LogStreamState::Live,
            total_entries: value.total_entries,
            active_level: value
                .active_level
                .map(|level| level.label().to_ascii_lowercase()),
            entries: value
                .entries
                .into_iter()
                .map(|entry| surface_snapshot::LogSnapshot {
                    id: entry.id,
                    raw: None,
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
            default_nameserver: parse_server_list(&value.form.bootstrap_nameserver),
            fallback_policy: value.form.fallback_policy.policy(),
            fake_ip_filter: parse_server_list(&value.form.fake_ip_filter),
            proxy_server_nameserver: parse_server_list(&value.form.proxy_server_nameserver),
            direct_nameserver: parse_server_list(&value.form.direct_nameserver),
            fake_ip_pool: value.fake_ip_pool,
            latency: value.latency,
            stun: value.stun,
            self_heal: value.self_heal,
        }
    }
}

impl From<DoctorProjection> for surface_snapshot::DoctorPageSnapshot {
    fn from(value: DoctorProjection) -> Self {
        Self {
            report_started_at: None,
            report_finished_at: value.report_finished_at,
            overall_healthy: value.overall_healthy,
            last_run: value.last_run,
            checks: value
                .checks
                .into_iter()
                .map(|check| surface_snapshot::DoctorCheckSnapshot {
                    kind: check.kind,
                    detail_copy_key: check.detail_copy_key.clone(),
                    id: check.id,
                    name: check.name,
                    category: check.category,
                    state: check.state,
                    detail: check.detail,
                    fix_available: check.fix_available,
                    hint: check.hint.clone(),
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
                AppRoutingMode::ProxySelected => "proxy_selected".to_owned(),
                AppRoutingMode::BypassSelected => "bypass_selected".to_owned(),
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
                        AppRoutingRule::Proxy => "proxy".to_owned(),
                        AppRoutingRule::Direct => "direct".to_owned(),
                        AppRoutingRule::Block => "block".to_owned(),
                    },
                    is_system: app.is_system,
                })
                .collect(),
        }
    }
}

impl From<SyncProjection> for SyncPageSnapshot {
    fn from(value: SyncProjection) -> Self {
        Self {
            status: value.status,
            history_status: value.history_status,
            server_url: value.server_url,
            username: value.username,
            last_sync: value.last_sync,
            auto_sync: value.auto_sync,
            conflict: value.conflict.map(|conflict| SyncConflictSnapshot {
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
                .map(|item| SnapshotItemSnapshot {
                    profile: item.profile.clone(),
                    id: item.id,
                    timestamp: item.timestamp,
                    device: item.device,
                    size_bytes: item.size_bytes,
                })
                .collect(),
        }
    }
}

fn fill_demo_runtime_fields(snapshot: &mut surface_snapshot::SurfaceSnapshot) {
    if snapshot.origin != surface_snapshot::SurfaceOrigin::Demo {
        return;
    }
    let Some(settings) = &snapshot.pages.settings.data else {
        return;
    };
    let runtime = &mut snapshot.runtime_control;
    runtime.mixed_port = settings.mixed_port;
    runtime.allow_lan = settings.allow_lan;
    runtime.lan_bind_address = settings.lan_bind_address.clone();
    runtime.lan_security = settings.lan_security.clone();
    runtime.ipv6_routing = settings.ipv6_routing;
    runtime.tun_enabled = settings.tun_enabled;
    runtime.tun_stack = settings.tun_stack.clone();
    runtime.tun_auto_route = settings.tun_auto_route;
    runtime.tun_strict_route = settings.tun_strict_route;
    runtime.log_level = settings.log_level.clone();
}
