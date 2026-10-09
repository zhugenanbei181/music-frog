//! Headless tests for the optional 2.5D globe widget (BEVY-030 / BANDROID-016):
//! orthographic projection and back-hemisphere culling, the no-fabrication
//! geographic contract, great-circle arc generation, the flat 2D / Eco
//! fallbacks, and a bounded mount/retire asset baseline.

use super::render_cache_tests::{app, image_handle, modifications};
use bevy::asset::Assets;
use bevy::ecs::message::MessageCursor;
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::scene::CommandsSceneExt;
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::globe::{
    ARC_ALTITUDE_FRACTION, GeoLocation, GeoPoint, GeoPrecision, GeoSource, GeoUnknown, GlobeLink,
    GlobeMode, GlobeNode, GlobeNodeCategory, GlobePlate, GlobeProjection, GlobeSpec,
    build_globe_arcs, build_globe_particles, globe_scene, project_flat_nodes, project_globe_nodes,
    rasterize_globe,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;
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
        GlobeNode::new("home", "Home", known(39.9, 116.4), GlobeNodeCategory::Home)
            .with_weight(0.8),
        GlobeNode::new("exit", "Exit", known(37.7, -122.4), GlobeNodeCategory::Exit),
    ];
    let links = vec![GlobeLink::new("home", "exit", 0.0)];
    GlobeSpec::new(nodes, links, 160, 160).with_flow(0.0, 0.0)
}

#[test]
fn globe_projection_maps_center_north_east_and_culls_the_back() {
    let projection = GlobeProjection::new(0.0, 0.0, 100.0, Vec2::new(200.0, 150.0));

    let center = projection.project_lat_lon(0.0, 0.0).expect("front center");
    assert!((center.x - 200.0).abs() < 1e-3);
    assert!((center.y - 150.0).abs() < 1e-3);
    assert!((center.depth - 1.0).abs() < 1e-3);

    let north = projection.project_lat_lon(45.0, 0.0).expect("front north");
    assert!(north.y < 150.0, "north projects upward");
    let east = projection.project_lat_lon(0.0, 45.0).expect("front east");
    assert!(east.x > 200.0, "east projects rightward");

    assert!(
        projection.project_lat_lon(0.0, 120.0).is_none(),
        "the back hemisphere is culled"
    );
    assert!(projection.project_lat_lon(0.0, 180.0).is_none());
}

#[test]
fn globe_yaw_rotation_brings_the_other_hemisphere_into_view() {
    let nodes = vec![
        GlobeNode::new("front", "Front", known(0.0, 0.0), GlobeNodeCategory::Home),
        GlobeNode::new("back", "Back", known(0.0, 180.0), GlobeNodeCategory::Exit),
    ];
    let spec = GlobeSpec::new(nodes, Vec::new(), 200, 200);
    let front = project_globe_nodes(&spec, &GlobeProjection::from_spec(&spec));
    assert_eq!(front.len(), 1, "only the front node is visible at yaw 0");

    let rotated = spec.with_view(180.0, 0.0);
    let back = project_globe_nodes(&rotated, &GlobeProjection::from_spec(&rotated));
    assert_eq!(back.len(), 1, "yaw 180 exposes the opposite node");
}

#[test]
fn unknown_geo_can_never_become_a_fabricated_coordinate() {
    assert!(GeoPoint::new(f64::NAN, 0.0, GeoSource::GeoIpDatabase, GeoPrecision::Exact).is_none());
    assert!(
        GeoPoint::new(
            0.0,
            f64::INFINITY,
            GeoSource::Configured,
            GeoPrecision::City
        )
        .is_none()
    );
    assert!(GeoPoint::new(91.0, 0.0, GeoSource::EndpointReported, GeoPrecision::Exact).is_none());
    assert!(GeoPoint::new(0.0, 181.0, GeoSource::EndpointReported, GeoPrecision::Exact).is_none());

    let unknown = GeoLocation::Unknown(GeoUnknown::NotObserved);
    assert!(unknown.plottable().is_none());

    let country = GeoPoint::new(48.0, 2.0, GeoSource::GeoIpDatabase, GeoPrecision::Country)
        .expect("in range");
    assert!(!country.is_plottable());
    assert!(
        GeoLocation::Known(country).plottable().is_none(),
        "country-only data carries no plottable point"
    );

    let spec = GlobeSpec::new(
        vec![
            GlobeNode::new("a", "A", unknown, GlobeNodeCategory::Home),
            GlobeNode::new(
                "b",
                "B",
                GeoLocation::Known(country),
                GlobeNodeCategory::Exit,
            ),
        ],
        vec![GlobeLink::new("a", "b", 1.0)],
        100,
        100,
    );
    assert!(build_globe_arcs(&spec).is_empty());
    assert!(project_globe_nodes(&spec, &GlobeProjection::from_spec(&spec)).is_empty());
    assert!(project_flat_nodes(&spec).is_empty());
}

#[test]
fn globe_arcs_and_particles_require_real_known_endpoints() {
    let nodes = vec![
        GlobeNode::new("a", "A", known(0.0, 0.0), GlobeNodeCategory::Home),
        GlobeNode::new("b", "B", known(0.0, 90.0), GlobeNodeCategory::Exit),
        GlobeNode::new(
            "c",
            "C",
            GeoLocation::Unknown(GeoUnknown::SourceEmpty),
            GlobeNodeCategory::Relay,
        ),
    ];
    let mut link = GlobeLink::new("a", "b", 5_000_000.0);
    link.highlighted = true;
    let links = vec![link, GlobeLink::new("b", "c", 5_000_000.0)];
    let mut spec = GlobeSpec::new(nodes, links, 200, 200);
    spec.flow_speed = 1.0;

    let arcs = build_globe_arcs(&spec);
    assert_eq!(
        arcs.len(),
        1,
        "a link touching an unknown node is not an arc"
    );
    let mid = arcs[0].arc.sample_point(0.5, ARC_ALTITUDE_FRACTION);
    assert!(mid.length() > 1.0, "the arc lifts off the unit sphere");

    let emitter = build_globe_particles(&spec, 64);
    assert!(emitter.count() > 0);
    assert!(emitter.count() <= 64, "the emitter stays bounded");

    spec.mode = GlobeMode::Eco;
    assert_eq!(
        build_globe_particles(&spec, 64).count(),
        0,
        "Eco emits no particles"
    );
}

