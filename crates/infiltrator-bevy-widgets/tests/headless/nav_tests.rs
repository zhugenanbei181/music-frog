//! Headless tests for the nav item: active/inactive token layers at spawn,
//! in-place repaint on an active flip, and palette-driven repaint on a
//! theme flip — entity ids never change.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::ecs::world::World;
use bevy::feathers::theme::{
    SurfaceLevel, ThemeBackgroundColor, ThemeTextColor, ThemeToken, UiTheme,
};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::nav::{
    NavActive, NavItem, nav_fill_token, nav_item_scene, nav_label_ink_token,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::theme::{Theme, ThemeSkin};

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

fn item_theme_tokens(world: &mut World, active: bool) -> (ThemeToken, ThemeToken) {
    let id = item_id(world, active);
    let fill = world
        .get::<ThemeBackgroundColor>(id)
        .expect("item fill token")
        .0
        .clone();
    let label = {
        let children = world.get::<Children>(id).expect("item children");
        *children.iter().next().expect("label is the first child")
    };
    let ink = world
        .get::<ThemeTextColor>(label)
        .expect("label ink token")
        .0
        .clone();
    (fill, ink)
}

fn expected_color(world: &World, token: Token) -> Color {
    world
        .resource::<UiTheme>()
        .context_color(&token.theme_token(), SurfaceLevel::Base)
}

#[test]
fn active_and_idle_items_spawn_on_their_token_layers() {
    let mut app = headless_app();
    spawn_items(&mut app);
    app.update();

    let expected_active_fill = expected_color(app.world(), Token::Accent);
    let expected_active_ink = expected_color(app.world(), Token::OnAccent);
    let expected_idle_fill = expected_color(app.world(), Token::SurfaceElevated);
    let expected_idle_ink = expected_color(app.world(), Token::Ink);
    let world = app.world_mut();
    let (active_fill, active_ink) = item_visuals(world, true);
    assert_eq!(active_fill.0, expected_active_fill);
    assert_eq!(active_ink.0, expected_active_ink);
    let (active_fill_token, active_ink_token) = item_theme_tokens(world, true);
    assert_eq!(active_fill_token, nav_fill_token(true).theme_token());
    assert_eq!(active_ink_token, nav_label_ink_token(true).theme_token());

    let (idle_fill, idle_ink) = item_visuals(world, false);
    assert_eq!(idle_fill.0, expected_idle_fill);
    assert_eq!(idle_ink.0, expected_idle_ink);
    let (idle_fill_token, idle_ink_token) = item_theme_tokens(world, false);
    assert_eq!(idle_fill_token, nav_fill_token(false).theme_token());
    assert_eq!(idle_ink_token, nav_label_ink_token(false).theme_token());
}

#[test]
fn active_flip_repaints_in_place() {
    let mut app = headless_app();
    spawn_items(&mut app);
    app.update();

    let idle = item_id(app.world_mut(), false);
    app.world_mut().entity_mut(idle).insert(NavActive(true));
    app.update();

    let expected_fill = expected_color(app.world(), Token::Accent);
    let expected_ink = expected_color(app.world(), Token::OnAccent);
    let world = app.world_mut();
    assert!(world.get::<NavItem>(idle).is_some(), "flip never remounts");
    let fill = *world.get::<BackgroundColor>(idle).expect("fill survives");
    let label = {
        let children = world.get::<Children>(idle).expect("children survive");
        *children.iter().next().expect("label survives")
    };
    let ink = world.get::<TextColor>(label).expect("ink survives");
    assert_eq!(fill.0, expected_fill);
    assert_eq!(ink.0, expected_ink);
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
        .trigger(ThemeSwitch(ThemeSkin::Light));
    app.update();

    let expected_active_fill = expected_color(app.world(), Token::Accent);
    let expected_active_ink = expected_color(app.world(), Token::OnAccent);
    let expected_idle_fill = expected_color(app.world(), Token::SurfaceElevated);
    let expected_idle_ink = expected_color(app.world(), Token::Ink);
    let world = app.world_mut();
    let (active_fill, active_ink) = item_visuals(world, true);
    assert_eq!(active_fill.0, expected_active_fill);
    assert_eq!(active_ink.0, expected_active_ink);
    let (idle_fill, idle_ink) = item_visuals(world, false);
    assert_eq!(idle_fill.0, expected_idle_fill);
    assert_eq!(idle_ink.0, expected_idle_ink);

    for id in [active, idle] {
        assert!(
            world.get::<NavItem>(id).is_some(),
            "nav item kept its id across the flip"
        );
    }
}
