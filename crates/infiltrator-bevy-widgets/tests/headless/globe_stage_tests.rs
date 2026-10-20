//! Headless tests for the globe's own seams (BANDROID-016.2 / BANDROID-016.3):
//! the degradation policy that caps the requested mode by the device
//! pressure, the pointer seam that resolves a node by its real identity, the
//! typed stage that sleeps and releases the raster, and the frame/memory
//! report bound to the source identity.

use super::render_cache_tests::app;
use bevy::app::App;
use bevy::asset::Assets;
use bevy::ecs::entity::Entity;
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::scene::CommandsSceneExt;
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::globe::degradation::{
    GlobeBattery, GlobeDevicePressure, GlobeMotion, GlobeThermal,
};
use infiltrator_bevy_widgets::globe::interaction::{
    GlobePointerTarget, drag_globe, globe_pointer_target,
};
use infiltrator_bevy_widgets::globe::report::{
    GlobeBaseline, GlobeBudgetVerdict, GlobeFrameReport, GlobeFrameWindow, GlobeMeasurement,
    GlobeSourceIdentity,
};
use infiltrator_bevy_widgets::globe::stage::{GlobeActivity, GlobeIdleBudget, GlobeStage};
use infiltrator_bevy_widgets::globe::{
    GeoLocation, GeoPoint, GeoPrecision, GeoSource, GeoUnknown, GlobeLink, GlobeMode, GlobeNode,
    GlobeNodeCategory, GlobePlate, GlobeProjection, GlobeSpec, globe_scene,
};
use std::thread::sleep;
use std::time;

fn known(lat_deg: f64, lon_deg: f64) -> GeoLocation {
    GeoLocation::Known(
        GeoPoint::new(
            lat_deg,
            lon_deg,
            GeoSource::GeoIpDatabase,
            GeoPrecision::City,
        )
        .expect("in-range test coordinate"),
    )
}

fn sample_spec() -> GlobeSpec {
    let nodes = vec![
        GlobeNode::new("home", "Home", known(39.9, 116.4), GlobeNodeCategory::Home),
        GlobeNode::new("exit", "Exit", known(37.7, -122.4), GlobeNodeCategory::Exit),
    ];
    let links = vec![GlobeLink::new("home", "exit", 0.0)];
    GlobeSpec::new(nodes, links, 160, 160).with_flow(0.0, 0.0)
}

fn stage_of(app: &App, entity: Entity) -> GlobeStage {
    *app.world()
        .get::<GlobeStage>(entity)
        .expect("the plate carries its own stage")
}

fn set_stage(app: &mut App, entity: Entity, stage: GlobeStage) {
    let mut current = app
        .world_mut()
        .get_mut::<GlobeStage>(entity)
        .expect("the plate carries its own stage");
    *current = stage;
}

/// The device pressure caps the requested mode: a low battery drops the
/// enhancement to the minimal Eco view, a throttling device and a
/// reduced-motion preference drop the sphere for the static 2D fallback.
#[test]
fn globe_degradation_caps_the_requested_mode_by_the_device_pressure() {
    let healthy = GlobeDevicePressure::default();
    assert_eq!(healthy.allowed_mode(), GlobeMode::Globe);
    assert_eq!(healthy.effective_mode(GlobeMode::Globe), GlobeMode::Globe);

    let low_battery = GlobeDevicePressure {
        battery: GlobeBattery::Low,
        ..GlobeDevicePressure::default()
    };
    assert_eq!(low_battery.allowed_mode(), GlobeMode::Eco);
    assert_eq!(
        low_battery.effective_mode(GlobeMode::Globe),
        GlobeMode::Eco,
        "a low battery drops the sphere"
    );

    let throttling = GlobeDevicePressure {
        thermal: GlobeThermal::Throttling,
        ..GlobeDevicePressure::default()
    };
    assert_eq!(throttling.allowed_mode(), GlobeMode::Flat2d);
    assert_eq!(
        throttling.effective_mode(GlobeMode::Globe),
        GlobeMode::Flat2d,
        "a throttling device drops the sphere"
    );

    let reduced = GlobeDevicePressure {
        motion: GlobeMotion::Reduced,
        ..GlobeDevicePressure::default()
    };
    assert_eq!(reduced.allowed_mode(), GlobeMode::Flat2d);
    assert_eq!(
        reduced.effective_mode(GlobeMode::Globe),
        GlobeMode::Flat2d,
        "reduced motion drops the animation"
    );

    assert_eq!(
        throttling.effective_mode(GlobeMode::Eco),
        GlobeMode::Eco,
        "the policy never raises a requested mode"
    );
    assert_eq!(
        low_battery.effective_mode(GlobeMode::Eco),
        GlobeMode::Eco,
        "an explicit Eco request always wins"
    );
}

