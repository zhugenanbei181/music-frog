//! Headless tests for Tabs & SegmentedControl: capsule percentage metrics,
//! pixel bounds, selection state, and sliding indicator sync.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::{Node, Val};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::tabs::{
    SegmentedControlValue, SegmentedPillIndicator, TabSelectEvent, capsule_metrics,
    capsule_px_bounds, segmented_control_scene,
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
fn capsule_metrics_and_pixel_bounds_math() {
    // 3 tabs: width is 33.333%, index 0 at 0%, index 1 at 33.333%, index 2 at 66.667%
    let (l0, w0) = capsule_metrics(0, 3);
    assert_eq!(l0, 0.0);
    assert!((w0 - 33.3333).abs() < 0.01);

    let (l1, w1) = capsule_metrics(1, 3);
    assert!((l1 - 33.3333).abs() < 0.01);
    assert!((w1 - 33.3333).abs() < 0.01);

    let (l2, _w2) = capsule_metrics(2, 3);
    assert!((l2 - 66.6666).abs() < 0.01);

    // Pixel bounds: 300px total, 4px padding -> 292px usable / 4 tabs = 73px per tab
    let (px_l0, px_w0) = capsule_px_bounds(0, 4, 300.0, 4.0);
    assert_eq!(px_l0, 4.0);
    assert_eq!(px_w0, 73.0);

    let (px_l2, px_w2) = capsule_px_bounds(2, 4, 300.0, 4.0);
    assert_eq!(px_l2, 4.0 + 73.0 * 2.0);
    assert_eq!(px_w2, 73.0);
}

#[test]
fn segmented_control_scene_spawns_and_syncs_capsule_indicator() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let tabs = vec!["Rule".to_owned(), "Global".to_owned(), "Direct".to_owned()];
            commands.spawn_scene(segmented_control_scene(tabs, 0, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut controls = world.query::<(Entity, &SegmentedControlValue)>();
    let (ctrl_entity, val) = controls
        .iter(world)
        .next()
        .expect("segmented control mounted");
    assert_eq!(val.0, 0);

    // Send selection change event
    app.world_mut().write_message(TabSelectEvent {
        container: ctrl_entity,
        selected_index: 2,
    });
    app.update();

    let world = app.world();
    let val_after = world
        .get::<SegmentedControlValue>(ctrl_entity)
        .expect("value exists");
    assert_eq!(val_after.0, 2);

    let world = app.world_mut();
    let mut indicators = world.query::<(&SegmentedPillIndicator, &Node)>();
    let (_, ind_node) = indicators.iter(world).next().expect("indicator exists");
    if let Val::Percent(left_pct) = ind_node.left {
        assert!((left_pct - 66.666).abs() < 0.1);
    } else {
        panic!("indicator left must be percentage");
    }
}
