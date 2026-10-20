//! Headless controller-pump tests (BEVY-005): the live mihomo controller
//! pump against a local mockito fake controller, the frame-drain bridge
//! firing typed refresh events inside the real shell and the live read
//! projection chain —
//! all on `MinimalPlugins`, no window, no real core.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin, Assets};
use bevy::ecs::entity;
use bevy::ecs::observer::On;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::{ImageNode, Text};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::controller::{ControllerConfig, MihomoOverviewSource, PumpDrainPlugin};
use infiltrator_bevy_ui::history::TrafficHistory;
use infiltrator_bevy_ui::pages::overview::{
    OverviewLine, OverviewLineKind, OverviewProjectionUpdated,
};
use infiltrator_bevy_ui::pages::overview_lifecycle::CoreControlButton;
use infiltrator_bevy_ui::projection::{
    DemoOverviewSource, OverviewOrigin, OverviewProjection, OverviewSource, SourceKind,
};
use infiltrator_bevy_ui::route::{OverviewSourceHandle, PagesPlugin};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_contract::resources::ResourceAvailability;
use infiltrator_domain::traffic_waveform::display_series;
use infiltrator_presentation::overview_page_model::OverviewRunState;
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentSnapshot};
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};

/// A short pump interval so tests sample in milliseconds, not seconds.
const TEST_INTERVAL: Duration = Duration::from_millis(60);

/// The `/connections` snapshot the fake controller serves: three active
/// connections with totals that grow by a fixed step per request (the rate
/// window then has a positive delta on the second sample).
fn connections_body(call: u64) -> String {
    let entry = |id: &str| json!({ "id": id, "metadata": {}, "rule": "MATCH" });
    json!({
        "uploadTotal": 1_000_000 + call * 700_000,
        "downloadTotal": 2_000_000 + call * 1_400_000,
        "connections": [entry("a"), entry("b"), entry("c")],
    })
    .to_string()
}

/// The fake controller: /version, /connections (growing totals),
/// /memory and /configs. Returns the server URL.
fn spawn_fake_controller() -> mockito::ServerGuard {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/version")
        .with_body(json!({ "version": "v1.19.18", "premium": false }).to_string())
        .create();
    let calls = Arc::new(AtomicU64::new(0));
    let writer_calls = Arc::clone(&calls);
    server
        .mock("GET", "/connections")
        .with_chunked_body(move |writer: &mut dyn Write| {
            let call = writer_calls.fetch_add(1, Ordering::SeqCst);
            writer.write_all(connections_body(call).as_bytes())
        })
        .create();
    server
        .mock("GET", "/memory")
        .with_body(json!({ "inuse": 40 * 1024 * 1024, "oslimit": 0 }).to_string())
        .create();
    server
        .mock("GET", "/configs")
        .with_body(configs_body("rule"))
        .create();
    server
}

/// A minimal `/configs` body; the pump derives routing intent from `mode`.
fn configs_body(mode: &str) -> String {
    json!({
        "port": 0, "socks-port": 0, "redir-port": 0, "tproxy-port": 0,
        "mixed-port": 7899, "mode": mode, "log-level": "silent",
        "allow-lan": false,
    })
    .to_string()
}

/// Poll `read` until it returns `Some`, with a hard deadline (no flaky
/// infinite loops; 5s is generous for a localhost fake controller).
fn wait_until<T>(read: impl Fn() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "the pump never produced the expected projection"
        );
        if let Some(value) = read() {
            return value;
        }
        sleep(Duration::from_millis(10));
    }
}

// ---- the live pump against the fake controller -------------------------------