/// The pointer resolves to the node's real identity — never to an index, and
/// never to a node without a real geographic observation.
#[test]
fn globe_pointer_resolves_to_the_node_real_identity() {
    let spec = GlobeSpec::new(
        vec![
            GlobeNode::new("front", "Front", known(0.0, 0.0), GlobeNodeCategory::Home),
            GlobeNode::new("back", "Back", known(0.0, 180.0), GlobeNodeCategory::Exit),
            GlobeNode::new(
                "unknown",
                "Unknown",
                GeoLocation::Unknown(GeoUnknown::NotObserved),
                GlobeNodeCategory::Relay,
            ),
        ],
        Vec::new(),
        200,
        200,
    );
    let projection = GlobeProjection::from_spec(&spec);
    let front = projection
        .project_lat_lon(0.0, 0.0)
        .expect("the front node is on the visible hemisphere");
    assert!(
        projection.project_lat_lon(0.0, 180.0).is_none(),
        "the back hemisphere is culled"
    );

    let target = globe_pointer_target(&spec, Vec2::new(front.x, front.y));
    assert_eq!(target, GlobePointerTarget::NodeId("front".to_owned()));
    assert_eq!(target.node_id(), Some("front"));

    let empty = globe_pointer_target(&spec, Vec2::new(4.0, 4.0));
    assert_eq!(
        empty,
        GlobePointerTarget::Empty,
        "an unknown node and the culled hemisphere resolve to nothing"
    );
    assert_eq!(empty.node_id(), None);

    // The 2D fallback is lossless: the antipode is hittable there.
    let flat = spec.clone().with_mode(GlobeMode::Flat2d);
    let antipode = globe_pointer_target(&flat, Vec2::new(200.0, 100.0));
    assert_eq!(antipode, GlobePointerTarget::NodeId("back".to_owned()));

    // The hover is stored by the node's real identity.
    let mut hovered = spec.clone();
    hovered.hover(&target);
    assert_eq!(hovered.hovered_node.as_deref(), Some("front"));
    hovered.hover(&GlobePointerTarget::Empty);
    assert_eq!(hovered.hovered_node, None);
}

/// A drag rotates the view and clamps the pitch.
#[test]
fn globe_drag_rotates_the_view_within_the_clamp() {
    let mut spec = sample_spec();
    let view = drag_globe(&mut spec, Vec2::new(120.0, 10_000.0));
    assert!(
        (-89.0..=89.0).contains(&view.pitch_deg),
        "the pitch stays clamped"
    );
    assert!(
        (-180.0..=180.0).contains(&view.yaw_deg),
        "the yaw stays wrapped"
    );
    assert_eq!(view, spec.view(), "the reported view is the spec's view");
}

