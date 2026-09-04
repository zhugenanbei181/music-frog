//! Headless tests for the telemetry & chart system: smooth Bezier waveforms,
//! dual-series shared scaling, GPU 2D mesh compilation, multi-dimensional
//! charts (donut, histogram, topology), interactive crosshairs, LTTB downsampling,
//! zero-allocation RingBuffer data pumps, and background power throttling.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::Assets;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Commands;
use bevy::image::Image;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::chart::bezier::{
    PlotPoint, ScaleMode, bezier_smooth_polyline, build_dual_series_curves, compute_shared_scale,
};
use infiltrator_bevy_widgets::chart::donut::{
    DonutChartPlate, DonutChartSpec, DonutSlice, calculate_donut_geometry, donut_chart_scene,
    rasterize_donut,
};
use infiltrator_bevy_widgets::chart::histogram::{
    HistogramBin, HistogramPlate, HistogramSpec, LatencyTier, histogram_scene, rasterize_histogram,
    tier_to_rgba,
};
use infiltrator_bevy_widgets::chart::interaction::{
    CrosshairState, TimeRangeZoom, apply_zoom_pan, decimate_lttb, decimate_min_max,
    find_nearest_sample_index,
};
use infiltrator_bevy_widgets::chart::mesh::{
    build_curve_area_mesh, build_curve_ribbon_mesh, build_donut_sector_mesh,
};
use infiltrator_bevy_widgets::chart::ring_buffer::{
    CadenceMode, FixedRingBuffer, TelemetryCadenceManager, TelemetryStatistics,
};
use infiltrator_bevy_widgets::chart::topology::{
    TopologyLink, TopologyNode, TopologyPlate, TopologySpec, rasterize_topology, topology_scene,
};
use infiltrator_bevy_widgets::chart::{
    ChartLayer, ChartPlate, ChartSpec, Grid, chart_scene, polyline, rasterize, to_rgba8,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

/// Pixel color at (x, y) in an RGBA8 buffer of the given width.
fn pixel(data: &[u8], width: u32, x: i32, y: i32) -> [u8; 4] {
    let offset = (y as usize * width as usize + x as usize) * 4;
    data[offset..offset + 4]
        .try_into()
        .expect("in-bounds pixel")
}

#[test]
fn polyline_normalizes_and_marks_gaps() {
    assert!(polyline(&[], 10.0, 10.0).is_empty(), "empty stays empty");

    let single = polyline(&[7.0], 100.0, 40.0);
    assert_eq!(single, vec![Some(PlotPoint { x: 100.0, y: 20.0 })]);

    let rising = polyline(&[0.0, 1.0], 10.0, 20.0);
    assert_eq!(
        rising,
        vec![
            Some(PlotPoint { x: 0.0, y: 20.0 }),
            Some(PlotPoint { x: 10.0, y: 0.0 }),
        ]
    );

    let flat = polyline(&[5.0, 5.0], 10.0, 20.0);
    assert!(flat.iter().all(|point| point.map(|p| p.y) == Some(10.0)));

    let gapped = polyline(&[0.0, f32::NAN, 1.0], 10.0, 20.0);
    assert!(gapped[1].is_none(), "the NaN slot is a gap");
    assert_eq!(gapped[0], Some(PlotPoint { x: 0.0, y: 20.0 }));
    assert_eq!(gapped[2], Some(PlotPoint { x: 10.0, y: 0.0 }));

    assert!(
        polyline(&[f32::NAN, f32::INFINITY], 10.0, 10.0)
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn rasterizer_draws_a_two_pixel_line_breaking_at_gaps() {
    let width = 8;
    let height = 6;
    let ink = [255u8, 0, 0, 255];
    let layer = ChartLayer {
        points: polyline(&[1.0, 1.0], width as f32, height as f32),
        line: ink,
        fill: None,
    };
    let data = rasterize(width, height, None, &[layer]);
    assert_eq!(pixel(&data, width, 2, 3), ink, "line upper row");
    assert_eq!(pixel(&data, width, 2, 4), ink, "line lower row");
    assert_eq!(pixel(&data, width, 2, 0), [0, 0, 0, 0], "sky stays clear");

    let layer = ChartLayer {
        points: polyline(&[0.0, 1.0, f32::NAN, 1.0, 0.0], width as f32, height as f32),
        line: ink,
        fill: None,
    };
    let data = rasterize(width, height, None, &[layer]);
    assert_ne!(
        pixel(&data, width, 1, 3),
        [0, 0, 0, 0],
        "the left run draws"
    );
    assert_ne!(
        pixel(&data, width, 6, 0),
        [0, 0, 0, 0],
        "the right run draws"
    );
    assert_eq!(
        pixel(&data, width, 4, 3),
        [0, 0, 0, 0],
        "the gap column never bridges"
    );
}

#[test]
fn rasterizer_fades_the_fill_and_paints_the_grid() {
    let width = 8;
    let height = 6;
    let fill = [0u8, 255, 0, 255];
    let layer = ChartLayer {
        points: polyline(&[1.0, 1.0], width as f32, height as f32),
        line: [255u8, 255, 255, 255],
        fill: Some(fill),
    };
    let data = rasterize(width, height, None, &[layer]);
    let near = pixel(&data, width, 3, 5)[3];
    assert!(near > 0, "the fill paints under the line");
    assert_eq!(pixel(&data, width, 3, 3), [255, 255, 255, 255]);

    let grid = Grid {
        fractions: vec![0.25],
        ink: to_rgba8(Color::srgba(1.0, 1.0, 1.0, 1.0)),
    };
    let data = rasterize(width, height, Some(grid), &[]);
    assert_eq!(pixel(&data, width, 5, 2), [255, 255, 255, 255], "grid row");
    assert_eq!(pixel(&data, width, 5, 1), [0, 0, 0, 0], "no stray fill");
}

#[test]
fn to_rgba8_is_channel_exact() {
    assert_eq!(
        to_rgba8(Color::srgba(0.5, 0.0, 1.0, 0.5)),
        [128, 0, 255, 128]
    );
}

#[test]
fn chart_scene_stamps_an_image_node_and_updates_rewrite_the_same_handle() {
    let mut app = headless_app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(chart_scene(
            vec![0.0, 1.0, 0.5],
            vec![1.0, 0.0, 0.5],
            200.0,
            60.0,
        ));
    });
    app.update();

    let plate_id: Entity = {
        let world = app.world_mut();
        let mut plates = world.query::<(Entity, &ChartPlate, &ImageNode)>();
        let (entity, plate, _node) = plates.iter(world).next().expect("chart drawn on mount");
        assert_eq!(plate.0.width, 200);
        assert_eq!(plate.0.height, 60);
        entity
    };

    let handle_of = |world: &bevy::ecs::world::World| {
        world
            .get::<ImageNode>(plate_id)
            .expect("image node survives")
            .image
            .clone()
    };
    let data_of = |world: &bevy::ecs::world::World, handle: &bevy::asset::Handle<Image>| {
        world
            .resource::<Assets<Image>>()
            .get(handle)
            .expect("chart image asset")
            .clone()
    };

    let handle = handle_of(app.world());
    let before = data_of(app.world(), &handle);
    assert_eq!(before.texture_descriptor.size.width, 200);
    assert_eq!(before.texture_descriptor.size.height, 60);

    {
        let world = app.world_mut();
        world.entity_mut(plate_id).insert(ChartPlate(ChartSpec::new(
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 1.0],
            200,
            60,
        )));
    }
    app.update();
    let world = app.world_mut();
    assert_eq!(
        handle_of(world).id(),
        handle.id(),
        "the handle never rotates"
    );
    let after = data_of(world, &handle_of(world));
    assert_ne!(before.data, after.data, "the raster followed the samples");

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();
    let world = app.world_mut();
    assert_eq!(
        handle_of(world).id(),
        handle.id(),
        "still the same handle after the retheme"
    );
    let retheme = data_of(world, &handle_of(world));
    assert_ne!(after.data, retheme.data, "the inks re-derived from light");
    assert!(
        world.get::<ChartPlate>(plate_id).is_some(),
        "the chart kept its entity id throughout"
    );
}