/// The telemetry-only pump folds /version + /connections + /memory into live
/// projections. RoutingIntent remains unobserved because it is read through
/// the typed surface port; rates come from growing totals on the second sample.
#[test]
fn pump_projects_live_controller_fields() {
    let server = spawn_fake_controller();
    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);

    // Before the first sample: the honest "waiting" projection.
    let first = source.current();
    assert_eq!(first.origin, OverviewOrigin::LiveCore);
    assert_eq!(
        first.run_state,
        OverviewRunState::Unavailable {
            reason: "waiting for the first controller sample".to_owned()
        },
        "before the first sample the core is not available, and says why"
    );
    assert_eq!(
        first.failure(),
        Some("waiting for the first controller sample"),
        "the failure line carries the core's own reason"
    );

    let projection = wait_until(|| {
        let current = source.current();
        (current.run_state == OverviewRunState::Running).then_some(current)
    });
    assert_eq!(projection.core_version.as_deref(), Some("v1.19.18"));
    assert_eq!(projection.active_connections, 3);
    assert_eq!(projection.memory_bytes, ResourceAvailability::NotObserved);
    assert_eq!(projection.routing_intent, RoutingIntentSnapshot::default());
    assert_eq!(projection.failure(), None);

    // The totals grow per request, so a later sample must carry a
    // positive uplink rate (the rate window has a delta).
    let rated = wait_until(|| {
        let current = source.current();
        (current.upload_bps > 0.0 && current.download_bps > 0.0).then_some(current)
    });
    assert_eq!(rated.active_connections, 3);
}

/// A configured controller that cannot be reached projects the typed
/// unavailable state with a human-readable failure — never a fabricated
/// zero, never a crash.
#[test]
fn unreachable_controller_projects_unavailable() {
    // Port 1 on loopback: nothing listens there (refused, not filtered).
    let mut config = ControllerConfig::new("http://127.0.0.1:1", None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);

    let projection = wait_until(|| {
        let current = source.current();
        (matches!(current.run_state, OverviewRunState::Unavailable { .. })
            && current.failure().is_some_and(|reason| !reason.is_empty()))
        .then_some(current)
    });
    assert_eq!(projection.origin, OverviewOrigin::LiveCore);
    assert_eq!(projection.active_connections, 0);
}

/// The idle-core shape the pinned v1.19.18 really serves, byte-for-byte:
/// `/version` without a `premium` key (the seam's `Version`, types.rs:20-23,
/// rejects it) and `/connections` with `"connections": null` (types.rs:160-166
/// rejects it) — both mihomo-api decodes fail, so the pump's null-tolerant
/// fallbacks must take over and still project a Running core with the real
/// version and honest zeros.
#[test]
fn idle_null_tracker_falls_back_and_stays_running() {
    let mut server = mockito::Server::new();
    // Verbatim real payload (no `premium`).
    server
        .mock("GET", "/version")
        .with_body(r#"{"meta":true,"version":"v1.19.18"}"#)
        .create();
    server
        .mock("GET", "/connections")
        .with_body(
            json!({
                "downloadTotal": 0,
                "uploadTotal": 0,
                "connections": null,
                "memory": 40_529_920,
            })
            .to_string(),
        )
        .create();
    server
        .mock("GET", "/memory")
        .with_body(json!({ "inuse": 0, "oslimit": 0 }).to_string())
        .create();
    server
        .mock("GET", "/configs")
        .with_body(configs_body("rule"))
        .create();

    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);

    let projection = wait_until(|| {
        let current = source.current();
        (current.run_state == OverviewRunState::Running).then_some(current)
    });
    assert_eq!(
        projection.core_version.as_deref(),
        Some("v1.19.18"),
        "the fallback read lands the real core version"
    );
    assert_eq!(
        projection.active_connections, 0,
        "idle tracker: honest zero"
    );
    assert_eq!(projection.memory_bytes, ResourceAvailability::NotObserved);
    assert_eq!(projection.upload_bps, 0.0);
    assert_eq!(projection.download_bps, 0.0);
    assert_eq!(projection.routing_intent, RoutingIntentSnapshot::default());
}

