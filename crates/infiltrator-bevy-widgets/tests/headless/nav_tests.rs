//! Headless tests for the nav item: active/inactive token layers at spawn,
//! in-place repaint on an active flip, and palette-driven repaint on a
//! theme flip — entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::ecs::world::World;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, nav_fill, nav_item_scene, nav_label_ink};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

fn spawn_items(app: &mut App) {
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(nav_item_scene("active".to_string(), true, &palette));
            commands.spawn_scene(nav_item_scene("idle".to_string(), false, &palette));
        },
    );
}

/// Entity of the one item whose [`NavActive`] bit equals `active`.
fn item_id(world: &mut World, active: bool) -> Entity {
    let mut items = world.query::<(Entity, &NavActive)>();
    items
        .iter(world)
        .find(|(_, bit)| bit.0 == active)
        .expect("nav item with the wanted bit")
        .0
}

/// (fill, label ink) of one item, joined through its children.
fn item_visuals(world: &mut World, active: bool) -> (BackgroundColor, TextColor) {
    let id = item_id(world, active);
    let fill = *world.get::<BackgroundColor>(id).expect("item fill");
    let label = {
        let children = world.get::<Children>(id).expect("item children");
        *children.iter().next().expect("label is the first child")
    };
    let ink = *world.get::<TextColor>(label).expect("label ink");
    (fill, ink)
}

#[test]
fn active_and_idle_items_spawn_on_their_token_layers() {
    let mut app = headless_app();
    spawn_items(&mut app);
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let (active_fill, active_ink) = item_visuals(world, true);
    assert_eq!(active_fill, BackgroundColor(nav_fill(true, &palette)));
    assert_eq!(active_fill, BackgroundColor(palette.accent));
    assert_eq!(active_ink.0, nav_label_ink(true, &palette));
    assert_eq!(active_ink.0, palette.on_accent);

    let (idle_fill, idle_ink) = item_visuals(world, false);
    assert_eq!(idle_fill, BackgroundColor(nav_fill(false, &palette)));
    assert_eq!(idle_fill, BackgroundColor(palette.surface_elevated));
    assert_eq!(idle_ink.0, nav_label_ink(false, &palette));
    assert_eq!(idle_ink.0, palette.ink);
}

#[test]
fn active_flip_repaints_in_place() {
    let mut app = headless_app();
    spawn_items(&mut app);
    app.update();

    let idle = item_id(app.world_mut(), false);
    app.world_mut().entity_mut(idle).insert(NavActive(true));
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    assert!(world.get::<NavItem>(idle).is_some(), "flip never remounts");
    let fill = *world.get::<BackgroundColor>(idle).expect("fill survives");
    let label = {
        let children = world.get::<Children>(idle).expect("children survive");
        *children.iter().next().expect("label survives")
    };
    let ink = world.get::<TextColor>(label).expect("ink survives");
    assert_eq!(fill, BackgroundColor(nav_fill(true, &palette)));
    assert_eq!(ink.0, nav_label_ink(true, &palette));
}

#[test]
fn theme_flip_repaints_items_without_respawn() {
    let mut app = headless_app();
    spawn_items(&mut app);
    app.update();

    let active = item_id(app.world_mut(), true);
    let idle = item_id(app.world_mut(), false);

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let (active_fill, active_ink) = item_visuals(world, true);
    assert_eq!(active_fill, BackgroundColor(light.accent));
    assert_eq!(active_ink.0, light.on_accent);
    let (idle_fill, idle_ink) = item_visuals(world, false);
    assert_eq!(idle_fill, BackgroundColor(light.surface_elevated));
    assert_eq!(idle_ink.0, light.ink);

    for id in [active, idle] {
        assert!(
            world.get::<NavItem>(id).is_some(),
            "nav item kept its id across the flip"
        );
    }
}