#[test]
fn bezier_smooth_normalizes_and_preserves_monotonicity() {
    let samples = vec![0.0, 10.0, 20.0, 30.0];
    let smooth_pts = bezier_smooth_polyline(&samples, 100.0, 50.0, None, 8);
    assert!(!smooth_pts.is_empty());

    let finite_pts: Vec<PlotPoint> = smooth_pts.iter().filter_map(|p| *p).collect();
    for i in 0..finite_pts.len() - 1 {
        assert!(
            finite_pts[i].x <= finite_pts[i + 1].x,
            "X coordinates strictly advance"
        );
        assert!(
            finite_pts[i].y >= finite_pts[i + 1].y - 1e-4,
            "Y coordinates monotonically rise (y decreases upwards)"
        );
    }

    let gapped = vec![1.0, 2.0, f32::NAN, 4.0, 5.0];
    let pts = bezier_smooth_polyline(&gapped, 100.0, 50.0, None, 4);
    assert!(
        pts.iter().any(|p| p.is_none()),
        "gaps are preserved as None"
    );
}

#[test]
fn shared_scale_dynamic_normalization_and_magnitude_comparison() {
    let up = vec![10.0, 10.0, 10.0];
    let down = vec![100.0, 100.0, 100.0];

    let shared_scale = compute_shared_scale(&up, &down, 0.05);
    assert!((shared_scale - 105.0).abs() < 1e-3);

    let (up_curves, down_curves, scale) =
        build_dual_series_curves(&up, &down, 100.0, 100.0, ScaleMode::Shared, false);
    assert_eq!(scale, shared_scale);

    let up_y = up_curves[0].unwrap().y;
    let down_y = down_curves[0].unwrap().y;

    assert!(
        up_y > down_y,
        "higher traffic draws closer to the top (lower y)"
    );
    assert!((up_y - 90.47).abs() < 1.0);
    assert!((down_y - 4.76).abs() < 1.0);
}

