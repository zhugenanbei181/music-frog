//! Headless controller-pump tests (BEVY-005): the live mihomo controller
//! pump against a local mockito fake controller, the frame-drain bridge
//! firing typed refresh events inside the real shell, and the mode-command
//! chain (demo flip + live PATCH + refused-command failure projection) —
//! all on `MinimalPlugins`, no window, no real core.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin, Assets};
use bevy::ecs::entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::world::World;
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use bevy::ui::widget::{ImageNode, Text};
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_application::proxy_mode_application::ProxyModeApplication;
use infiltrator_bevy_ui::app::{ModeAckSlot, ModeActionState, PendingModeAck, ShellPlugin};
use infiltrator_bevy_ui::controller::{ControllerConfig, MihomoOverviewSource, PumpDrainPlugin};
use infiltrator_bevy_ui::history::TrafficHistory;
use infiltrator_bevy_ui::pages::overview::{
    OverviewLine, OverviewLineKind, OverviewModePill, OverviewProjectionUpdated,
};
use infiltrator_bevy_ui::pages::overview_lifecycle::CoreControlButton;
use infiltrator_bevy_ui::projection::{
    DemoOverviewSource, OverviewOrigin, OverviewProjection, OverviewSource, OverviewState,
};
use infiltrator_bevy_ui::route::{OverviewSourceHandle, PagesPlugin};
use infiltrator_bevy_ui::shell_mode_issue::{ModeIssueRoot, ModeIssueText};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::chart::ChartPlate;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_mode::ProxyModeSnapshot;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::traffic_waveform::display_series;
use infiltrator_shared::locales::{Lang, Localizer};
use serde_json::json;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::channel;
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

/// A minimal `/configs` body; only `mode` is read by the pump.
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

/// The pump folds /version + /connections + /memory + /configs into live
/// projections: real version string, connection count, memory reading,
/// mode readback, live-origin marker — and rates from the growing totals
/// on the second sample.
#[test]
fn pump_projects_live_controller_fields() {
    let server = spawn_fake_controller();
    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);

    // Before the first sample: the honest "waiting" projection.
    let first = source.current();
    assert_eq!(first.origin, OverviewOrigin::LiveCore);
    assert_eq!(first.state, OverviewState::Unavailable);

    let projection = wait_until(|| {
        let current = source.current();
        (current.state == OverviewState::Running).then_some(current)
    });
    assert_eq!(projection.core_version.as_deref(), Some("v1.19.18"));
    assert_eq!(projection.active_connections, 3);
    assert_eq!(projection.memory_bytes, Some(40 * 1024 * 1024));
    assert_eq!(projection.proxy_mode.current, Some(ProxyMode::Rule));
    assert_eq!(projection.failure, None);

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
        (current.state == OverviewState::Unavailable
            && current.failure.as_deref().is_some_and(|f| !f.is_empty()))
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
        (current.state == OverviewState::Running).then_some(current)
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
    assert_eq!(projection.memory_bytes, Some(0));
    assert_eq!(projection.upload_bps, 0.0);
    assert_eq!(projection.download_bps, 0.0);
    assert_eq!(projection.proxy_mode.current, Some(ProxyMode::Rule));
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
    // the command-channel disconnect as the pump's stop signal (the other
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

// ---- the live mode command against the fake controller -----------------------

/// `set_mode` posts into the pump, the pump PATCHes `/configs` with the
/// wire-spelled body, the typed receipt comes back `Ok`, and the next
/// `/configs` readback flips the projection's mode.
#[test]
fn mode_command_patches_configs_and_reads_back() {
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
        .mock("GET", "/configs")
        .with_body(configs_body("global"))
        .create();
    let patch = server
        .mock("PATCH", "/configs")
        .match_body(mockito::Matcher::JsonString(
            json!({ "mode": "global" }).to_string(),
        ))
        .with_status(204)
        .create();

    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);

    let (ack_tx, ack_rx) = channel();
    source.set_mode(ProxyMode::Global, ack_tx);
    let receipt = ack_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the pump always answers the receipt");
    assert_eq!(receipt, Ok(ProxyMode::Global));
    patch.assert();

    let mode = wait_until(|| {
        let current = source.current();
        (current.proxy_mode.current == Some(ProxyMode::Global))
            .then_some(current.proxy_mode.current)
    });
    assert_eq!(mode, Some(ProxyMode::Global));
}

