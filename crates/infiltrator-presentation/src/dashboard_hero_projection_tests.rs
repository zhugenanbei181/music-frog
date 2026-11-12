use super::*;
use infiltrator_contract::relative_time::RelativeTimeBucket;
use infiltrator_contract::surface_snapshot::ProfilesPageSnapshot;
use infiltrator_contract::traffic_waveform::TrafficSample;
use lurker_core_contract::page::PageData;
use serde_json::{from_value, json};

#[test]
fn traffic_waveform_projection_peak_and_normalization() {
    let waveform = TrafficWaveformSnapshot {
        generation: 1,
        revision: 1,
        samples: vec![
            TrafficSample {
                sampled_at_epoch_ms: Some(1000),
                upload_bps: 1000.0,
                download_bps: 2000.0,
            },
            TrafficSample {
                sampled_at_epoch_ms: Some(2000),
                upload_bps: 5000.0,
                download_bps: 10000.0,
            },
        ],
    };

    let proj = TrafficWaveformProjection::from_waveform(&waveform, 1048576, 524288);
    assert_eq!(proj.live_down_formatted, "10.0 KB/s");
    assert_eq!(proj.live_up_formatted, "5.0 KB/s");
    assert_eq!(proj.peak_down_formatted, "峰值 10.0 KB/s");
    assert_eq!(proj.peak_up_formatted, "峰值 5.0 KB/s");
    assert_eq!(proj.normalized_down_points, vec![0.2, 1.0]);
    assert_eq!(proj.normalized_up_points, vec![0.2, 1.0]);
}

#[test]
fn memory_info_card_projection_proportions() {
    let proj = MemoryInfoCardProjection::from_bytes(100, 300);
    assert_eq!(proj.app_fraction, 0.25);
    assert_eq!(proj.core_fraction, 0.75);
}

#[test]
fn service_health_matrix_grading() {
    let proj = ServiceHealthMatrixProjection::from_latencies(Some(80), Some(200), Some(500), None);
    assert_eq!(proj.services[0].status_role, "good");
    assert_eq!(proj.services[1].status_role, "warning");
    assert_eq!(proj.services[2].status_role, "poor");
    assert_eq!(proj.services[3].status_role, "dim");
}

#[test]
fn quick_switches_projection_states() {
    let switches =
        QuickSwitchesProjection::from_states(&SystemProxyStatus::Enabled, true, true, None);
    assert!(switches.system_proxy_enabled);
    assert_eq!(switches.system_proxy_state_text, "已开启");
    assert!(switches.tun_enabled);
    assert_eq!(switches.tun_state_text, "已开启");
    assert!(!switches.lan_share_enabled);
}

#[test]
fn hero_and_mode_selector_projection_snapshots() {
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use lurker_core_contract::error::Failure;
    let mut snap = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("test"),
    );
    snap.runtime_status.core_process = CoreProcessObservation::Running;
    snap.public_ip.ip = Some("189.24.110.20".to_string());
    snap.public_ip.country_code = Some("DE".to_string());
    snap.public_ip.isp = Some("NetLab Global".to_string());

    let hero = DashboardHeroProjection::from_snapshot(&snap);
    assert!(hero.is_running);
    assert_eq!(hero.status_label, "运行中");
    assert_eq!(hero.status_role, "running");
    assert_eq!(hero.exit_ip, "189.24.110.20");
    assert_eq!(hero.exit_country_code, "DE");
    assert_eq!(hero.fact_row.organization.value.text(), "NetLab Global");
    assert_eq!(hero.fact_row.organization.alignment, FactAlignment::End);
    assert!(!hero.fact_row.uptime.value.is_observed());

    let mode = OutboundModeSelectorProjection::from_snapshot(&snap);
    assert_eq!(mode.selected, OutboundModeChoice::Rule);
    assert!(mode.is_interactive);
}

#[test]
fn the_hero_fact_row_reads_the_session_length_the_owner_published() {
    use infiltrator_contract::snapshot::CoreSessionObservation;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use lurker_core_contract::error::Failure;

    let mut snap = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("test"),
    );
    // The owner publishes the session it runs; the projection formats the
    // length it measured and never counts on a clock of its own.
    snap.core.session = CoreSessionObservation::Active {
        started_at_epoch_ms: 1_700_000_000_000,
        elapsed_ms: 11_089_000,
    };

    let hero = DashboardHeroProjection::from_snapshot(&snap);
    assert!(hero.fact_row.uptime.value.is_observed());
    assert_eq!(hero.fact_row.uptime.value.text(), "03:04:49");
    assert_eq!(hero.fact_row.uptime.alignment, FactAlignment::Start);

    // An owner that published no session leaves the fact unobserved; the
    // projection never substitutes a duration of its own.
    snap.core.session = CoreSessionObservation::Inactive;
    let hero = DashboardHeroProjection::from_snapshot(&snap);
    assert!(!hero.fact_row.uptime.value.is_observed());
    assert_eq!(hero.fact_row.uptime.value.text(), "—");
}

