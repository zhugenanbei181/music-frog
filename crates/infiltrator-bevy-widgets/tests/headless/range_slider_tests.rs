//! Headless tests for RangeSlider: dual-thumb fraction math, clamping,
//! step snapping, and in-place fill/thumb re-projections.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::{Node, Val};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::slider::{
    RangeSliderFill, RangeSliderRange, RangeSliderThumbMax, RangeSliderThumbMin, RangeSliderValues,
    clamp_range_values, range_slider_fractions, range_slider_scene, step_value,
};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn range_slider_fractions_and_clamping_math() {
    // Normal range
    let (s, e) = range_slider_fractions(20.0, 80.0, 0.0, 100.0);
    assert!((s - 0.2).abs() < 1e-4);
    assert!((e - 0.8).abs() < 1e-4);

    // Inverted input gets normalized
    let (s_inv, e_inv) = range_slider_fractions(80.0, 20.0, 0.0, 100.0);
    assert!((s_inv - 0.2).abs() < 1e-4);
    assert!((e_inv - 0.8).abs() < 1e-4);

    // Out of bounds clamped
    let (s_clamp, e_clamp) = range_slider_fractions(-50.0, 150.0, 0.0, 100.0);
    assert_eq!(s_clamp, 0.0);
    assert_eq!(e_clamp, 1.0);

    // Min span clamping
    let (s_span, e_span) = clamp_range_values(50.0, 52.0, 0.0, 100.0, 10.0);
    assert!((e_span - s_span - 10.0).abs() < 1e-4);

    // Step snapping
    assert_eq!(step_value(23.4, 5.0, 0.0, 100.0), 25.0);
    assert_eq!(step_value(22.0, 5.0, 0.0, 100.0), 20.0);
}

#[test]
fn range_slider_scene_spawns_and_syncs_geometry() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(range_slider_scene(25.0, 75.0, 0.0, 100.0, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut sliders = world.query::<(Entity, &RangeSliderValues, &RangeSliderRange)>();
    let (entity, values, range) = sliders.iter(world).next().expect("range slider mounted");
    assert_eq!(values.start, 25.0);
    assert_eq!(values.end, 75.0);
    assert_eq!(range.min, 0.0);
    assert_eq!(range.max, 100.0);

    // Mutate values and verify repaint in place
    let world = app.world_mut();
    world
        .entity_mut(entity)
        .insert(RangeSliderValues::new(10.0, 90.0));
    app.update();

    let world = app.world_mut();
    let mut fills = world.query::<(&RangeSliderFill, &Node)>();
    let (_, fill_node) = fills.iter(world).next().expect("fill mounted");
    assert_eq!(fill_node.left, Val::Percent(10.0));
    if let Val::Percent(w) = fill_node.width {
        assert!((w - 80.0).abs() < 1e-4);
    } else {
        panic!("expected percent width");
    }

    let mut min_thumbs = world.query::<(&RangeSliderThumbMin, &Node)>();
    let (_, min_node) = min_thumbs.iter(world).next().expect("min thumb mounted");
    assert_eq!(min_node.left, Val::Percent(10.0));

    let mut max_thumbs = world.query::<(&RangeSliderThumbMax, &Node)>();
    let (_, max_node) = max_thumbs.iter(world).next().expect("max thumb mounted");
    assert_eq!(max_node.left, Val::Percent(90.0));
}