/// A controller that refuses the mode switch (HTTP 400) delivers the
/// failure through the typed receipt. The HTTP adapter rejects the non-2xx
/// response directly, so the receipt carries the honest controller refusal.
#[test]
fn refused_mode_patch_answers_err_receipt() {
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
        .mock("GET", "/configs")
        .with_body(configs_body("rule"))
        .create();
    server
        .mock("PATCH", "/configs")
        .match_body(mockito::Matcher::JsonString(
            json!({ "mode": "direct" }).to_string(),
        ))
        .with_status(400)
        .with_body(json!({ "message": "unknown mode" }).to_string())
        .create();

    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);

    let (ack_tx, ack_rx) = channel();
    source.set_mode(ProxyMode::Direct, ack_tx);
    let receipt = ack_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the pump always answers the receipt");
    let reason = receipt.expect_err("a refused switch must arrive as Err");
    assert!(
        reason.message.contains("400") || reason.message.contains("仍为 rule"),
        "the refusal names the controller rejection: {reason:?}"
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
            && snapshot.state == OverviewState::Running
            && snapshot.core_version.as_deref() == Some("v1.19.18")
        {
            assert_eq!(snapshot.active_connections, 3);
            assert_eq!(snapshot.memory_bytes, Some(40 * 1024 * 1024));
            assert_eq!(snapshot.proxy_mode.current, Some(ProxyMode::Rule));
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

// ---- the shell-side mode command chain ---------------------------------------
/// The headless demo composition (real shell + router) with capture of
/// refresh events, for the mode-pill affordance tests.
fn mounted_demo_app() -> (App, Captured) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    let captured: Captured = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&captured);
    app.add_observer(move |update: On<OverviewProjectionUpdated>| {
        *sink.lock().expect("capture lock") = Some(update.0.clone());
    });
    app.update();
    (app, captured)
}

fn mode_pill_entity(world: &mut World, mode: ProxyMode) -> entity::Entity {
    let mut pills = world.query::<(entity::Entity, &OverviewModePill)>();
    pills
        .iter(world)
        .find(|(_, pill)| pill.0 == mode)
        .expect("mode pill mounted")
        .0
}

fn pill_selected(world: &mut World, mode: ProxyMode) -> bool {
    let mut pills = world.query::<(&OverviewModePill, &ControlVisual)>();
    pills
        .iter(world)
        .find(|(pill, _)| pill.0 == mode)
        .expect("mode pill mounted")
        .1
        .0
}

/// The mode pill's full accept path: an `Activate` on the 全局模式 pill
/// posts `set_mode`, the demo fixture flips, the receipt drain re-reads
/// the source and fires the refresh event, the pills and banner chip
/// follow, and the in-flight latch clears (a duplicate click may follow).
#[test]
fn activating_a_mode_pill_flips_the_demo_fixture() {
    let (mut app, captured) = mounted_demo_app();

    let pill = mode_pill_entity(app.world_mut(), ProxyMode::Global);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();

    let world = app.world_mut();
    assert!(
        pill_selected(world, ProxyMode::Global),
        "the accepted command re-projects the flipped fixture"
    );
    assert!(!pill_selected(world, ProxyMode::Rule));
    assert!(
        world.resource::<ModeActionState>().0.pending.is_none(),
        "the receipt cleared the in-flight latch"
    );
    assert!(world.resource::<PendingModeAck>().0.is_none());
    let projection = captured
        .lock()
        .expect("capture lock")
        .clone()
        .expect("the drain fired a refresh event");
    assert_eq!(projection.proxy_mode.current, Some(ProxyMode::Global));
    assert_eq!(projection.origin, OverviewOrigin::Demo);
}

/// While a command is in flight, further pill activations are ignored —
/// no duplicate command is ever submitted, and the latch is untouched.
#[test]
fn in_flight_mode_commands_ignore_duplicate_activations() {
    let (mut app, _captured) = mounted_demo_app();

    let snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let actions = &mut app.world_mut().resource_mut::<ModeActionState>().0;
    actions.observe(
        snapshot.generation,
        snapshot.revision,
        ProxyModeApplication::from_surface(&snapshot),
    );
    let request = actions.begin(ProxyMode::Global).unwrap();
    let pill = mode_pill_entity(app.world_mut(), ProxyMode::Direct);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    app.update();

    let world = app.world_mut();
    assert_eq!(
        world.resource::<ModeActionState>().0.pending,
        Some(request),
        "request remains pending"
    );
    assert!(
        world.resource::<PendingModeAck>().0.is_none(),
        "no second command was submitted"
    );
    assert!(
        !pill_selected(world, ProxyMode::Direct),
        "the ignored activation changed nothing"
    );
}