#[test]
fn globe_flat2d_and_eco_are_lossless_and_draw_no_particles() {
    let palette = UiPalette::new(&Theme::dark());
    let nodes = vec![
        GlobeNode::new("a", "A", known(0.0, 0.0), GlobeNodeCategory::Home),
        GlobeNode::new("b", "B", known(0.0, 180.0), GlobeNodeCategory::Exit),
    ];
    let base = GlobeSpec::new(nodes, vec![GlobeLink::new("a", "b", 5_000_000.0)], 120, 120)
        .with_flow(0.0, 1.0);

    let sphere_markers = project_globe_nodes(&base, &GlobeProjection::from_spec(&base));
    assert_eq!(sphere_markers.len(), 1, "the sphere culls one antipode");

    let flat = project_flat_nodes(&base);
    assert_eq!(flat.len(), 2, "the flat fallback is lossless");

    let flat_pixels = rasterize_globe(&base.clone().with_mode(GlobeMode::Flat2d), &palette);
    let eco_pixels = rasterize_globe(&base.clone().with_mode(GlobeMode::Eco), &palette);
    let sphere_pixels = rasterize_globe(&base, &palette);
    assert_eq!(flat_pixels.len(), 120 * 120 * 4);
    assert_eq!(eco_pixels.len(), flat_pixels.len());
    assert_ne!(flat_pixels, eco_pixels, "Eco drops the link geometry");
    assert_ne!(flat_pixels, sphere_pixels, "sphere and flat differ");
    assert_eq!(
        build_globe_particles(&base.with_mode(GlobeMode::Eco), 64).count(),
        0
    );
}

#[test]
fn globe_drag_clamps_pitch_and_advance_rotates_only_while_active() {
    let mut spec = GlobeSpec::new(Vec::new(), Vec::new(), 100, 100);
    spec.drag(Vec2::new(10_000.0, 10_000.0));
    assert!(
        (-89.0..=89.0).contains(&spec.pitch_deg),
        "pitch stays clamped"
    );
    assert!(
        (-180.0..=180.0).contains(&spec.yaw_deg),
        "yaw stays wrapped"
    );

    let mut app = app();
    let mut active = GlobeSpec::new(Vec::new(), Vec::new(), 100, 100);
    active.auto_rotate_speed_deg_per_sec = 90.0;
    let entity = {
        let mut commands = app.world_mut().commands();
        commands.spawn_scene(globe_scene(active)).id()
    };
    app.world_mut().flush();
    app.update();
    let before = app.world().get::<GlobePlate>(entity).unwrap().0.yaw_deg;
    sleep(time::Duration::from_millis(25));
    app.update();
    let after = app.world().get::<GlobePlate>(entity).unwrap().0.yaw_deg;
    assert_ne!(before, after, "auto-rotation advances yaw");

    let static_entity = {
        let mut commands = app.world_mut().commands();
        commands.spawn_scene(globe_scene(sample_spec())).id()
    };
    app.world_mut().flush();
    app.update();
    let yaw_before = app
        .world()
        .get::<GlobePlate>(static_entity)
        .unwrap()
        .0
        .yaw_deg;
    let phase_before = app
        .world()
        .get::<GlobePlate>(static_entity)
        .unwrap()
        .0
        .flow_phase;
    sleep(time::Duration::from_millis(25));
    app.update();
    let static_spec = &app.world().get::<GlobePlate>(static_entity).unwrap().0;
    assert_eq!(
        static_spec.yaw_deg, yaw_before,
        "a static globe never rotates"
    );
    assert_eq!(static_spec.flow_phase, phase_before);
}

#[test]
fn globe_mount_is_static_and_retire_returns_assets_to_baseline() {
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

    let handle = image_handle(&app, entity);
    let handle_id = handle.id();
    let mut cursor = MessageCursor::default();
    modifications(&app, &mut cursor, handle_id);
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(
        modifications(&app, &mut cursor, handle_id),
        0,
        "a static globe must not re-rasterize or re-upload"
    );
    // Release the test's own strong handle before retiring the plate; the
    // asset owner must be the entity, not the test harness.
    drop(handle);

    app.world_mut().entity_mut(entity).remove::<GlobePlate>();
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<Assets<Image>>().len(),
        baseline,
        "retiring the plate releases its owned texture"
    );
    assert!(app.world().get::<ImageNode>(entity).is_none());
    assert!(
        app.world().get_entity(entity).is_ok(),
        "the entity survives"
    );
}

#[test]
fn globe_one_hundred_mount_retire_cycles_return_assets_to_baseline() {
    let mut app = app();
    app.update();
    let baseline = app.world().resource::<Assets<Image>>().len();
    for cycle in 0..100 {
        let entity = {
            let mut commands = app.world_mut().commands();
            commands.spawn_scene(globe_scene(sample_spec())).id()
        };
        app.world_mut().flush();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            baseline + 1,
            "mount {cycle}"
        );
        app.world_mut().entity_mut(entity).remove::<GlobePlate>();
        app.update();
        app.update();
        assert_eq!(
            app.world().resource::<Assets<Image>>().len(),
            baseline,
            "retire {cycle}"
        );
    }
}
