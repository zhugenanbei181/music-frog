//! Headless tests for the embedded font sources: registration through the
//! `Assets<Font>` store, the role → face table, and the graceful fallback
//! when faces are unregistered.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::{AssetPlugin, Assets, Handle};
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::{Font, FontSize, FontSource, TextColor, TextFont, TextPlugin};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::button::pill_scene;
use infiltrator_bevy_widgets::fonts::FontSources;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, role_typography};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin, TextPlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn embedded_faces_register_exactly_four_font_assets() {
    let mut app = headless_app();
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<FontSources>();
    assert!(sources.heading.is_strong());
    assert!(sources.body.is_strong());
    assert!(sources.caption.is_strong());
    assert!(sources.mono.is_strong());
    assert_ne!(sources.heading, sources.body, "faces are distinct assets");

    let world = app.world_mut();
    let fonts = world.resource::<Assets<Font>>();
    assert_eq!(
        fonts.len(),
        4,
        "the four OFL faces are the whole embedded store"
    );
}

#[test]
fn faces_map_per_role() {
    let mut app = headless_app();
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<FontSources>();
    let table = [
        (Role::Display, sources.heading.clone()),
        (Role::Heading, sources.heading.clone()),
        (Role::Body, sources.body.clone()),
        (Role::Caption, sources.caption.clone()),
        (Role::Mono, sources.mono.clone()),
    ];
    for (role, face) in table {
        assert_eq!(sources.face(role), face, "role resolves its own face");
        assert!(face.is_strong());
    }
}

/// The display rung: banner-state size (22px, one step over the 20px
/// page-heading rung), full ink, SemiBold — the iced reference's banner
/// word, without moving the global heading scale.
#[test]
fn display_role_sits_one_step_over_heading() {
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(palette.display_font_px, 22.0);
    assert_eq!(palette.heading_font_px, 20.0);

    let display = role_typography(Role::Display, &palette, None);
    let heading = role_typography(Role::Heading, &palette, None);
    assert!(matches!(display.size, FontSize::Px(size) if size == 22.0));
    assert!(matches!(heading.size, FontSize::Px(size) if size == 20.0));
    assert_eq!(display.ink, palette.ink, "display draws at full ink");
    assert_eq!(
        display.font, heading.font,
        "display borrows the SemiBold face"
    );
}

#[test]
fn unregistered_sources_fall_back_to_default_handles() {
    let sources = FontSources::default();
    for role in [
        Role::Display,
        Role::Heading,
        Role::Body,
        Role::Caption,
        Role::Mono,
    ] {
        assert_eq!(sources.face(role), Handle::default());
    }
    let palette = UiPalette::new(&Theme::dark());
    let typography = role_typography(Role::Body, &palette, None);
    assert_eq!(typography.font, FontSource::default());
    assert!(matches!(typography.size, FontSize::Px(size) if size == 15.0));
}

#[test]
fn spawned_label_stamps_the_embedded_face() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(pill_scene("proxy".to_string(), false, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let sources = world.resource::<FontSources>();
    let body = sources.body.clone();
    let world = app.world_mut();
    let mut labels = world.query::<(&TextFont, &TextColor)>();
    let (font, ink) = labels.iter(world).next().expect("one stamped label");
    assert_eq!(
        font.font,
        FontSource::Handle(body),
        "body role stamps the Regular face"
    );
    assert_eq!(ink.0, UiPalette::new(&Theme::dark()).ink);
}