/// A refused command (typed `Err` receipt, as the live pump answers on
/// HTTP 400) projects the failure: the banner flips to 不可用 with the
/// failure copy, and the latch clears for the next attempt.
#[test]
fn refused_mode_receipt_projects_the_failure() {
    let (mut app, _captured) = mounted_demo_app();

    let (ack_tx, ack_rx) = channel();
    ack_tx
        .send(Err(Failure::new(
            ErrorCode::InvalidInput,
            "400 Bad Request",
            false,
        )))
        .expect("injected receipt");
    let snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    let actions = &mut app.world_mut().resource_mut::<ModeActionState>().0;
    actions.observe(
        snapshot.generation,
        snapshot.revision,
        ProxyModeApplication::from_surface(&snapshot),
    );
    let request = actions.begin(ProxyMode::Global).unwrap();
    app.world_mut()
        .insert_resource(PendingModeAck(Some(ModeAckSlot {
            request,
            receiver: Mutex::new(ack_rx),
        })));
    app.update();

    let world = app.world_mut();
    let (_, state_text, _) = world
        .query::<(&OverviewLine, &Text)>()
        .iter(world)
        .find(|(line, _)| line.0 == OverviewLineKind::State)
        .map(|(line, text)| (line, text.0.clone(), ()))
        .expect("state line mounted");
    assert_eq!(
        state_text, "运行中",
        "a mode write failure cannot change observed lifecycle"
    );
    let (_, failure_text) = world
        .query::<(&ModeIssueText, &Text)>()
        .iter(world)
        .next()
        .map(|(line, text)| (line, text.0.clone()))
        .expect("failure line mounted");
    assert!(
        failure_text.contains("切换到全局失败"),
        "the refusal reason rides the failure line: {failure_text}"
    );
    assert!(
        world.resource::<ModeActionState>().0.pending.is_none(),
        "the failed receipt cleared the latch"
    );
    assert_eq!(
        world
            .query_filtered::<&Node, With<ModeIssueRoot>>()
            .single(world)
            .unwrap()
            .display,
        Display::Flex
    );
    assert_eq!(
        world
            .resource::<ModeActionState>()
            .0
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidInput
    );
}

// ---- the honest banner (origin note + stop slot) ------------------------------

/// A live-origin projection names the core's real version in the banner
/// and replaces the stop button with the honest lifecycle caption; the
/// demo projection keeps the 演示数据 note and the stop button.
#[test]
fn banner_note_and_stop_slot_follow_the_projection_origin() {
    struct LiveStub;
    impl OverviewSource for LiveStub {
        fn current(&self) -> OverviewProjection {
            OverviewProjection {
                state: OverviewState::Running,
                lifecycle: CoreLifecycle::Running,
                upload_bps: 0.0,
                download_bps: 0.0,
                readout: Default::default(),
                active_connections: 0,
                memory_bytes: None,
                sampled_at: Duration::from_secs(1),
                failure: None,
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
                    current: Some(ProxyMode::Rule),
                    ..ProxyModeSnapshot::demo_fixture()
                },
                speedtest: Default::default(),
            }
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

/// The page state word.
fn state_line_text(world: &mut World) -> String {
    world
        .query::<(&OverviewLine, &Text)>()
        .iter(world)
        .find(|(line, _)| line.0 == OverviewLineKind::State)
        .map(|(_, text)| text.0.clone())
        .expect("state line mounted")
}

/// A refused mode write retains its failure until explicit dismissal.
/// Successful polling preserves the actual running lifecycle throughout.
#[test]
fn refused_mode_failure_survives_healthy_reads_without_corrupting_lifecycle_until_dismissed() {
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
        .mock("PATCH", "/configs")
        .match_body(mockito::Matcher::JsonString(
            json!({ "mode": "direct" }).to_string(),
        ))
        .with_status(400)
        .with_body(json!({ "message": "unknown mode" }).to_string())
        .create();

    let mut config = ControllerConfig::new(server.url(), None);
    config.sample_interval = TEST_INTERVAL;
    let source = MihomoOverviewSource::spawn(config);
    let (mut app, _captured) = mounted_live_app(&source);

    // The pump reaches Running and the page shows it.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "the pump never reached Running");
        app.update();
        if state_line_text(app.world_mut()) == "运行中" {
            break;
        }
    }

    // Refuse a Direct switch through the real pill affordance.
    let pill = mode_pill_entity(app.world_mut(), ProxyMode::Direct);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: pill });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "the refusal never surfaced");
        app.update();
        if app
            .world()
            .resource::<ModeActionState>()
            .0
            .failure
            .is_some()
        {
            break;
        }
    }
    assert!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .pending
            .is_none(),
        "the receipt cleared the command latch"
    );

    // Successful polling cannot dismiss an independent mode-write failure.
    let deadline = Instant::now() + Duration::from_millis(600);
    while Instant::now() < deadline {
        app.update();
        assert_eq!(
            source.current().state,
            OverviewState::Running,
            "the pump continues to report the actual running lifecycle"
        );
        assert_eq!(
            state_line_text(app.world_mut()),
            "运行中",
            "a mode failure cannot alter the observed lifecycle"
        );
    }

    assert!(
        app.world()
            .resource::<ModeActionState>()
            .0
            .failure
            .is_some(),
        "healthy reads do not dismiss a rejected write"
    );
    app.world_mut()
        .resource_mut::<ModeActionState>()
        .0
        .dismiss_failure();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "the page never recovered");
        app.update();
        if state_line_text(app.world_mut()) == "运行中" {
            break;
        }
    }
    assert_eq!(
        source.current().state,
        OverviewState::Running,
        "the pump was Running throughout"
    );
}
