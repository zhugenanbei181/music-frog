//! Headless tests for the embedded font sources: registration through the
//! `Assets<Font>` store, the role → face table, and the graceful fallback
//! when faces are unregistered.

use bevy::MinimalPlugins;
use bevy::app::{App, PreStartup, Startup};
use bevy::asset::{AssetPlugin, Assets, Handle};
use bevy::ecs::system::{Commands, Res};
use bevy::render::RenderPlugin;
use bevy::render::settings::WgpuSettings;
use bevy::scene::{CommandsSceneExt, ScenePlugin, bsn};
use bevy::shader::Shader;
use bevy::text::{
    Font, FontSize, FontSource, Justify, RemSize, TextColor, TextFont, TextLayout, TextPlugin,
};
use bevy::ui::prelude::{FlexDirection, Node};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::bidi::LayoutDirection;
use infiltrator_bevy_widgets::button::pill_scene;
use infiltrator_bevy_widgets::fonts::FontSources;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole, role_typography};
use infiltrator_bevy_widgets::text_runs::{TextRuns, TextRunsDirection};
use infiltrator_bevy_widgets::theme::REM_BASE;
use infiltrator_bevy_widgets::theme::Theme;
use zhuge_ui_core::text::SearchTextRun;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin, TextPlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn widgets_keep_the_render_hosts_existing_shader_handles() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        TextPlugin,
    ));
    app.add_plugins(RenderPlugin {
        render_creation: WgpuSettings {
            backends: None,
            ..Default::default()
        }
        .into(),
        ..Default::default()
    });
    let shader = app
        .world_mut()
        .resource_mut::<Assets<Shader>>()
        .add(Shader::from_wgsl("const VALUE: f32 = 1.0;", file!()));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    assert!(
        app.world()
            .resource::<Assets<Shader>>()
            .get(&shader)
            .is_some(),
        "adding widget themes must preserve the render host's store and allocator"
    );
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
    assert!(sources.cjk.is_strong());
    assert_ne!(sources.heading, sources.body, "faces are distinct assets");

    let world = app.world_mut();
    let fonts = world.resource::<Assets<Font>>();
    assert_eq!(
        fonts.len(),
        5,
        "the five faces are the whole embedded store"
    );
}

#[test]
fn font_initialization_precedes_startup_and_preserves_preexisting_labels() {
    let mut app = headless_app();
    let label = app
        .world_mut()
        .commands()
        .spawn_scene(bsn! { TextRole(Role::Mono) TextFont::default() })
        .id();
    app.add_systems(Startup, |sources: Res<FontSources>| {
        assert!(
            sources.mono.is_strong(),
            "assets must be ready before scenes"
        );
    });
    app.update();
    let source = app.world().resource::<FontSources>().mono.clone();
    assert_eq!(
        app.world().get::<TextFont>(label).unwrap().font,
        FontSource::Handle(source)
    );
    app.update();
    assert_eq!(app.world().resource::<Assets<Font>>().len(), 5);
    assert!(app.world().get_entity(label).is_ok());
}

#[test]
fn widgets_plugin_requires_asset_plugin_before_it() {
    // WidgetsPlugin installs FeathersCorePlugin, whose embedded .wesl assets
    // need the embedded registry at build time: AssetPlugin must precede it
    // (capability norm ② host requirement).
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin, TextPlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app.add_systems(PreStartup, |mut commands: Commands| {
        commands.spawn_scene(bsn! { TextRole(Role::Body) TextFont::default() });
    });
    app.update();
    assert!(app.world().resource::<Assets<Font>>().len() >= 5);
    assert!(
        app.world().contains_resource::<Assets<Shader>>(),
        "feather embedded-shader asset store registered"
    );
    assert!(app.world().resource::<FontSources>().body.is_strong());
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
    assert!(matches!(display.size, FontSize::Rem(f) if (f * REM_BASE - 22.0).abs() < 1e-4));
    assert!(matches!(heading.size, FontSize::Rem(f) if (f * REM_BASE - 20.0).abs() < 1e-4));
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
    assert!(matches!(typography.size, FontSize::Rem(f) if (f * REM_BASE - 15.0).abs() < 1e-4));
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

#[test]
fn text_runs_direction_propagates_to_layout() {
    let mut app = headless_app();
    let rtl_entity = app
        .world_mut()
        .spawn((
            TextRuns(vec![SearchTextRun {
                text: "مرحبا".to_string(),
                highlighted: false,
            }]),
            TextRunsDirection(LayoutDirection::Rtl),
            Node {
                flex_direction: FlexDirection::Row,
                ..Default::default()
            },
            TextLayout {
                justify: Justify::Left,
                ..Default::default()
            },
        ))
        .id();
    let ltr_entity = app
        .world_mut()
        .spawn((
            TextRuns(vec![SearchTextRun {
                text: "hello".to_string(),
                highlighted: false,
            }]),
            TextRunsDirection(LayoutDirection::Ltr),
            Node {
                flex_direction: FlexDirection::Row,
                ..Default::default()
            },
            TextLayout {
                justify: Justify::Left,
                ..Default::default()
            },
        ))
        .id();
    app.update();

    let world = app.world();
    let rtl_node = world.get::<Node>(rtl_entity).expect("rtl node");
    let rtl_layout = world.get::<TextLayout>(rtl_entity).expect("rtl layout");
    assert_eq!(rtl_node.flex_direction, FlexDirection::RowReverse);
    assert_eq!(rtl_layout.justify, Justify::Right);

    let ltr_node = world.get::<Node>(ltr_entity).expect("ltr node");
    let ltr_layout = world.get::<TextLayout>(ltr_entity).expect("ltr layout");
    assert_eq!(ltr_node.flex_direction, FlexDirection::Row);
    assert_eq!(ltr_layout.justify, Justify::Left);
}

/// Capability norm (BEVY_020_UI_CAPABILITIES ①): the shell installs the
/// root font size, roles are stated rem-relative, and rescaling `RemSize`
/// is the only knob — nothing else may touch type sizes.
#[test]
fn root_rem_size_is_installed_and_roles_track_it() {
    let mut app = headless_app();
    app.update();
    let rem = app.world().resource::<RemSize>();
    assert_eq!(rem.0, REM_BASE, "WidgetsPlugin installs the root font size");

    let palette = UiPalette::new(&Theme::dark());
    let FontSize::Rem(body) = role_typography(Role::Body, &palette, None).size else {
        panic!("body must ride the rem channel");
    };
    let FontSize::Rem(display) = role_typography(Role::Display, &palette, None).size else {
        panic!("display must ride the rem channel");
    };
    assert!((body * REM_BASE - 15.0).abs() < 1e-4);
    assert!((display * REM_BASE - 22.0).abs() < 1e-4);
}
