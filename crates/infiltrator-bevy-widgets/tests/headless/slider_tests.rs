//! Headless tests for the slider wrapper: official `Slider` state on the
//! root, token track/fill/thumb chrome, and the stylist system that
//! re-projects geometry when `SliderValue` changes.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::prelude::{Node, Val};
use bevy::ui_widgets::{Slider, SliderThumb, SliderValue};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::slider::{SliderFill, slider_fraction, slider_scene};
use infiltrator_bevy_widgets::theme::Theme;

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn slider_spawns_official_state_with_token_chrome() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(slider_scene(25.0, 0.0, 100.0, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut sliders = world.query::<(&Slider, &SliderValue)>();
    let (_, value) = sliders.iter(world).next().expect("one slider");
    assert_eq!(value.0, 25.0);

    let world = app.world_mut();
    let mut thumbs = world.query::<&SliderThumb>();
    assert_eq!(thumbs.iter(world).count(), 1, "one marked thumb");

    let world = app.world_mut();
    let mut fills = world.query::<&SliderFill>();
    assert_eq!(fills.iter(world).count(), 1, "one marked fill bar");
}

#[test]
fn value_change_reprojects_fill_and_thumb_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(slider_scene(25.0, 0.0, 100.0, &palette));
        },
    );
    app.update();

    let slider = {
        let world = app.world_mut();
        let mut sliders = world.query::<(Entity, &Slider)>();
        sliders.iter(world).next().expect("one slider").0
    };

    {
        let world = app.world_mut();
        world.entity_mut(slider).insert(SliderValue(75.0));
    }
    app.update();

    let world = app.world_mut();
    let mut groups = world.query::<&Children>();
    let root_children: Vec<Entity> = groups
        .get(world, slider)
        .expect("slider keeps children")
        .iter()
        .copied()
        .collect();
    let mut parts = root_children.clone();
    for child in root_children {
        if let Ok(nested) = groups.get(world, child) {
            parts.extend(nested.iter().copied());
        }
    }
    let mut fills = world.query::<(&SliderFill, &Node)>();
    let mut thumbs = world.query::<(&SliderThumb, &Node)>();
    let mut saw_fill = false;
    let mut saw_thumb = false;
    for part in parts {
        if let Ok((_, node)) = fills.get(world, part) {
            saw_fill = true;
            assert_eq!(node.width, Val::Percent(75.0), "fill width follows value");
        }
        if let Ok((_, node)) = thumbs.get(world, part) {
            saw_thumb = true;
            assert_eq!(node.left, Val::Percent(75.0), "thumb travel follows value");
        }
    }
    assert!(saw_fill && saw_thumb, "both value-painted parts restamped");
}

#[test]
fn fraction_clamps_and_survives_degenerate_ranges() {
    assert_eq!(slider_fraction(-5.0, 0.0, 100.0), 0.0);
    assert_eq!(slider_fraction(25.0, 0.0, 100.0), 0.25);
    assert_eq!(slider_fraction(150.0, 0.0, 100.0), 1.0);
    assert_eq!(slider_fraction(10.0, 10.0, 10.0), 0.5);
}
