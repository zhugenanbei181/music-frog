//! Headless tests for the stat chip: icon tile + label + marked value at
//! spawn, and in-place fill repaint when the palette changes (theme flip
//! keeps every entity id).

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::stat_chip::{
    StatChip, StatChipValue, stat_chip_fill, stat_chip_scene,
};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

fn spawn_chip(app: &mut App) {
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(stat_chip_scene(
                IconId::Activity,
                "connections".to_string(),
                "12".to_string(),
                &palette,
            ));
        },
    );
}

#[test]
fn chip_spawns_tile_label_and_marked_value() {
    let mut app = headless_app();
    spawn_chip(&mut app);
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut chips = world.query::<(&StatChip, &BackgroundColor)>();
    let (_, fill) = chips.iter(world).next().expect("chip fill mounted");
    assert_eq!(*fill, BackgroundColor(stat_chip_fill(&palette)));
    assert_eq!(*fill, BackgroundColor(palette.surface));

    let world = app.world_mut();
    let mut labels = world.query::<(&Text, &TextRole)>();
    let mut saw_caption = false;
    let mut saw_mono_value = false;
    for (text, role) in labels.iter(world) {
        match role.0 {
            Role::Caption => {
                saw_caption = true;
                assert_eq!(text.0, "connections");
            }
            Role::Mono => {
                saw_mono_value = true;
                assert_eq!(text.0, "12");
            }
            _ => {}
        }
    }
    assert!(saw_caption, "caption label mounted");
    assert!(saw_mono_value, "mono value mounted");

    let world = app.world_mut();
    let mut values = world.query::<&StatChipValue>();
    assert_eq!(values.iter(world).count(), 1, "exactly one marked value");
}

#[test]
fn chip_fill_flips_with_the_theme_without_respawn() {
    let mut app = headless_app();
    spawn_chip(&mut app);
    app.update();

    let dark = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut chips = world.query::<(bevy::ecs::entity::Entity, &StatChip)>();
    let (chip_root, _) = chips.iter(world).next().expect("chip mounted");
    let world = app.world_mut();
    let mut roots = world.query::<(bevy::ecs::entity::Entity, &bevy::ui::BackgroundColor)>();
    let (chip_root, dark_fill) = roots
        .iter(world)
        .find(|(id, _)| *id == chip_root)
        .expect("chip root fill");
    assert_eq!(*dark_fill, BackgroundColor(dark.surface));

    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let fill = world
        .get::<BackgroundColor>(chip_root)
        .expect("id unchanged");
    assert_eq!(*fill, BackgroundColor(light.surface), "fill flipped");

    let world = app.world_mut();
    let mut values = world.query::<(&StatChipValue, &Text, &TextColor)>();
    let (_, text, _) = values.iter(world).next().expect("value survives");
    assert_eq!(text.0, "12", "value text untouched by the reskin");
}