#[test]
fn profile_quick_card_and_connection_table_projections() {
    use ConnectionSnapshot;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use lurker_core_contract::error::Failure;

    let snap = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("test"),
    );

    let card = ProfileQuickCardProjection::from_snapshot(&snap);
    assert_eq!(card.profile_name, "默认配置.yaml");
    assert_eq!(card.source, ProfileSourceObservation::Unobserved);
    assert_eq!(card.update_age, ProfileUpdateAge::Unobserved);

    let conns = vec![ConnectionSnapshot {
        id: "1".into(),
        start: "0".into(),
        destination_host: "google.com".into(),
        host: "google.com".into(),
        process: "".into(),
        rule: "MATCH".into(),
        rule_payload: "".into(),
        chain: "Proxy".into(),
        chains: vec![],
        network: "tcp".into(),
        source_ip: "127.0.0.1".into(),
        source_port: "1234".into(),
        destination_ip: "8.8.8.8".into(),
        destination_port: "443".into(),
        destination_geo_ip: None,
        destination_ip_asn: "".into(),
        rate_observation: Default::default(),
        upload_bps: 0.0,
        download_bps: 0.0,
        upload_total: 1024,
        download_total: 2048,
    }];

    let table = LiveConnectionMiniTableProjection::from_connections(&conns);
    assert_eq!(table.total_active, 1);
    assert_eq!(table.tcp_count, 1);
    assert_eq!(table.udp_count, 0);
    assert_eq!(table.rows[0].destination, "google.com");
    assert_eq!(table.rows[0].network, "TCP");
}

#[test]
fn profile_quick_card_publishes_one_typed_owner_update_age() {
    use infiltrator_contract::snapshot::CoreSessionObservation;
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use lurker_core_contract::error::Failure;

    let mut snap = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::unsupported("test"),
    );
    snap.core.session = CoreSessionObservation::Active {
        started_at_epoch_ms: 600_000,
        elapsed_ms: 0,
    };
    snap.pages.profiles = PageData::ready(
        from_value::<ProfilesPageSnapshot>(json!({
            "profiles": [{
                "id": "profile-1",
                "name": "Remote Profile",
                "url": "https://example.test/profile",
                "updated_at": "1970-01-01T00:05:00Z",
                "upload_bytes": 0,
                "download_bytes": 0,
                "total_bytes": 0,
                "is_active": true
            }],
            "auto_update_interval_hours": 24,
            "updating": false
        }))
        .expect("typed profiles fixture"),
    );

    let card = ProfileQuickCardProjection::from_snapshot(&snap);
    assert_eq!(
        card.update_age,
        ProfileUpdateAge::Observed {
            bucket: RelativeTimeBucket::Minutes { count: 5 }
        }
    );
}

#[test]
fn policy_group_grid_projection_preserves_ids_and_flags() {
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, EngineId,
    };
    use lurker_core_contract::outbound::details::OutboundSnapshot;
    use lurker_core_contract::outbound::identity::SnapshotRevision;
    use lurker_core_contract::outbound::inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
    };

    let engine = EngineId::new("mihomo").expect("test engine ID is valid");
    let scope = CoreScope {
        identity: CoreIdentity::new(
            engine.clone(),
            CoreInstanceId::new("test").expect("test instance ID is valid"),
            None,
        ),
        generation: CoreGeneration::new(1),
        session_token: None,
    };
    let id = |value: &str| OutboundId::new(engine.clone(), value).expect("test outbound ID");
    let entries = [
        ("PROXY", Some("节点选择")),
        ("HK-01", Some("🇭 香港 01")),
        ("JP-02", Some("日本东京 02")),
    ]
    .into_iter()
    .map(|(value, display_name)| OutboundSnapshot {
        id: id(value),
        display_name: display_name.map(str::to_owned),
    })
    .collect();
    let inventory = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(7),
        OutboundInventoryState::Ready {
            entries,
            collections: vec![OutboundSelectionSet {
                id: id("PROXY"),
                member_ids: vec![id("HK-01"), id("JP-02")],
                availability: SelectionAvailability::Selectable,
                selection: OutboundSelectionState::Ready {
                    selected: id("HK-01"),
                },
            }],
        },
    )
    .expect("test inventory is valid");

    let projection = PolicyGroupGridProjection::from_inventory(&inventory);
    assert!(projection.observed);
    assert_eq!(projection.total_count, 1);
    let group = &projection.groups[0];
    assert_eq!(group.name, "节点选择");
    assert_eq!(group.avatar_char, "节");
    assert_eq!(group.selected_flag_code.as_deref(), Some("HK"));
    assert_eq!(group.members.len(), 2);
    assert_eq!(group.members[1].display_name, "日本东京 02");
}
