//! Behavior cases for projection.
//! test-intent: behavior

use super::*;
use bevy::scene::{CommandsSceneExt, bsn};
use infiltrator_application::shell_readout_application::ShellReadoutApplication;
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_contract::active_exit::ActiveExitStatus;
use infiltrator_contract::proxy_mode::ProxyModeSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::{OverviewPageSnapshot, PageData};
use infiltrator_contract::traffic_waveform::TrafficSample;
use infiltrator_domain::traffic_waveform::display_series;

/// A projection event restamps texts, inks, pill selection, the banner's
/// stored state and the chip values in place — every restampable entity
/// keeps its id.
#[test]
fn projection_updates_restamp_in_place() {
    let palette = UiPalette::new(&Theme::dark());
    let mut app = mounted_app_with(DemoOverviewSource::running());
    let ids_before = overview_entity_ids(app.world_mut());

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
            current: Some(ProxyMode::Global),
            ..ProxyModeSnapshot::demo_fixture()
        },
        speedtest: Default::default(),
    };
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let observations = ShellReadoutApplication::default();
    observations.project(&snapshot);
    snapshot.revision += 1;
    snapshot.core.lifecycle = CoreLifecycle::Stopped;
    snapshot.core.proxy_mode = stopped.proxy_mode.current;
    snapshot.core.upload_bps = stopped.upload_bps;
    snapshot.core.download_bps = stopped.download_bps;
    snapshot.core.active_connections = stopped.active_connections;
    snapshot.core.memory_bytes = stopped.memory_bytes;
    snapshot.runtime_control.mode = stopped.proxy_mode.current;
    snapshot.pages.overview = PageData::ready(OverviewPageSnapshot {
        proxy_mode: stopped.proxy_mode.current,
        upload_bps: stopped.upload_bps,
        download_bps: stopped.download_bps,
        active_connections: stopped.active_connections,
        memory_bytes: stopped.memory_bytes,
        core_version: stopped.core_version,
    });
    snapshot.shell_readout = observations.project(&snapshot);
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();

    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "refresh never remounts: ids unchanged"
    );
    let (_, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "已停止");
    assert_eq!(ink.0, palette.ink_dim, "stopped ink restamped");
    let (_, upload, _) = line(world, OverviewLineKind::Upload);
    assert_eq!(upload, "↑ 1.40 MB/s（已失效）");
    let (_, mode_chip, _) = line(world, OverviewLineKind::ModeChip);
    assert_eq!(
        mode_chip, "全局（已失效）",
        "the banner chip renames the mode"
    );
    assert!(
        pill_selected(world, ProxyMode::Global),
        "Global pill selected"
    );
    assert!(
        !pill_selected(world, ProxyMode::Rule),
        "Rule pill deselected"
    );
    let (_, fill, stored) = card(world);
    assert_eq!(stored, CoreLifecycle::Stopped);
    assert_eq!(
        fill, palette.accent_container,
        "stopped keeps the accent container banner"
    );
    let (_, memory) = chip_value(world, OverviewChipKind::Memory);
    assert_eq!(memory, "—");

    let unavailable = OverviewProjection {
        readout: Default::default(),
        state: OverviewState::Unavailable,
        lifecycle: CoreLifecycle::Failed,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        sampled_at: Duration::from_secs(10),
        failure: Some("refused".to_owned()),
        origin: OverviewOrigin::LiveCore,
        core_version: Some("v1.19.18".to_owned()),
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
            current: Some(ProxyMode::Global),
            ..ProxyModeSnapshot::demo_fixture()
        },
        speedtest: Default::default(),
    };
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(unavailable));
    app.update();

    let world = app.world_mut();
    assert_eq!(
        overview_entity_ids(world),
        ids_before,
        "still zero remounts"
    );
    let (_, failure_text, _) = line(world, OverviewLineKind::Failure);
    assert_eq!(failure_text, "refused");
    let (_, fill, _) = card(world);
    assert_eq!(
        fill, palette.danger,
        "unavailable flips the banner to danger"
    );
    let (_, state_text, ink) = line(world, OverviewLineKind::State);
    assert_eq!(state_text, "运行出错");
    assert_eq!(
        ink.0, palette.on_accent,
        "state ink readable on the danger banner"
    );
}