#[test]
fn mesh_generation_waveform_donut_and_histogram() {
    let pts = vec![
        Some(PlotPoint::new(0.0, 50.0)),
        Some(PlotPoint::new(50.0, 20.0)),
        Some(PlotPoint::new(100.0, 30.0)),
    ];
    let area_mesh = build_curve_area_mesh(&pts, [1.0, 0.0, 0.0, 0.5], [1.0, 0.0, 0.0, 0.0], 100.0);
    assert_eq!(area_mesh.vertices.len(), 6);
    assert_eq!(area_mesh.indices.len(), 12);

    let ribbon_mesh = build_curve_ribbon_mesh(&pts, 2.0, [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(ribbon_mesh.vertices.len(), 6);

    let bevy_mesh = area_mesh.to_bevy_mesh();
    assert!(
        bevy_mesh
            .attribute(bevy::render::mesh::Mesh::ATTRIBUTE_POSITION)
            .is_some()
    );
    assert!(
        bevy_mesh
            .attribute(bevy::render::mesh::Mesh::ATTRIBUTE_COLOR)
            .is_some()
    );

    let sector =
        build_donut_sector_mesh([50.0, 50.0], 20.0, 40.0, 0.0, 1.57, [0.0, 1.0, 0.0, 1.0], 8);
    assert!(!sector.vertices.is_empty());
    assert!(!sector.indices.is_empty());
}

#[test]
fn donut_chart_geometry_and_scene_sync() {
    let slices = vec![
        DonutSlice::new("Direct", 70.0, [0, 255, 0, 255]),
        DonutSlice::new("Proxy", 30.0, [0, 128, 255, 255]),
    ];
    let geo = calculate_donut_geometry(&slices, 0.04);
    assert_eq!(geo.len(), 2);
    assert!((geo[0].percentage - 70.0).abs() < 1e-3);
    assert!((geo[1].percentage - 30.0).abs() < 1e-3);

    let spec = DonutChartSpec::new(slices, 100, 100);
    let pixels = rasterize_donut(&spec, &UiPalette::new(&Theme::dark()));
    assert_eq!(pixels.len(), 100 * 100 * 4);

    let mut app = headless_app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(donut_chart_scene(DonutChartSpec::new(
            vec![DonutSlice::new("Direct", 10.0, [0, 255, 0, 255])],
            80,
            80,
        )));
    });
    app.update();

    let world = app.world_mut();
    let mut plates = world.query::<(Entity, &DonutChartPlate, &ImageNode)>();
    let (_entity, plate, _node) = plates.iter(world).next().expect("donut plate mounted");
    assert_eq!(plate.0.width, 80);
}

