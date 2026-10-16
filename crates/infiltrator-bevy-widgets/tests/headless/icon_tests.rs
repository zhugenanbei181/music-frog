//! Headless tests for the shared icon catalog, embedded Bevy image store, and
//! observer that stamps tinted bitmap nodes. Missing plates stay transparent.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::{AssetApp, AssetPlugin, Assets, Handle};
use bevy::color::Color;
use bevy::ecs::system::{Commands, Res};
use bevy::image::Image;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::widget::ImageNode;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::icon::{IconId, IconPlate, IconSources, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_shared::assets::{AssetCatalog, AssetFormat};
use std::collections::HashSet;

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
fn every_id_resolves_to_a_distinct_shared_png_asset() {
    let records: Vec<_> = IconId::ALL
        .iter()
        .map(|icon| {
            AssetCatalog::find(
                &format!("icon.{}", icon.asset_name()),
                AssetFormat::Png,
                "white-plate-64",
            )
            .expect("every Bevy icon is registered in the shared catalog")
        })
        .collect();
    assert_eq!(records.len(), 10);
    assert_eq!(
        records
            .iter()
            .map(|record| &record.path)
            .collect::<HashSet<_>>()
            .len(),
        10,
        "no two ids share a plate file"
    );
    for record in records {
        assert!(record.path.starts_with("ui/bevy/png/"));
        assert_eq!(record.format, "png");
    }
}

/// The traffic chips' arrow ids keep their semantic plates: upload draws
/// the up arrow, download the down arrow — never the generic Plus /
/// FileText stand-ins the first capture shipped.
#[test]
fn arrow_ids_map_to_their_own_arrow_plates() {
    assert_eq!(IconId::ArrowUp.asset_name(), "arrow-up");
    assert_eq!(IconId::ArrowDown.asset_name(), "arrow-down");
    assert_eq!(IconId::ArrowUp.index(), 8, "appended after the M1 set");
    assert_eq!(IconId::ArrowDown.index(), 9);
    let mut app = headless_app();
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<IconSources>();
    for icon in [IconId::ArrowUp, IconId::ArrowDown] {
        let handle = sources.handle(icon).expect("arrow plate registered");
        assert!(handle.is_strong());
    }
    assert_eq!(world.resource::<Assets<Image>>().len(), IconId::ALL.len());
}

#[test]
fn store_decodes_every_canonical_plate_into_bevy_images() {
    let mut app = headless_app();
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<IconSources>();
    for icon in IconId::ALL {
        let handle = sources.handle(icon).expect("every plate registered");
        assert!(handle.is_strong());
    }

    assert_eq!(world.resource::<Assets<Image>>().len(), IconId::ALL.len());
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
fn icon_initialization_preserves_existing_entities_and_tints() {
    let mut app = headless_app();
    let tint = Color::srgb(0.25, 0.5, 0.75);
    let plate = app
        .world_mut()
        .commands()
        .spawn_scene(icon_scene(IconId::ArrowDown, 20.0, tint))
        .id();
    app.update();
    let expected = app
        .world()
        .resource::<IconSources>()
        .handle(IconId::ArrowDown)
        .unwrap();
    let node = app.world().get::<ImageNode>(plate).unwrap();
    assert_eq!(node.image, expected);
    assert_eq!(node.color, tint);
    app.update();
    assert_eq!(app.world().get::<ImageNode>(plate).unwrap().image, expected);
}

#[test]
fn unregistered_store_degrades_to_transparent_without_panic() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    // No host AssetServer → the plugin installs the empty store.
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn_scene(icon_scene(IconId::Plus, 16.0, Color::WHITE));
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
