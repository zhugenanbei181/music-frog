use super::*;
use crate::pages::app_routing::AppRoutingProjection;
use crate::pages::connections::ConnectionsProjection;
use crate::pages::dns::DnsProjection;
use crate::pages::doctor::DoctorProjection;
use crate::pages::logs::LogsProjection;
use crate::pages::profiles::ProfilesProjection;
use crate::pages::rules::RulesProjection;
use crate::pages::settings::settings_core::SettingsProjection;
use crate::pages::sync::SyncProjection;
use crate::projection::DemoOverviewSource;
use infiltrator_application::shell_readout_application::ShellReadoutApplication;
use infiltrator_contract::capability::CapabilitySnapshot;
use infiltrator_contract::controller::ControllerAuthSnapshot;
use infiltrator_contract::core_identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
};
use infiltrator_contract::dns::parse_server_list;
use infiltrator_contract::dns_cache::DnsCacheSnapshot;
use infiltrator_contract::dns_hosts::DnsHostsProfile;
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_contract::dns_query::DnsQuerySnapshot;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::logs::LogStreamState;
use infiltrator_contract::mtu::MtuNegotiationSnapshot;
use infiltrator_contract::network_roaming::NetworkRoamingSnapshot;
use infiltrator_contract::offline_startup::OfflineStartupSnapshot;
use infiltrator_contract::outbound::details::OutboundLatencySample;
use infiltrator_contract::outbound::preferences::OutboundHealthFilter;
use infiltrator_contract::outbound::preferences::OutboundLayout;
use infiltrator_contract::outbound::{
    details::{
        OutboundCollectionKind, OutboundDetail, OutboundDetailsSnapshot, OutboundDetailsState,
        OutboundEndpoint, OutboundFeature, OutboundHealthObservation, OutboundObservation,
        OutboundProbeAvailability, OutboundProtocolObservation, OutboundSnapshot,
        OutboundUdpSupport,
    },
    identity::{OutboundCipherId, OutboundId, OutboundProtocolId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    preferences::{OutboundSortOrder, OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues},
};
use infiltrator_contract::port_conflict::PortConflictSnapshot;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;
use infiltrator_contract::resources::CoreResourceSnapshot;
use infiltrator_contract::rule_provider_snapshot::RuleProviderSnapshot;
use infiltrator_contract::rule_snapshot::RuleSnapshot;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::service_mode::ServiceModeSnapshot;
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::sync_snapshot::{
    SnapshotItemSnapshot, SyncConflictSnapshot, SyncPageSnapshot,
};
use infiltrator_contract::system_proxy::{
    SystemProxyObservation, SystemProxyRecoverySnapshot, SystemProxySnapshot,
};
use infiltrator_contract::traffic_scale::TrafficScaleSnapshot;
use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_contract::vpn::VpnSurfaceObservation;

#[path = "surface_demo/projections.rs"]
mod projections;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverviewFixture {
    DemoPages,
    OverviewOnly,
}

pub(crate) fn snapshot_from_overview(
    overview: &OverviewProjection,
) -> surface_snapshot::SurfaceSnapshot {
    snapshot_from_overview_fixture(overview, OverviewFixture::OverviewOnly)
}

pub(crate) fn demo_snapshot_from_overview(
    overview: &OverviewProjection,
) -> surface_snapshot::SurfaceSnapshot {
    snapshot_from_overview_fixture(overview, OverviewFixture::DemoPages)
}

fn snapshot_from_overview_fixture(
    overview: &OverviewProjection,
    fixture: OverviewFixture,
) -> surface_snapshot::SurfaceSnapshot {
    let core = CoreSnapshot {
        identity: match fixture {
            OverviewFixture::DemoPages => Some(demo_core_identity()),
            OverviewFixture::OverviewOnly => None,
        },
        lifecycle: overview.lifecycle.clone(),
        generation: 1,
        session_token: None,
        revision: 1,
        routing_intent: overview.routing_intent.clone(),
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
        routing_intent: overview.routing_intent.clone(),
        upload_bps: overview.upload_bps,
        download_bps: overview.download_bps,
        active_connections: overview.active_connections,
        memory_bytes: overview.memory_bytes,
        core_version: overview.core_version.clone(),
    };
    let pages = match fixture {
        OverviewFixture::DemoPages => demo_pages_with_overview(overview_data),
        OverviewFixture::OverviewOnly => surface_snapshot::SurfacePages {
            overview: surface_snapshot::PageData::ready(overview_data),
            profiles: surface_snapshot::PageData::unavailable(page_not_composed()),
            rules: surface_snapshot::PageData::unavailable(page_not_composed()),
            connections: surface_snapshot::PageData::unavailable(page_not_composed()),
            logs: surface_snapshot::PageData::unavailable(page_not_composed()),
            dns: surface_snapshot::PageData::unavailable(page_not_composed()),
            doctor: surface_snapshot::PageData::unavailable(page_not_composed()),
            app_routing: surface_snapshot::PageData::unavailable(page_not_composed()),
            sync: surface_snapshot::PageData::unavailable(page_not_composed()),
            settings: surface_snapshot::PageData::unavailable(page_not_composed()),
        },
    };
    let runtime_control = match fixture {
        OverviewFixture::DemoPages => RuntimeControlSnapshot {
            status: RuntimeControlStatus::Ready,
            tun_enabled: Some(false),
            ..Default::default()
        },
        OverviewFixture::OverviewOnly => RuntimeControlSnapshot {
            status: RuntimeControlStatus::Unobserved,
            ..Default::default()
        },
    };
    let mut snapshot = surface_snapshot::SurfaceSnapshot {
        shell_readout: Default::default(),
        runtime_control,
        probe_settings: Default::default(),
        language_settings: Default::default(),
        dns_cache: DnsCacheSnapshot::default(),
        rule_trace: Default::default(),
        dns_query: DnsQuerySnapshot::unavailable(),
        dns_hosts: match fixture {
            OverviewFixture::DemoPages => surface_snapshot::PageData::ready(DnsHostsProfile {
                profile: "demo".into(),
                entries: DnsProjection::demo().hosts,
                legacy_entries: Vec::new(),
            }),
            OverviewFixture::OverviewOnly => surface_snapshot::unobserved_hosts(),
        },
        dns_leak: match fixture {
            OverviewFixture::DemoPages => DnsProjection::demo().leak,
            OverviewFixture::OverviewOnly => DnsLeakReport::default(),
        },
        surface: SurfaceKind::BevyDesktop,
        origin: match fixture {
            OverviewFixture::DemoPages => surface_snapshot::SurfaceOrigin::Demo,
            OverviewFixture::OverviewOnly => surface_snapshot::SurfaceOrigin::Live,
        },
        generation: core.generation,
        revision: core.revision,
        core,
        capabilities: CapabilitySnapshot::new(HostKind::Desktop, 1, Vec::new()),
        core_capability_snapshot: surface_snapshot::PageData::unavailable(Failure::unsupported(
            "demo Core capabilities are unavailable",
        )),
        failure: overview
            .failure
            .as_ref()
            .map(|message| Failure::new(ErrorCode::NotReady, message.clone(), true)),
        pages,
        versions: CoreVersionSnapshot::for_engine(
            EngineId::new("mihomo").expect("demo Mihomo engine ID is valid"),
        ),
        controller_auth: ControllerAuthSnapshot::default(),
        service_mode: ServiceModeSnapshot::default(),
        port_conflicts: PortConflictSnapshot::default(),
        resources: CoreResourceSnapshot {
            cpu_percent: Some(2.4),
            ..Default::default()
        },
        offline_startup: OfflineStartupSnapshot::default(),
        mtu: MtuNegotiationSnapshot::default(),
        system_proxy: match fixture {
            OverviewFixture::DemoPages => demo_system_proxy(),
            OverviewFixture::OverviewOnly => SystemProxySnapshot::default(),
        },
        system_proxy_recovery: SystemProxyRecoverySnapshot::default(),
        network_roaming: match fixture {
            OverviewFixture::DemoPages => SettingsProjection::demo().network_roaming,
            OverviewFixture::OverviewOnly => NetworkRoamingSnapshot::default(),
        },
        vpn: match fixture {
            OverviewFixture::DemoPages => VpnSurfaceObservation::Unsupported {
                reason: "Android VpnService is not part of the desktop demo host".to_owned(),
            },
            OverviewFixture::OverviewOnly => VpnSurfaceObservation::NotApplicable,
        },
        privileged_network: PrivilegedNetworkSnapshot::unsupported(
            1,
            "privileged network regression is a host-test capability",
        ),
        traffic_waveform: overview.traffic_waveform.clone(),
        traffic_scale: overview.traffic_scale.clone(),
        traffic_topology: overview.traffic_topology.clone(),
        active_exit: overview.active_exit.clone(),
        outbound_inventory: match fixture {
            OverviewFixture::DemoPages => {
                surface_snapshot::PageData::ready(demo_outbound_inventory())
            }
            OverviewFixture::OverviewOnly => {
                surface_snapshot::PageData::unavailable(page_not_composed())
            }
        },
        outbound_details: match fixture {
            OverviewFixture::DemoPages => {
                surface_snapshot::PageData::ready(demo_outbound_details())
            }
            OverviewFixture::OverviewOnly => {
                surface_snapshot::PageData::unavailable(page_not_composed())
            }
        },
        outbound_ui_preferences: match fixture {
            OverviewFixture::DemoPages => {
                surface_snapshot::PageData::ready(demo_outbound_preferences())
            }
            OverviewFixture::OverviewOnly => {
                surface_snapshot::PageData::unavailable(page_not_composed())
            }
        },
        protocol_studio: match fixture {
            OverviewFixture::DemoPages => {
                surface_snapshot::PageData::ready(ProtocolStudioSnapshot::default())
            }
            OverviewFixture::OverviewOnly => {
                surface_snapshot::PageData::unavailable(page_not_composed())
            }
        },
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
    let core = demo_snapshot_from_overview(&overview).core;
    let pages = demo_pages_with_overview(surface_snapshot::OverviewPageSnapshot {
        routing_intent: overview.routing_intent.clone(),
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
        core_capability_snapshot: surface_snapshot::PageData::unavailable(Failure::unsupported(
            "demo Core capabilities are unavailable",
        )),
        failure: None,
        pages,
        versions: CoreVersionSnapshot::for_engine(
            EngineId::new("mihomo").expect("demo Mihomo engine ID is valid"),
        ),
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
        vpn: VpnSurfaceObservation::Unsupported {
            reason: "Android VpnService is not part of the desktop demo host".to_owned(),
        },
        privileged_network: PrivilegedNetworkSnapshot::unsupported(
            1,
            "privileged network regression is a host-test capability",
        ),
        traffic_waveform: TrafficWaveformSnapshot::default(),
        traffic_scale: TrafficScaleSnapshot::default(),
        traffic_topology: overview.traffic_topology.clone(),
        active_exit: overview.active_exit.clone(),
        outbound_inventory: surface_snapshot::PageData::ready(demo_outbound_inventory()),
        outbound_details: surface_snapshot::PageData::ready(demo_outbound_details()),
        outbound_ui_preferences: surface_snapshot::PageData::ready(demo_outbound_preferences()),
        protocol_studio: surface_snapshot::PageData::ready(ProtocolStudioSnapshot::default()),
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

fn demo_core_identity() -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new("mihomo").expect("demo Mihomo engine ID is valid"),
        CoreInstanceId::new("demo").expect("demo core instance ID is valid"),
        None,
    )
}

fn demo_outbound_inventory() -> OutboundInventorySnapshot {
    let scope = CoreScope {
        identity: demo_core_identity(),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let id = |value: &str| {
        OutboundId::new(
            EngineId::new("mihomo").expect("demo Mihomo engine ID is valid"),
            value,
        )
        .expect("demo outbound ID is valid")
    };
    let entries = ["HK-01", "JP-02", "SG-03", "US-04"]
        .into_iter()
        .map(|name| OutboundSnapshot {
            id: id(name),
            display_name: Some(name.to_owned()),
        })
        .collect();
    let collection = |name: &str,
                      members: &[&str],
                      availability: SelectionAvailability,
                      selected: &str| OutboundSelectionSet {
        id: id(name),
        member_ids: members.iter().map(|member| id(member)).collect(),
        availability,
        selection: OutboundSelectionState::Ready {
            selected: id(selected),
        },
    };
    OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(1),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![
                collection(
                    "PROXIES",
                    &["HK-01", "JP-02", "SG-03", "US-04"],
                    SelectionAvailability::Selectable,
                    "HK-01",
                ),
                collection(
                    "AUTO",
                    &["HK-01", "JP-02"],
                    SelectionAvailability::Automatic,
                    "HK-01",
                ),
                collection(
                    "STREAMING",
                    &["SG-03", "US-04"],
                    SelectionAvailability::Selectable,
                    "SG-03",
                ),
            ],
        },
    )
    .expect("demo outbound inventory is valid")
}

fn demo_outbound_details() -> OutboundDetailsSnapshot {
    let inventory = demo_outbound_inventory();
    let OutboundInventoryState::Ready {
        entries,
        collections,
    } = inventory.inventory()
    else {
        unreachable!("demo inventory is ready")
    };
    let engine = EngineId::new("mihomo").expect("demo engine ID is valid");
    let mut details = Vec::new();
    for entry in entries {
        let (protocol, display_name, delay_ms) = match entry.id.value() {
            "HK-01" => ("shadowsocks", "Shadowsocks", 38),
            "JP-02" => ("vmess", "VMess", 65),
            "SG-03" => ("trojan", "Trojan", 72),
            "US-04" => ("hysteria2", "Hysteria2", 152),
            _ => ("unknown", "Unknown", 0),
        };
        details.push(OutboundDetail {
            id: entry.id.clone(),
            protocol: OutboundProtocolObservation::Reported(
                OutboundProtocolId::new(engine.clone(), protocol)
                    .expect("demo protocol ID is valid"),
            ),
            protocol_display_name: Some(display_name.to_owned()),
            cipher: if protocol == "shadowsocks" {
                OutboundObservation::Observed(
                    OutboundCipherId::new(engine.clone(), "aes-128-gcm")
                        .expect("demo cipher ID is valid"),
                )
            } else {
                OutboundObservation::Unknown
            },
            cipher_display_name: (protocol == "shadowsocks").then(|| "aes-128-gcm".to_owned()),
            collection_kind: OutboundObservation::Unknown,
            features: OutboundObservation::Observed(if delay_ms > 0 {
                vec![OutboundFeature::Udp]
            } else {
                Vec::new()
            }),
            endpoint: OutboundObservation::Observed(OutboundEndpoint {
                host: format!("{}.demo.invalid", entry.id.value().to_ascii_lowercase()),
                port: OutboundObservation::Observed(443),
            }),
            udp: OutboundUdpSupport::Supported,
            health: OutboundHealthObservation::Healthy,
            current_latency_ms: OutboundObservation::Observed(delay_ms),
            latency_history: OutboundObservation::Observed(vec![OutboundLatencySample {
                observed_at: "demo".to_owned(),
                delay_ms,
            }]),
            rtt: OutboundObservation::Unknown,
            historical_egress: OutboundObservation::Unknown,
            probe_availability: OutboundProbeAvailability::Supported,
        });
    }
    for collection in collections {
        let kind = match collection.id.value() {
            "AUTO" => OutboundCollectionKind::AutomaticSelection,
            _ => OutboundCollectionKind::ManualSelection,
        };
        details.push(OutboundDetail {
            id: collection.id.clone(),
            protocol: OutboundProtocolObservation::Unknown,
            protocol_display_name: None,
            cipher: OutboundObservation::Unknown,
            cipher_display_name: None,
            collection_kind: OutboundObservation::Observed(kind),
            features: OutboundObservation::Unknown,
            endpoint: OutboundObservation::Unknown,
            udp: OutboundUdpSupport::Unknown,
            health: OutboundHealthObservation::Unknown,
            current_latency_ms: OutboundObservation::Unknown,
            latency_history: OutboundObservation::Unknown,
            rtt: OutboundObservation::Unknown,
            historical_egress: OutboundObservation::Unknown,
            probe_availability: OutboundProbeAvailability::Unsupported,
        });
    }
    OutboundDetailsSnapshot::new(
        inventory.scope().clone(),
        SnapshotRevision::new(1),
        OutboundDetailsState::Ready { details },
    )
    .expect("demo outbound details are valid")
}

fn demo_outbound_preferences() -> OutboundUiPreferencesSnapshot {
    let inventory = demo_outbound_inventory();
    OutboundUiPreferencesSnapshot::new(
        inventory.scope().clone(),
        SnapshotRevision::new(1),
        OutboundUiPreferencesValues {
            search_query: String::new(),
            favorite_outbound_ids: Vec::new(),
            group_order: Vec::new(),
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::LatencyAscending,
            health_filter: OutboundHealthFilter::All,
            layout: OutboundLayout::Grid,
        },
    )
    .expect("demo outbound preferences are valid")
}

fn demo_pages_with_overview(
    overview: surface_snapshot::OverviewPageSnapshot,
) -> surface_snapshot::SurfacePages {
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

fn fill_demo_runtime_fields(snapshot: &mut surface_snapshot::SurfaceSnapshot) {
    match snapshot.origin {
        surface_snapshot::SurfaceOrigin::Demo => {}
        surface_snapshot::SurfaceOrigin::Live => return,
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
