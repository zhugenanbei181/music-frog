//! Headless tests for the scrollarea wrapper: the official `ScrollArea`
//! primitive rides the viewport node, the chrome carries token colors, and
//! the pure clamp mirrors the over-scroll contract.

use bevy::app::Startup;
use bevy::asset::AssetPlugin;
use bevy::ecs::system::{Commands, Res};
use bevy::MinimalPlugins;
use bevy::scene::CommandsSceneExt;
use bevy::scene::ScenePlugin;
use bevy::ui::ScrollPosition;
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::scrollarea::{clamp_scroll, scrollarea_scene};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn scrollarea_spawns_official_viewport_over_token_chrome() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let content = checkbox_scene("node-a".to_string(), false, &palette);
            commands.spawn_scene(scrollarea_scene(Box::new(content), 240.0, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut areas = world.query::<&ScrollArea>();
    assert_eq!(areas.iter(world).count(), 1, "one official viewport");

    let world = app.world_mut();
    let mut positions = world.query::<&ScrollPosition>();
    assert!(
        positions.iter(world).next().is_some(),
        "ScrollArea requires the scroll position state"
    );

    let world = app.world_mut();
    let mut checkboxes = world.query::<&Checkbox>();
    assert_eq!(
        checkboxes.iter(world).count(),
        1,
        "the composed content scene is hosted"
    );
}

#[test]
fn scroll_clamps_into_content_range() {
    assert_eq!(clamp_scroll(-50.0, 800.0, 300.0), 0.0);
    assert_eq!(clamp_scroll(250.0, 800.0, 300.0), 250.0);
    assert_eq!(clamp_scroll(999.0, 800.0, 300.0), 500.0);
}

#[test]
fn short_content_never_scrolls() {
    assert_eq!(clamp_scroll(120.0, 200.0, 300.0), 0.0);
    assert_eq!(clamp_scroll(0.0, 300.0, 300.0), 0.0);
}
