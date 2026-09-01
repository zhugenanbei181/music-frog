//! Headless tests for the radio wrapper: official `RadioButton`/`RadioGroup`
//! semantics, one `Checked` per group visual, token ring repaint.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::query::Has;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use bevy::ui::Checked;
use bevy::ui_widgets::{RadioButton, RadioGroup};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::radio::{radio_fill, radio_group_scene, radio_ring, radio_scene};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn radio_group_hosts_exactly_one_checked_button() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let members = vec![
                Box::new(radio_scene("http".to_string(), true, &palette)) as Box<_>,
                Box::new(radio_scene("socks".to_string(), false, &palette)) as Box<_>,
            ];
            commands.spawn_scene(radio_group_scene(members));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut groups = world.query::<&RadioGroup>();
    assert_eq!(
        groups.iter(world).count(),
        1,
        "one official group container"
    );

    let world = app.world_mut();
    let mut buttons = world.query::<(&RadioButton, Has<Checked>)>();
    let rows: Vec<_> = buttons.iter(world).collect();
    assert_eq!(rows.len(), 2, "both members carry the official primitive");
    assert_eq!(
        rows.iter().filter(|(_, checked)| *checked).count(),
        1,
        "exactly one member is checked"
    );
}

#[test]
fn ring_visuals_follow_checked_state_and_tokens() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let members = vec![
                Box::new(radio_scene("http".to_string(), true, &palette)) as Box<_>,
                Box::new(radio_scene("socks".to_string(), false, &palette)) as Box<_>,
            ];
            commands.spawn_scene(radio_group_scene(members));
        },
    );
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut rings = world.query::<(&BackgroundColor, Option<&bevy::ui::BorderColor>)>();
    let mut seen = (false, false);
    for (fill, border) in rings.iter(world) {
        if *fill == BackgroundColor(radio_fill(true, &palette)) {
            seen.0 = true;
            assert_eq!(
                border.expect("checked ring has an outline").top,
                radio_ring(true, &palette)
            );
        }
        if *fill == BackgroundColor(radio_fill(false, &palette)) {
            seen.1 = true;
            assert_eq!(
                border.expect("idle ring has an outline").top,
                radio_ring(false, &palette)
            );
        }
    }
    assert!(seen.0 && seen.1, "both ring states painted from tokens");

    let light = UiPalette::new(&Theme::light());
    assert_ne!(radio_fill(false, &palette), radio_fill(false, &light));
}
