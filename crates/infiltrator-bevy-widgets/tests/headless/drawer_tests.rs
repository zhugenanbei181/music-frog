//! Headless tests for Drawer: edge-docked panel slide trajectory across all placements.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin, bsn};
use bevy::ui::prelude::Node;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::drawer::{
    DrawerPanel, DrawerPlacement, DrawerScrim, drawer_rect, drawer_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::popover::Rect;
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn drawer_rect_computes_slide_geometry_for_all_edges() {
    let viewport = Rect {
        x: 0.0,
        y: 0.0,
        w: 1000.0,
        h: 800.0,
    };
    let size = 300.0;

    // Left drawer fully open (ratio = 1.0)
    let left_open = drawer_rect(DrawerPlacement::Left, size, viewport, 1.0);
    assert_eq!(left_open.x, 0.0);
    assert_eq!(left_open.w, 300.0);
    assert_eq!(left_open.h, 800.0);

    // Left drawer half open (ratio = 0.5) -> x = -150.0
    let left_half = drawer_rect(DrawerPlacement::Left, size, viewport, 0.5);
    assert_eq!(left_half.x, -150.0);

    // Right drawer fully open -> x = 700.0
    let right_open = drawer_rect(DrawerPlacement::Right, size, viewport, 1.0);
    assert_eq!(right_open.x, 700.0);
    assert_eq!(right_open.w, 300.0);

    // Top drawer fully open -> y = 0.0, h = 300.0
    let top_open = drawer_rect(DrawerPlacement::Top, size, viewport, 1.0);
    assert_eq!(top_open.y, 0.0);
    assert_eq!(top_open.h, 300.0);

    // Bottom drawer fully open -> y = 500.0, h = 300.0
    let bot_open = drawer_rect(DrawerPlacement::Bottom, size, viewport, 1.0);
    assert_eq!(bot_open.y, 500.0);
    assert_eq!(bot_open.h, 300.0);
}

#[test]
fn drawer_scene_spawns_scrim_and_panel() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let content = Box::new(bsn! { ( Text({ "Drawer Content".to_owned() }) ) });
            commands.spawn_scene(drawer_scene(
                DrawerPlacement::Left,
                280.0,
                content,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut scrims = world.query::<&DrawerScrim>();
    assert_eq!(scrims.iter(world).count(), 1);

    let mut panels = world.query::<(&DrawerPanel, &Node)>();
    let (panel, node) = panels.iter(world).next().expect("drawer panel mounted");
    assert_eq!(panel.0, DrawerPlacement::Left);
    assert_eq!(node.width, bevy::ui::Val::Px(280.0));
}
