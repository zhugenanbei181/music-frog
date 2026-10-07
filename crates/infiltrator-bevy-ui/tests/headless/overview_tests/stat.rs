//! Behavior cases for stat.
//! test-intent: behavior

use super::*;
use infiltrator_contract::proxy_mode::ProxyModeSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;

/// The four stat chips carry labeled Group semantics ("name value") that
/// the refresh observer restamps, and the banner's state word carries a
/// Status semantic that follows the run state.
#[test]
fn stat_chips_and_banner_status_carry_accesskit_semantics() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut chips = world.query::<(&OverviewChip, &AccessibilityNode)>();
    let mut labels: Vec<(OverviewChipKind, String)> = Vec::new();
    for (chip, node) in chips.iter(world) {
        assert_eq!(node.role(), accesskit::Role::Group);
        labels.push((chip.0, node.label().expect("chip group label").to_owned()));
    }
    assert_eq!(labels.len(), 6, "every chip carries one group node");
    assert!(labels.contains(&(OverviewChipKind::Connections, "连接数 12".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Memory, "内存 96.00 MB".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Cpu, "CPU 2.4%".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Upload, "上传 1.40 MB/s".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Download, "下载 8.60 MB/s".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::TotalTraffic, "总流量 98.32 MB".to_owned())));

    let mut lines = world.query::<(&OverviewLine, &AccessibilityNode)>();
    let (_, status) = lines
        .iter(world)
        .find(|(line, _)| line.0 == OverviewLineKind::State)
        .expect("the state line carries semantics");
    assert_eq!(status.role(), accesskit::Role::Status);
    assert_eq!(status.label(), Some("运行中"));

    // A refresh restamps the semantics alongside the visible texts.
    let stopped = OverviewProjection {
        readout: Default::default(),
        state: OverviewState::Stopped,
        lifecycle: CoreLifecycle::Stopped,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        sampled_at: Duration::from_secs(9),
        failure: None,
        origin: OverviewOrigin::Demo,
        core_version: None,
        traffic_waveform: Default::default(),
        traffic_scale: Default::default(),
        traffic_topology: Default::default(),
        active_exit: Default::default(),
        public_ip: Default::default(),
        layout: Default::default(),
        reconnect_mask: Default::default(),
        viewport: Default::default(),
        subscription_quota: Default::default(),
        system_toggles: Default::default(),
        cpu_percent: None,
        total_traffic_bytes: None,
        proxy_mode: ProxyModeSnapshot {
            current: Some(ProxyMode::Rule),
            ..ProxyModeSnapshot::demo_fixture()
        },
        speedtest: Default::default(),
    };
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(stopped));
    app.update();

    let world = app.world_mut();
    let mut chips = world.query::<(&OverviewChip, &AccessibilityNode)>();
    let mut labels: Vec<(OverviewChipKind, String)> = Vec::new();
    for (chip, node) in chips.iter(world) {
        labels.push((chip.0, node.label().expect("chip group label").to_owned()));
    }
    assert!(labels.contains(&(OverviewChipKind::Connections, "连接数 0".to_owned())));
    assert!(labels.contains(&(OverviewChipKind::Memory, "内存 —".to_owned())));
    let mut lines = world.query::<(&OverviewLine, &AccessibilityNode)>();
    let (_, status) = lines
        .iter(world)
        .find(|(line, _)| line.0 == OverviewLineKind::State)
        .expect("the state line carries semantics");
    assert_eq!(status.label(), Some("已停止"), "the status word follows");
}
