//! Headless tests for Modal / Dialog: card geometry, alert presets,
//! and theme reskin without recreation.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::BackgroundColor;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::modal::{ModalDialogCard, ModalScrim, confirm_dialog_scene};
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

#[test]
fn modal_state_and_dialog_scenes_mount() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(confirm_dialog_scene(
                "Delete Profile".to_owned(),
                "Are you sure you want to delete this configuration?".to_owned(),
                "Delete".to_owned(),
                "Cancel".to_owned(),
                true,
                &palette,
            ));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut scrims = world.query::<(&ModalScrim, &BackgroundColor)>();
    assert!(scrims.iter(world).next().is_some());

    let mut cards = world.query::<(&ModalDialogCard, &BackgroundColor)>();
    let (_, bg) = cards.iter(world).next().expect("modal card mounted");
    let dark_palette = UiPalette::new(&Theme::dark());
    assert_eq!(*bg, BackgroundColor(dark_palette.surface));

    // Theme switch
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light_palette = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let mut cards = world.query::<(&ModalDialogCard, &BackgroundColor)>();
    let (_, bg_light) = cards.iter(world).next().expect("modal card survives");
    assert_eq!(*bg_light, BackgroundColor(light_palette.surface));
}