#[test]
fn histogram_latency_tiers_and_scene_sync() {
    let bins = vec![
        HistogramBin::new("<50ms", 0.0, 50.0, 120, LatencyTier::Fast),
        HistogramBin::new("50-100ms", 50.0, 100.0, 45, LatencyTier::Normal),
        HistogramBin::new("100-300ms", 100.0, 300.0, 12, LatencyTier::Slow),
        HistogramBin::new("Timeout", 300.0, 1000.0, 2, LatencyTier::Timeout),
    ];
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(
        tier_to_rgba(LatencyTier::Fast, &palette),
        to_rgba8(palette.success)
    );
    assert_eq!(
        tier_to_rgba(LatencyTier::Timeout, &palette),
        to_rgba8(palette.danger)
    );

    let spec = HistogramSpec::new(bins, 160, 80);
    let pixels = rasterize_histogram(&spec, &palette);
    assert_eq!(pixels.len(), 160 * 80 * 4);

    let mut app = headless_app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(histogram_scene(HistogramSpec::new(Vec::new(), 120, 60)));
    });
    app.update();

    let world = app.world_mut();
    let mut plates = world.query::<(Entity, &HistogramPlate, &ImageNode)>();
    assert!(plates.iter(world).next().is_some());
}

#[test]
fn topology_diagram_ribbon_flow_and_scene_sync() {
    let nodes = vec![
        TopologyNode::new(
            "inbound",
            "Chrome",
            infiltrator_bevy_widgets::chart::topology::NodeCategory::Inbound,
            0.1,
            0.5,
        ),
        TopologyNode::new(
            "rule",
            "Rule Router",
            infiltrator_bevy_widgets::chart::topology::NodeCategory::Rule,
            0.5,
            0.5,
        ),
        TopologyNode::new(
            "outbound",
            "HK-Node",
            infiltrator_bevy_widgets::chart::topology::NodeCategory::Outbound,
            0.9,
            0.5,
        ),
    ];
    let links = vec![
        TopologyLink::new("inbound", "rule", 5_000_000.0),
        TopologyLink::new("rule", "outbound", 4_500_000.0),
    ];
    let spec = TopologySpec::new(nodes, links, 200, 100);
    let palette = UiPalette::new(&Theme::dark());
    let pixels = rasterize_topology(&spec, &palette);
    assert_eq!(pixels.len(), 200 * 100 * 4);

    let mut app = headless_app();
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(topology_scene(TopologySpec::new(
            Vec::new(),
            Vec::new(),
            100,
            50,
        )));
    });
    app.update();

    let world = app.world_mut();
    let mut plates = world.query::<(Entity, &TopologyPlate, &ImageNode)>();
    assert!(plates.iter(world).next().is_some());
}