/// An untouched enhancement sleeps and releases its raster; a pointer
/// gesture wakes it and the raster comes back.
#[test]
fn globe_sleeps_when_idle_and_releases_its_raster() {
    let mut app = app();
    app.update();
    let baseline = app.world().resource::<Assets<Image>>().len();

    let entity = {
        let mut commands = app.world_mut().commands();
        commands.spawn_scene(globe_scene(sample_spec())).id()
    };
    app.world_mut().flush();
    app.update();
    assert_eq!(app.world().resource::<Assets<Image>>().len(), baseline + 1);
    assert!(stage_of(&app, entity).is_live());

    // The idle budget is spent at once: the enhancement falls asleep.
    app.world_mut().resource_mut::<GlobeIdleBudget>().0 = 0.0;
    sleep(time::Duration::from_millis(5));
    app.update();
    app.update();
    app.update();

    assert_eq!(
        stage_of(&app, entity),
        GlobeStage::Sleeping,
        "an untouched enhancement sleeps"
    );
    assert_eq!(
        app.world().resource::<Assets<Image>>().len(),
        baseline,
        "a sleeping enhancement owns no raster"
    );
    assert!(app.world().get::<ImageNode>(entity).is_none());

    // A pointer gesture wakes it, and the raster comes back. The idle budget
    // is restored first: a budget of zero would put the woken globe straight
    // back to sleep, which is a different fact from "a gesture wakes it".
    app.world_mut().resource_mut::<GlobeIdleBudget>().0 = 20.0;
    let woken = stage_of(&app, entity).interact();
    set_stage(&mut app, entity, woken);
    app.update();
    app.update();
    assert!(stage_of(&app, entity).is_live());
    assert_eq!(
        app.world().resource::<Assets<Image>>().len(),
        baseline + 1,
        "a woken enhancement owns its raster again"
    );
    assert!(app.world().get::<ImageNode>(entity).is_some());
}

/// A closed enhancement releases its resources while its plate stays mounted.
#[test]
fn globe_closed_enhancement_releases_its_raster_and_keeps_the_plate() {
    let mut app = app();
    app.update();
    let baseline = app.world().resource::<Assets<Image>>().len();

    let entity = {
        let mut commands = app.world_mut().commands();
        commands.spawn_scene(globe_scene(sample_spec())).id()
    };
    app.world_mut().flush();
    app.update();
    assert_eq!(app.world().resource::<Assets<Image>>().len(), baseline + 1);

    set_stage(&mut app, entity, GlobeStage::Closed);
    app.update();
    app.update();
    app.update();

    assert!(stage_of(&app, entity).is_closed());
    assert_eq!(
        app.world().resource::<Assets<Image>>().len(),
        baseline,
        "a closed enhancement owns no raster"
    );
    assert!(app.world().get::<ImageNode>(entity).is_none());
    assert!(
        app.world().get::<GlobePlate>(entity).is_some(),
        "the plate itself stays mounted"
    );
}

/// The stage's typed transitions: the idle clock, the sleep, the wake and
/// the close are one value, never a set of flags.
#[test]
fn globe_stage_transitions_are_one_typed_value() {
    let live = GlobeStage::Live { idle_seconds: 0.0 };
    assert!(live.is_live());
    assert_eq!(
        live.tick(1.0, 20.0, GlobeActivity::Idle),
        GlobeStage::Live { idle_seconds: 1.0 }
    );
    assert_eq!(
        live.tick(30.0, 20.0, GlobeActivity::Idle),
        GlobeStage::Sleeping,
        "the idle budget spends the enhancement"
    );
    assert_eq!(
        live.tick(30.0, 20.0, GlobeActivity::Busy),
        GlobeStage::Live { idle_seconds: 0.0 },
        "an animating globe is not idle"
    );
    assert_eq!(
        GlobeStage::Sleeping.tick(30.0, 20.0, GlobeActivity::Idle),
        GlobeStage::Sleeping,
        "a sleeping enhancement stays asleep"
    );
    assert_eq!(
        GlobeStage::Sleeping.interact(),
        GlobeStage::Live { idle_seconds: 0.0 },
        "a pointer gesture wakes the enhancement"
    );
    assert_eq!(
        GlobeStage::Closed.interact(),
        GlobeStage::Closed,
        "a closed enhancement is only opened on demand"
    );
    assert_eq!(
        GlobeStage::Closed.open(),
        GlobeStage::Live { idle_seconds: 0.0 },
        "an explicit demand opens the enhancement"
    );
    assert!(GlobeStage::Closed.close().is_closed());
}

