//! Headless tests for Splitter: size math, drag deltas, and in-place pane basis sync.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin, bsn};
use bevy::ui::prelude::{Node, Val};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::splitter::{
    SplitterDirection, SplitterDragEvent, SplitterFirstPane, SplitterFraction, SplitterSecondPane,
    apply_drag_delta, compute_pane_sizes, splitter_scene,
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
fn splitter_math_and_drag_clamping() {
    // 1000px total, 6px handle -> 994px available
    let (p1, p2) = compute_pane_sizes(0.5, 1000.0, 6.0);
    assert_eq!(p1, 497.0);
    assert_eq!(p2, 497.0);

    // 0.3 fraction
    let (f1, f2) = compute_pane_sizes(0.3, 1000.0, 6.0);
    assert_eq!(f1, 298.0);
    assert_eq!(f2, 696.0);

    // Drag delta: 0.5 + 100px on 1000px total = 0.6
    let next_f = apply_drag_delta(0.5, 100.0, 1000.0, 0.1, 0.9);
    assert!((next_f - 0.6).abs() < 1e-4);

    // Clamped at max fraction
    let clamp_max = apply_drag_delta(0.85, 200.0, 1000.0, 0.1, 0.9);
    assert_eq!(clamp_max, 0.9);
}

#[test]
fn splitter_scene_spawns_and_drag_event_updates_basis() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let left = Box::new(bsn! { ( Text({ "Left Pane".to_owned() }) ) });
            let right = Box::new(bsn! { ( Text({ "Right Pane".to_owned() }) ) });
            commands.spawn_scene(splitter_scene(
                SplitterDirection::Horizontal,
                0.4,
                left,
                right,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut roots = world.query::<(Entity, &SplitterFraction)>();
    let (root_entity, frac) = roots.iter(world).next().expect("splitter mounted");
    assert_eq!(frac.0, 0.4);

    // Send drag message
    app.world_mut().write_message(SplitterDragEvent {
        splitter: root_entity,
        delta_px: 200.0,
        total_size_px: 1000.0,
    });
    app.update();

    let world = app.world();
    let frac_after = world
        .get::<SplitterFraction>(root_entity)
        .expect("frac exists");
    assert!((frac_after.0 - 0.6).abs() < 1e-4);

    let world = app.world_mut();
    let mut firsts = world.query::<(&SplitterFirstPane, &Node)>();
    let (_, node1) = firsts.iter(world).next().expect("first pane exists");
    if let Val::Percent(p) = node1.flex_basis {
        assert!((p - 60.0).abs() < 1e-3);
    } else {
        panic!("expected percent flex_basis");
    }

    let mut seconds = world.query::<(&SplitterSecondPane, &Node)>();
    let (_, node2) = seconds.iter(world).next().expect("second pane exists");
    if let Val::Percent(p) = node2.flex_basis {
        assert!((p - 40.0).abs() < 1e-3);
    } else {
        panic!("expected percent flex_basis");
    }
}
