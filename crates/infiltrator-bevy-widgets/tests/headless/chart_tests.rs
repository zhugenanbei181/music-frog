//! Headless tests for the chart: the polyline projection (normalization,
//! gaps never bridged), the hand-written rasterizer's pixel contract, and
//! the asset adapter (ImageNode stamped on mount, series updates rewriting
//! the same handle, theme flips re-rasterizing in place).

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
use infiltrator_bevy_widgets::chart::{
    ChartLayer, ChartPlate, ChartSpec, Grid, PlotPoint, chart_scene, polyline, rasterize, to_rgba8,
};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    // The render-backed host registers Assets<Image> through its render
    // plugins; a headless composition registers the store itself so the
    // chart adapter's write-back path is exercisable.
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

    // A single sample pins to the newest edge at mid height.
    let single = polyline(&[7.0], 100.0, 40.0);
    assert_eq!(single, vec![Some(PlotPoint { x: 100.0, y: 20.0 })]);

    // Rising series: min sits on the bottom edge, max on the top edge.
    let rising = polyline(&[0.0, 1.0], 10.0, 20.0);
    assert_eq!(
        rising,
        vec![
            Some(PlotPoint { x: 0.0, y: 20.0 }),
            Some(PlotPoint { x: 10.0, y: 0.0 }),
        ]
    );

    // A constant series is a flat mid line.
    let flat = polyline(&[5.0, 5.0], 10.0, 20.0);
    assert!(flat.iter().all(|point| point.map(|p| p.y) == Some(10.0)));

    // Non-finite samples are gaps — never fabricated values.
    let gapped = polyline(&[0.0, f32::NAN, 1.0], 10.0, 20.0);
    assert!(gapped[1].is_none(), "the NaN slot is a gap");
    assert_eq!(gapped[0], Some(PlotPoint { x: 0.0, y: 20.0 }));
    assert_eq!(gapped[2], Some(PlotPoint { x: 10.0, y: 0.0 }));

    // No finite observation at all: every slot is a gap.
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
    // Constant series → flat mid line (rows 3 and 4, the 2px slab).
    let layer = ChartLayer {
        points: polyline(&[1.0, 1.0], width as f32, height as f32),
        line: ink,
        fill: None,
    };
    let data = rasterize(width, height, None, &[layer]);
    assert_eq!(pixel(&data, width, 2, 3), ink, "line upper row");
    assert_eq!(pixel(&data, width, 2, 4), ink, "line lower row");
    assert_eq!(pixel(&data, width, 2, 0), [0, 0, 0, 0], "sky stays clear");

    // A gap: no segment bridges it. Rising–gap–falling leaves the middle
    // columns unpainted while both sides draw.
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
    // Fill starts under the 2px slab (row 5) and fades downward; the alpha
    // channel must strictly decrease with depth.
    let near = pixel(&data, width, 3, 5)[3];
    assert!(near > 0, "the fill paints under the line");
    // The line on top stays fully opaque white where the slab sits.
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

    // New series data: restamp the spec, same handle, different pixels.
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

    // A theme flip re-derives the inks: same handle again, different raster.
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