/// Dropping the last source clone stops the pump thread (channel-driven
/// cancellation): the sampler against the (dead) endpoint stops ticking,
/// observable through the fake controller's request counter.
#[test]
fn dropping_the_source_stops_the_pump() {
    let mut server = mockito::Server::new();
    server
        .mock("GET", "/version")
        .with_body(json!({ "version": "v1.19.18", "premium": false }).to_string())
        .create();
    let requests = Arc::new(AtomicU64::new(0));
    let counted = Arc::clone(&requests);
    server
        .mock("GET", "/connections")
        .with_chunked_body(move |writer: &mut dyn Write| {
            let call = counted.fetch_add(1, Ordering::SeqCst);
            writer.write_all(connections_body(call).as_bytes())
        })
        .create();

    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);
    // Keep the snapshot receiving end alive across the drop: this isolates
    // the owner-lifetime cancellation token as the pump's stop signal (the other
    // exit path — snapshot side gone — is trivially exercised everywhere
    // else).
    let _bridge = source.bridge();

    // The pump is sampling (two windows at least).
    wait_until(|| (requests.load(Ordering::SeqCst) >= 2).then_some(()));

    // Drop the source: the command channel disconnects and the loop exits.
    drop(source);
    // Grace window for an in-flight request, then verify the counter is
    // stable across several sampling intervals.
    sleep(Duration::from_millis(150));
    let settled = requests.load(Ordering::SeqCst);
    sleep(TEST_INTERVAL * 6);
    assert_eq!(
        requests.load(Ordering::SeqCst),
        settled,
        "the pump kept sampling after its source was dropped"
    );
}

// ---- the bridge: pump -> bevy events ----------------------------------------

/// The captured payload of an `OverviewProjectionUpdated` trigger.
type Captured = Arc<Mutex<Option<OverviewProjection>>>;

/// The headless composition under test: real shell + router + pump drain
/// over `MinimalPlugins`, with an observer capturing every projection
/// refresh event.
fn mounted_live_app(source: &MihomoOverviewSource) -> (App, Captured) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(source.clone()));
    app.add_plugins(PumpDrainPlugin::new(source));
    let captured: Captured = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&captured);
    app.add_observer(move |update: On<OverviewProjectionUpdated>| {
        *sink.lock().expect("capture lock") = Some(update.0.clone());
    });
    app.update();
    (app, captured)
}

/// The full live chain inside bevy: the pump publishes snapshots, the
/// frame drain coalesces them into `OverviewProjectionUpdated` triggers,
/// and the page-visible payload carries the real controller data.
#[test]
fn pump_snapshots_flow_through_the_bridge_into_refresh_events() {
    let server = spawn_fake_controller();
    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);
    let (mut app, captured) = mounted_live_app(&source);

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            Instant::now() < deadline,
            "the bridge never delivered a running live projection"
        );
        app.update();
        let snapshot = captured.lock().expect("capture lock").clone();
        if let Some(snapshot) = snapshot
            && snapshot.run_state == OverviewRunState::Running
            && snapshot.core_version.as_deref() == Some("v1.19.18")
        {
            assert_eq!(snapshot.active_connections, 3);
            assert_eq!(snapshot.memory_bytes, ResourceAvailability::NotObserved);
            assert_eq!(snapshot.routing_intent, RoutingIntentSnapshot::default());
            break;
        }
    }
}

