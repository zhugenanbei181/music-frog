//! Headless tests for the icon seam: the id → plate path table, the
//! AssetServer-backed handle store, and the observer that stamps tinted
//! bitmap nodes. Icons never ride text codepoints, so a missing plate must
//! degrade to an invisible square.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::{AssetApp, AssetPlugin, Handle};
use bevy::ecs::system::{Commands, Res};
use bevy::image::Image;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::icon::{IconId, IconPlate, IconSources, icon_path, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    // The image store a render-backed host contributes (bevy's ImagePlugin
    // registers it); the plate handles need it to exist.
    app.init_asset::<Image>();
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn every_id_maps_to_a_distinct_plate_path() {
    let paths: Vec<_> = IconId::ALL.iter().map(|icon| icon_path(*icon)).collect();
    assert_eq!(paths.len(), 10);
    assert_eq!(
        paths.iter().collect::<std::collections::HashSet<_>>().len(),
        10,
        "no two ids share a plate"
    );
    for path in paths {
        assert!(path.starts_with("icons/"));
        assert!(path.ends_with(".png"));
    }
}

/// The traffic chips' arrow ids keep their semantic plates: upload draws
/// the up arrow, download the down arrow — never the generic Plus /
/// FileText stand-ins the first capture shipped.
#[test]
fn arrow_ids_map_to_their_own_arrow_plates() {
    assert_eq!(icon_path(IconId::ArrowUp), "icons/arrow-up.png");
    assert_eq!(icon_path(IconId::ArrowDown), "icons/arrow-down.png");
    assert_eq!(IconId::ArrowUp.index(), 8, "appended after the M1 set");
    assert_eq!(IconId::ArrowDown.index(), 9);
    let mut app = headless_app();
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<IconSources>();
    for icon in [IconId::ArrowUp, IconId::ArrowDown] {
        let handle = sources.handle(icon).expect("arrow plate registered");
        assert!(handle.is_strong());
        assert_eq!(
            handle.path().map(|path| path.to_string()),
            Some(icon_path(icon).to_string()),
            "the arrow handle carries its own plate path"
        );
    }
}

#[test]
fn store_loads_every_plate_through_the_asset_server() {
    let mut app = headless_app();
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<IconSources>();
    for icon in IconId::ALL {
        let handle = sources.handle(icon).expect("every plate registered");
        assert!(handle.is_strong());
    }

    let world = app.world_mut();
    let sources = world.resource::<IconSources>();
    let settings = sources.handle(IconId::Settings).expect("settings plate");
    assert_eq!(
        settings.path().map(|path| path.to_string()),
        Some("icons/settings.png".to_string()),
        "the handle carries the plate's asset path"
    );
}

#[test]
fn plate_landing_stamps_a_tinted_image_node() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(icon_scene(IconId::Zap, 24.0, palette.ink_dim));
        },
    );
    app.update();

    let world = app.world_mut();
    let tint = world.resource::<UiPalette>().ink_dim;
    let mut plates = world.query::<(&IconPlate, &ImageNode)>();
    let (plate, node) = plates.iter(world).next().expect("stamped icon node");
    assert_eq!(plate.0, IconId::Zap);
    assert_eq!(node.color, tint, "the tint rides the image widget");
    assert_ne!(
        node.image,
        Handle::default(),
        "the registered plate resolved to a real handle"
    );
}

#[test]
fn unregistered_store_degrades_to_transparent_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    // No host AssetServer → the plugin installs the empty store.
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(icon_scene(IconId::Plus, 16.0, bevy::color::Color::WHITE));
    });
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<IconSources>();
    assert!(
        sources.handle(IconId::Plus).is_none(),
        "without an asset server no plate is registered"
    );
    let mut nodes = world.query::<&ImageNode>();
    let node = nodes.iter(world).next().expect("node still stamped");
    assert_eq!(
        node.image,
        Handle::default(),
        "missing plate degrades to the transparent default image"
    );
}
