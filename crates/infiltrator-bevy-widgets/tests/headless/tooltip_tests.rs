//! Headless tests for Tooltip: placement computation, viewport clamping, and bubble scene.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::Node;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::popover::Rect;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_bevy_widgets::tooltip::{
    TooltipBubble, TooltipPosition, compute_tooltip_rect, tooltip_scene,
};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn tooltip_geometry_and_viewport_clamping() {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        w: 800.0,
        h: 600.0,
    };
    let target = Rect {
        x: 300.0,
        y: 200.0,
        w: 100.0,
        h: 40.0,
    };
    let size = (120.0, 30.0);
    let gap = 8.0;

    // Top placement: centered horizontally (300 + (100 - 120)/2 = 290), y = 200 - 8 - 30 = 162
    let top_rect = compute_tooltip_rect(target, size, viewport, TooltipPosition::Top, gap);
    assert_eq!(top_rect.x, 290.0);
    assert_eq!(top_rect.y, 162.0);
    assert_eq!(top_rect.w, 120.0);
    assert_eq!(top_rect.h, 30.0);

    // Bottom placement: y = 240 + 8 = 248
    let bot_rect = compute_tooltip_rect(target, size, viewport, TooltipPosition::Bottom, gap);
    assert_eq!(bot_rect.x, 290.0);
    assert_eq!(bot_rect.y, 248.0);

    // Left placement: x = 300 - 8 - 120 = 172, cy = 200 + (40 - 30)/2 = 205
    let left_rect = compute_tooltip_rect(target, size, viewport, TooltipPosition::Left, gap);
    assert_eq!(left_rect.x, 172.0);
    assert_eq!(left_rect.y, 205.0);

    // Out of bounds target clamps inside viewport
    let corner_target = Rect {
        x: 10.0,
        y: 10.0,
        w: 20.0,
        h: 20.0,
    };
    let corner_top = compute_tooltip_rect(corner_target, size, viewport, TooltipPosition::Top, gap);
    assert!(corner_top.x >= 0.0);
    assert!(corner_top.y >= 0.0);
}

#[test]
fn tooltip_scene_spawns_bubble() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let target = Rect {
                x: 100.0,
                y: 100.0,
                w: 50.0,
                h: 30.0,
            };
            let viewport = Rect {
                x: 0.0,
                y: 0.0,
                w: 800.0,
                h: 600.0,
            };
            commands.spawn_scene(tooltip_scene(
                "Tooltip text".to_owned(),
                target,
                viewport,
                TooltipPosition::Top,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut bubbles = world.query::<(&TooltipBubble, &Node)>();
    let (_, node) = bubbles.iter(world).next().expect("tooltip bubble mounted");
    assert!(matches!(node.left, bevy::ui::Val::Px(_)));
}