/// Successful controller deltas reach the application waveform, the native
/// chart plate and its actual CPU raster without replacing the entity or handle.
#[test]
fn pump_drain_records_actual_shared_samples_into_the_chart() {
    let server = spawn_fake_controller();
    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);
    let (mut app, _captured) = mounted_live_app(&source);
    app.init_asset::<Image>();

    let deadline = Instant::now() + Duration::from_secs(5);
    let (plate_id, handle) = loop {
        assert!(
            Instant::now() < deadline,
            "the drain never recorded a sample in the ring"
        );
        app.update();
        let world = app.world_mut();
        if world.resource::<TrafficHistory>().len() >= 10 {
            let mut plates = world.query::<(entity::Entity, &ChartPlate, &ImageNode)>();
            let (id, _, node) = plates
                .single(world)
                .expect("the traffic chart plate is mounted and rasterized");
            break (id, node.image.clone());
        }
    };

    let world = app.world();
    // Actual successful observations, rather than failed reads padded with zeros.
    let history = world.resource::<TrafficHistory>();
    assert!(
        history.upload_series().iter().any(|rate| *rate > 0.0)
            && history.download_series().iter().any(|rate| *rate > 0.0),
        "the actual controller deltas are recorded"
    );
    let plate = world.get::<ChartPlate>(plate_id).expect("plate survives");
    let expected = display_series(&world.resource::<LatestSurfaceSnapshot>().0.traffic_waveform);
    assert_eq!(
        (plate.0.up.clone(), plate.0.down.clone()),
        expected,
        "the plate spells the application-owned samples"
    );
    let image = world
        .resource::<Assets<Image>>()
        .get(&handle)
        .expect("chart asset under the original handle")
        .clone();
    // The rewrite happened: actual samples draw beyond the mount-time grid-only raster. (The
    // CPU-side data is always present in a headless composition — only a
    // render-backed host may strip it after GPU upload.)
    let data = image.data.expect("cpu-side pixel data headless");
    let painted = data
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|px| px[3] > 0)
        .count();
    assert!(
        painted as u32 > image.texture_descriptor.size.width * 2,
        "the observed samples were rasterized: painted={painted}, width={}, height={}, points={}",
        image.texture_descriptor.size.width,
        image.texture_descriptor.size.height,
        plate.0.up.len(),
    );
}

#[test]
fn banner_note_and_stop_slot_follow_the_projection_origin() {
    struct LiveStub;
    impl OverviewSource for LiveStub {
        fn current(&self) -> OverviewProjection {
            OverviewProjection {
                failure: None,
                run_state: OverviewRunState::Running,
                lifecycle: CoreLifecycle::Running,
                upload_bps: 0.0,
                download_bps: 0.0,
                readout: Default::default(),
                active_connections: 0,
                memory_bytes: ResourceAvailability::NotObserved,
                sampled_at: Duration::from_secs(1),
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
                cpu_percent: ResourceAvailability::NotObserved,
                total_traffic_bytes: None,
                routing_intent: RoutingIntentSnapshot {
                    current: Some(RoutingIntent::FollowRules),
                    ..RoutingIntentSnapshot::demo_fixture()
                },
                speedtest: Default::default(),
            }
        }

        fn kind(&self) -> SourceKind {
            SourceKind::LiveCore
        }
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(LiveStub));
    app.update();

    let world = app.world_mut();
    let note = banner_text(world, OverviewLineKind::BannerNote);
    assert_eq!(
        note, "实时内核 · v1.19.18",
        "the banner names the real core version"
    );
    let mut stops = world.query::<&CoreControlButton>();
    assert_eq!(
        stops.iter(world).count(),
        1,
        "the live source exposes the lifecycle control surface"
    );
    assert!(
        world
            .query::<(&CoreControlButton, &ButtonDisabled)>()
            .iter(world)
            .all(|(_, disabled)| disabled.0),
        "an uncomposed legacy source cannot execute lifecycle actions"
    );
    assert!(
        world
            .query::<&Text>()
            .iter(world)
            .any(|text| text.0 == Lang("zh-CN").tr("core_control_unavailable_hint")),
        "the honest lifecycle caption is visible"
    );
    assert!(
        world.resource::<OverviewSourceHandle>().0.current().origin == OverviewOrigin::LiveCore,
    );

    // Demo regression: same page, demo note, stop button present.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.update();
    let world = app.world_mut();
    assert_eq!(
        banner_text(world, OverviewLineKind::BannerNote),
        "演示数据 · 未接实时内核"
    );
    let mut stops = world.query::<&CoreControlButton>();
    assert_eq!(
        stops.iter(world).count(),
        1,
        "demo exposes the same lifecycle control surface"
    );
}

fn banner_text(world: &mut World, kind: OverviewLineKind) -> String {
    world
        .query::<(&OverviewLine, &Text)>()
        .iter(world)
        .find(|(line, _)| line.0 == kind)
        .map(|(_, text)| text.0.clone())
        .unwrap_or_else(|| panic!("no {kind:?} line mounted"))
}

#[path = "controller_configuration_tests.rs"]
mod configuration_tests;