#[test]
fn interactive_crosshair_and_nearest_sample_snapping() {
    let sample_idx = find_nearest_sample_index(10, 100.0, 48.0);
    assert_eq!(sample_idx, Some(4), "snaps to closest 4th sample");

    let crosshair = CrosshairState::new(48.0, 25.0);
    assert!(crosshair.active);
    assert_eq!(crosshair.cursor_x, 48.0);
}

#[test]
fn time_range_zoom_and_lttb_decimation() {
    let samples: Vec<f32> = (0..100).map(|i| (i as f32 * 0.1).sin()).collect();

    let zoom = TimeRangeZoom::new(2.0, 0.0);
    let sub = apply_zoom_pan(&samples, &zoom);
    assert_eq!(sub.len(), 50);

    let lttb = decimate_lttb(&samples, 10);
    assert_eq!(lttb.len(), 10);
    assert_eq!(lttb[0], samples[0]);
    assert_eq!(*lttb.last().unwrap(), *samples.last().unwrap());

    let minmax = decimate_min_max(&samples, 20);
    assert_eq!(minmax.len(), 20);
}

#[test]
fn fixed_ring_buffer_zero_alloc_and_statistics() {
    let mut ring: FixedRingBuffer<f32, 4> = FixedRingBuffer::new();
    assert!(ring.is_empty());
    assert_eq!(ring.capacity(), 4);

    ring.push(10.0);
    ring.push(20.0);
    ring.push(30.0);
    ring.push(40.0);
    assert!(ring.is_full());
    assert_eq!(ring.len(), 4);
    assert_eq!(ring.oldest(), Some(&10.0));
    assert_eq!(ring.newest(), Some(&40.0));

    ring.push(50.0);
    assert_eq!(ring.len(), 4);
    assert_eq!(ring.oldest(), Some(&20.0));
    assert_eq!(ring.newest(), Some(&50.0));
    assert_eq!(ring.to_vec(), vec![20.0, 30.0, 40.0, 50.0]);

    let stats = TelemetryStatistics::compute(&ring.to_vec(), 0.2);
    assert_eq!(stats.count, 4);
    assert_eq!(stats.min, 20.0);
    assert_eq!(stats.max, 50.0);
    assert_eq!(stats.mean, 35.0);
    assert_eq!(stats.sum, 140.0);
}

#[test]
fn telemetry_cadence_power_throttling() {
    let mut manager = TelemetryCadenceManager::default();
    assert_eq!(manager.mode, CadenceMode::ForegroundActive);

    assert!(manager.on_frame(0.02));

    manager.is_window_focused = false;
    assert!(!manager.on_frame(0.016));
    assert_eq!(manager.mode, CadenceMode::BackgroundThrottled);

    assert!(manager.on_frame(1.05));

    manager.is_window_visible = false;
    assert!(!manager.on_frame(10.0));
    assert_eq!(manager.mode, CadenceMode::Suspended);

    manager.is_window_visible = true;
    manager.is_window_focused = true;
    manager.wake();
    assert!(manager.on_frame(0.001));
}

#[test]
fn test_radar_geometry_and_composite_health_scoring() {
    use infiltrator_bevy_widgets::chart::radar::{
        BandwidthSaturationDetector, HealthGrade, NodeHealthAssessment, RadarGeometry,
        SaturationLevel,
    };

    let assessment = NodeHealthAssessment::evaluate(45.0, 8.0, 1.0, 99.5);
    assert_eq!(assessment.grade, HealthGrade::Excellent);
    assert!(assessment.composite_score >= 85.0);

    let vertices = RadarGeometry::compute_vertices(100.0, 100.0, 50.0, &[0.8, 0.9, 0.7, 0.95]);
    assert_eq!(vertices.len(), 4);
    let area = RadarGeometry::polygon_area(&vertices);
    assert!(area > 0.0);

    let mut detector = BandwidthSaturationDetector::new(50_000_000);
    detector.update(46_000_000, 2_000_000);
    assert_eq!(detector.alert_level(40_000_000), SaturationLevel::Saturated);
}