/// The source identity fingerprints the source, not the frame: the view and
/// the flow phase do not change it, another node does.
#[test]
fn globe_report_is_bound_to_the_source_identity() {
    let spec = sample_spec();
    let identity = GlobeSourceIdentity::of(&spec);
    assert_eq!(identity, GlobeSourceIdentity::of(&spec.clone()));

    let mut moved = spec.clone();
    moved.drag(Vec2::new(120.0, 40.0));
    moved.flow_phase = 0.75;
    assert_eq!(
        GlobeSourceIdentity::of(&moved),
        identity,
        "the source identity survives the view and the flow"
    );

    let mut other = spec.clone();
    other.nodes.push(GlobeNode::new(
        "relay",
        "Relay",
        known(0.0, 30.0),
        GlobeNodeCategory::Relay,
    ));
    assert_ne!(
        GlobeSourceIdentity::of(&other),
        identity,
        "another node is another source"
    );

    let window = GlobeFrameWindow {
        frames: 120,
        elapsed_seconds: 2.0,
        owned_bytes: spec.raster_bytes(),
    };
    let report = GlobeFrameReport::from_window(&spec, window);
    assert_eq!(report.source, identity);
    assert!(report.describes(&spec));
    assert!(!report.describes(&other));
    assert!((report.fps() - 60.0).abs() < 1e-3);
}

/// The report is compared against the measured baseline — and a baseline of
/// another source is never a silent pass.
#[test]
fn globe_report_is_compared_against_the_measured_baseline() {
    let spec = sample_spec();
    let baseline = GlobeBaseline::of(&spec, 30.0, spec.raster_bytes());
    let report = GlobeFrameReport::from_window(
        &spec,
        GlobeFrameWindow {
            frames: 60,
            elapsed_seconds: 1.0,
            owned_bytes: spec.raster_bytes(),
        },
    );
    assert_eq!(report.compare(&baseline), GlobeBudgetVerdict::WithinBudget);

    let slow = GlobeFrameReport::from_window(
        &spec,
        GlobeFrameWindow {
            frames: 10,
            elapsed_seconds: 1.0,
            owned_bytes: 0,
        },
    );
    assert_eq!(
        slow.compare(&baseline),
        GlobeBudgetVerdict::FrameRateBelow {
            measured_fps: 10.0,
            baseline_fps: 30.0,
        }
    );

    let heavy = GlobeFrameReport::from_window(
        &spec,
        GlobeFrameWindow {
            frames: 60,
            elapsed_seconds: 1.0,
            owned_bytes: spec.raster_bytes() * 2,
        },
    );
    assert_eq!(
        heavy.compare(&baseline),
        GlobeBudgetVerdict::MemoryAbove {
            measured_bytes: spec.raster_bytes() * 2,
            baseline_bytes: spec.raster_bytes(),
        }
    );

    let mut other = spec.clone();
    other.nodes.push(GlobeNode::new(
        "relay",
        "Relay",
        known(0.0, 30.0),
        GlobeNodeCategory::Relay,
    ));
    let foreign = GlobeBaseline::of(&other, 30.0, u64::MAX);
    assert_eq!(
        report.compare(&foreign),
        GlobeBudgetVerdict::OtherSource {
            report_source: GlobeSourceIdentity::of(&spec),
            baseline_source: GlobeSourceIdentity::of(&other),
        },
        "a baseline of another source is not comparable, not a pass"
    );
}

/// The measurement window counts the host's frames and the raster bytes the
/// enhancement owns: displayed and closed are directly comparable.
#[test]
fn globe_measurement_counts_the_frames_and_the_owned_bytes() {
    let mut app = app();
    app.update();
    let entity = {
        let mut commands = app.world_mut().commands();
        commands.spawn_scene(globe_scene(sample_spec())).id()
    };
    app.world_mut().flush();

    *app.world_mut().resource_mut::<GlobeMeasurement>() = GlobeMeasurement::default().open();
    app.update();
    app.update();
    let window = app
        .world()
        .resource::<GlobeMeasurement>()
        .close()
        .expect("the measurement window was open");
    assert!(window.frames >= 1, "the window counted the host's frames");
    assert!(
        window.owned_bytes > 0,
        "a displayed enhancement owns its raster bytes"
    );

    set_stage(&mut app, entity, GlobeStage::Closed);
    *app.world_mut().resource_mut::<GlobeMeasurement>() = GlobeMeasurement::default().open();
    app.update();
    app.update();
    let closed = app
        .world()
        .resource::<GlobeMeasurement>()
        .close()
        .expect("the measurement window was open");
    assert!(
        closed.frames >= 1,
        "the closed window still counts the host's frames"
    );
    assert_eq!(
        closed.owned_bytes, 0,
        "a closed enhancement owns no raster bytes"
    );
}
