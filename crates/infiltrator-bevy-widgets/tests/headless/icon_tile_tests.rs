//! Headless tests for the icon tile: token fill + accent-tinted icon at
//! spawn, and in-place repaint when the palette changes (theme flip keeps
//! every entity id).

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::{
    IconTile, icon_tile_fill, icon_tile_scene, icon_tile_tint,
};
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

fn spawn_tile(app: &mut App) {
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(icon_tile_scene(IconId::Activity, 40.0, &palette));
        },
    );
}

#[test]
fn tile_spawns_with_token_fill_and_accent_tint() {
    let mut app = headless_app();
    spawn_tile(&mut app);
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut fills = world.query_filtered::<&BackgroundColor, With<IconTile>>();
    let fill = *fills.iter(world).next().expect("tile fill mounted");
    assert_eq!(fill, BackgroundColor(icon_tile_fill(&palette)));
    assert_eq!(fill, BackgroundColor(palette.icon_tile));
}

#[test]
fn tile_tint_and_image_follow_the_palette() {
    let mut app = headless_app();
    spawn_tile(&mut app);
    app.update();

    let dark = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut tints = world.query::<&infiltrator_bevy_widgets::icon::IconTint>();
    let tint = tints.iter(world).next().expect("icon tint mounted");
    assert_eq!(tint.0, icon_tile_tint(&dark), "dark spawn tint is accent");

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let mut tints = world.query::<&infiltrator_bevy_widgets::icon::IconTint>();
    let tint = *tints.iter(world).next().expect("tint survives the switch");
    assert_eq!(tint.0, icon_tile_tint(&light), "tint restamped to light");

    let mut images = world.query::<&ImageNode>();
    let image = images.iter(world).next().expect("plate node mounted");
    assert_eq!(image.color, icon_tile_tint(&light), "image mirrors tint");
}

#[test]
fn tile_fill_flips_with_the_theme_without_respawn() {
    let mut app = headless_app();
    spawn_tile(&mut app);
    app.update();

    let dark = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut tiles =
        world.query_filtered::<(bevy::ecs::entity::Entity, &BackgroundColor), With<IconTile>>();
    let (id, dark_fill) = tiles.iter(world).next().expect("tile mounted");
    assert_eq!(*dark_fill, BackgroundColor(dark.icon_tile));

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let fill = world.get::<BackgroundColor>(id).expect("id unchanged");
    assert_eq!(*fill, BackgroundColor(light.icon_tile), "fill flipped");
    assert!(
        world
            .get::<infiltrator_bevy_widgets::icon_tile::IconTile>(id)
            .is_some(),
        "same entity, restamped in place"
    );
}