/// A projection update re-derives the chart series (demo waves → the live
/// ring) and restamps the plate in place — same entity, same handle
/// contract with sync_charts.
#[test]
fn projection_updates_refresh_the_chart_series() {
    let mut app = mounted_default();
    let (plate_id, before) = chart_plate(app.world_mut());
    let (demo_up, _) = demo_traffic_series();
    assert_eq!(before.0.up, demo_up, "precondition: the demo trend mounted");

    // Conflicting legacy history cannot replace the shared waveform.
    let mut history = TrafficHistory::default();
    history.push(100.0, 200.0);
    history.push(300.0, 400.0);
    let mut projection = live_projection(5.0, 6.0);
    projection.traffic_waveform.samples = vec![
        TrafficSample {
            sampled_at_epoch_ms: Some(1),
            upload_bps: 1.0,
            download_bps: 2.0,
        },
        TrafficSample {
            sampled_at_epoch_ms: Some(2),
            upload_bps: 3.0,
            download_bps: 4.0,
        },
    ];
    let expected = display_series(&projection.traffic_waveform);
    {
        let world = app.world_mut();
        world.insert_resource(history);
        world
            .commands()
            .trigger(OverviewProjectionUpdated(projection));
    }
    app.update();

    let world = app.world_mut();
    let (plate_id_after, after) = chart_plate(world);
    assert_eq!(plate_id_after, plate_id, "the chart never remounts");
    assert_eq!(
        (after.0.up, after.0.down),
        expected,
        "the plate replays the shared samples"
    );
}

#[test]
fn projection_replay_preserves_unrelated_text_and_conflicting_native_roles() {
    let mut app = mounted_default();
    let unrelated = app
        .world_mut()
        .commands()
        .spawn_scene(bsn! {
            Text({ "outside {opaque}".to_owned() }) TextRole(Role::Body)
        })
        .id();
    let excluded = app
        .world_mut()
        .commands()
        .spawn_scene(bsn! {
            Text({ "conflicting roles".to_owned() }) TextRole(Role::Body)
            ActiveExitText(ActiveExitTextKind::Name)
            PublicIpText(PublicIpTextKind::Status)
        })
        .id();
    let topology = app
        .world_mut()
        .commands()
        .spawn_scene(bsn! {
            Text({ "old topology".to_owned() }) TextRole(Role::Body)
            TopologyText::default()
            ActiveExitText(ActiveExitTextKind::Name)
        })
        .id();
    let exit = app
        .world_mut()
        .commands()
        .spawn_scene(bsn! {
            Text({ "old node".to_owned() }) TextRole(Role::Body)
            ActiveExitText(ActiveExitTextKind::Name)
        })
        .id();
    app.update();
    let original_ink = app.world().get::<TextColor>(topology).unwrap().0;
    let mut projection = live_projection(5.0, 6.0);
    projection.active_exit.status = ActiveExitStatus::Ready;
    projection.active_exit.name = Some("node {name}/中文🙂".to_owned());
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();

    assert_eq!(
        app.world().get::<Text>(unrelated).unwrap().0,
        "outside {opaque}"
    );
    assert_eq!(
        app.world().get::<Text>(excluded).unwrap().0,
        "conflicting roles"
    );
    assert_eq!(app.world().get::<Text>(topology).unwrap().0, "拓扑未观测");
    assert_eq!(
        app.world().get::<TextColor>(topology).unwrap().0,
        original_ink
    );
    assert_eq!(
        app.world().get::<Text>(exit).unwrap().0,
        "node {name}/中文🙂"
    );

    // The role still updates in place once the conflicting marker is removed.
    app.world_mut()
        .entity_mut(excluded)
        .remove::<PublicIpText>();
    app.world_mut()
        .commands()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    assert_eq!(
        app.world().get::<Text>(excluded).unwrap().0,
        "node {name}/中文🙂"
    );
}
