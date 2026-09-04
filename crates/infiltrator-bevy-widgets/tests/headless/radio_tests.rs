//! Headless tests for the radio wrapper: official `RadioButton` / `RadioGroup`
//! semantics, token ring visuals driven by the official `Checked` marker,
//! keyboard navigation state machine and event flow.

use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::ui::BackgroundColor;
use bevy::ui::Checked;
use bevy::ui_widgets::RadioGroup;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::radio::{
    RadioGroupNavEvent, RadioGroupState, RadioNavAction, RadioRing, indexed_radio_group_scene,
    navigate_radio_group, radio_fill, radio_group_scene, radio_ring, radio_scene,
};
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
            commands.spawn_scene(radio_group_scene(vec![
                radio_scene("rule".to_string(), true, &palette),
                radio_scene("global".to_string(), false, &palette),
                radio_scene("direct".to_string(), false, &palette),
            ]));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut groups = world.query::<(&RadioGroup, &Children)>();
    let (_, children) = groups.iter(world).next().expect("one radio group");
    assert_eq!(children.len(), 3, "three options mounted under the group");

    let checked_count = children
        .iter()
        .filter(|&&c| world.get::<Checked>(c).is_some())
        .count();
    assert_eq!(checked_count, 1, "exactly the requested row starts checked");
}

#[test]
fn ring_visuals_follow_checked_state_and_tokens() {
    let palette = UiPalette::new(&Theme::dark());
    assert_eq!(radio_fill(true, &palette), palette.accent);
    assert_eq!(radio_fill(false, &palette), palette.surface_elevated);
    assert_eq!(radio_ring(true, &palette), palette.accent);
    assert_eq!(radio_ring(false, &palette), palette.border);

    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(radio_scene("tun".to_string(), true, &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut rings = world.query::<(&RadioRing, &BackgroundColor)>();
    let (_, fill) = rings.iter(world).next().expect("ring mounted");
    assert_eq!(*fill, BackgroundColor(palette.accent));
}

#[test]
fn radio_keyboard_navigation_state_machine() {
    // 3 options: [0, 1, 2]
    assert_eq!(
        navigate_radio_group(Some(0), 3, RadioNavAction::Next, true),
        1
    );
    assert_eq!(
        navigate_radio_group(Some(2), 3, RadioNavAction::Next, true),
        0
    );
    assert_eq!(
        navigate_radio_group(Some(2), 3, RadioNavAction::Next, false),
        2
    );

    assert_eq!(
        navigate_radio_group(Some(1), 3, RadioNavAction::Previous, true),
        0
    );
    assert_eq!(
        navigate_radio_group(Some(0), 3, RadioNavAction::Previous, true),
        2
    );

    assert_eq!(
        navigate_radio_group(Some(1), 3, RadioNavAction::First, true),
        0
    );
    assert_eq!(
        navigate_radio_group(Some(1), 3, RadioNavAction::Last, true),
        2
    );
    assert_eq!(
        navigate_radio_group(Some(1), 3, RadioNavAction::SelectIndex(2), true),
        2
    );
}

#[test]
fn radio_group_ecs_keyboard_event_flow() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let options = vec!["Direct".to_owned(), "Rule".to_owned(), "Global".to_owned()];
            commands.spawn_scene(indexed_radio_group_scene(options, Some(0), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut groups = world.query::<(Entity, &RadioGroupState, &Children)>();
    let (group_entity, state, children) = groups.iter(world).next().expect("group mounted");
    assert_eq!(state.active_index, Some(0));

    let direct_btn = children[0];
    let rule_btn = children[1];
    assert!(world.get::<Checked>(direct_btn).is_some());
    assert!(world.get::<Checked>(rule_btn).is_none());

    // Send Next navigation message
    app.world_mut().write_message(RadioGroupNavEvent {
        group: group_entity,
        action: RadioNavAction::Next,
    });
    app.update();

    let world = app.world();
    let state_after = world
        .get::<RadioGroupState>(group_entity)
        .expect("state exists");
    assert_eq!(state_after.active_index, Some(1));
    assert!(world.get::<Checked>(direct_btn).is_none());
    assert!(world.get::<Checked>(rule_btn).is_some());
}
