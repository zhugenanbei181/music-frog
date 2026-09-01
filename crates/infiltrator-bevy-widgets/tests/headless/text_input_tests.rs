//! Headless tests for the text field: the pure state machine (controlled
//! text, caret, minimal selection, IME preedit), the state → visual
//! projection (runs, selection wash, caret blink), and the scene adapter
//! whose sync systems restamp everything in place.

use std::time::Duration;

use bevy::MinimalPlugins;
use bevy::app::{App, Startup, Update};
use bevy::asset::AssetPlugin;
use bevy::camera::visibility::Visibility;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::time::{Time, Virtual};
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text_input::{
    TextField, TextFieldAfter, TextFieldCaret, TextFieldInput, TextFieldPreeditText,
    TextFieldSelection, TextFieldSelectionText, TextFieldState, field_visual, sync_field_carets,
    text_field_scene,
};
use infiltrator_bevy_widgets::theme::{LightDark, Theme};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn state_machine_edits_text_around_the_caret() {
    let mut field = TextFieldState::new("tcp");
    assert!(field.apply(TextFieldInput::Home));
    assert!(field.apply(TextFieldInput::Insert("udp/".to_string())));
    assert_eq!(field.text(), "udp/tcp");
    assert_eq!(field.cursor(), 4);
    assert!(field.apply(TextFieldInput::Backspace));
    assert_eq!(field.text(), "udptcp");
}

#[test]
fn selection_replaces_and_collapses() {
    let mut field = TextFieldState::new("mixed");
    assert!(field.apply(TextFieldInput::Home));
    assert!(field.apply(TextFieldInput::Right(true)));
    assert_eq!(field.selection(), Some((0, 1)));
    assert!(field.apply(TextFieldInput::Insert("M".to_string())));
    assert_eq!(field.text(), "Mixed");
    assert_eq!(field.selection(), None, "editing collapses the selection");
    assert!(field.apply(TextFieldInput::SelectAll));
    assert_eq!(field.selection(), Some((0, 5)));
    assert!(field.apply(TextFieldInput::Delete));
    assert_eq!(field.text(), "");
}

#[test]
fn multibyte_text_never_splits_a_codepoint() {
    let mut field = TextFieldState::new("青蛙");
    assert!(field.apply(TextFieldInput::Left(false)));
    assert_eq!(field.cursor(), 1, "caret counts chars, not bytes");
    assert!(field.apply(TextFieldInput::Backspace));
    assert_eq!(
        field.text(),
        "蛙",
        "backspace removed the char before the caret"
    );
    assert_eq!(field.cursor(), 0);
}

#[test]
fn field_scene_hosts_controlled_state_and_label() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("proxies".to_string(), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut fields = world.query::<(&TextField, &Children)>();
    let (field, _) = fields.iter(world).next().expect("one field root");
    assert_eq!(field.0.text(), "proxies");

    let world = app.world_mut();
    let mut labels = world.query::<&Text>();
    assert_eq!(
        labels
            .iter(world)
            .filter(|text| text.0 == "proxies")
            .count(),
        1,
        "the field's text node mirrors the initial state"
    );
}

#[test]
fn state_change_restamps_text_node_without_respawn() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("proxies".to_string(), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut fields = world.query::<(Entity, &TextField)>();
    let (field_entity, field) = fields.iter(world).next().expect("one field root");
    let mut state = field.0.clone();
    let _ = state.apply(TextFieldInput::End);
    assert!(state.apply(TextFieldInput::Insert("/group".to_string())));

    let world = app.world_mut();
    world.entity_mut(field_entity).insert(TextField(state));
    app.update();

    let world = app.world_mut();
    let mut labels = world.query::<&Text>();
    assert!(
        labels.iter(world).any(|text| text.0 == "proxies/group"),
        "the sync system mirrored the controlled state"
    );
    let mut field_count = world.query::<&TextField>();
    assert_eq!(
        field_count.iter(world).count(),
        1,
        "the field was restamped, not remounted"
    );
}

#[test]
fn the_field_is_not_a_button_and_keeps_its_box() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("proxies".to_string(), &palette));
        },
    );
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut buttons = world.query::<&Button>();
    assert_eq!(buttons.iter(world).count(), 0, "no button semantics");
    let mut roots = world.query::<(&TextField, &BackgroundColor)>();
    let (_, fill) = roots.iter(world).next().expect("field root keeps its fill");
    assert_eq!(fill.0, palette.surface_elevated);
}

#[test]
fn preedit_rides_the_state_not_the_edit_operations() {
    let mut field = TextFieldState::new("ab");
    assert!(field.set_preedit("混"));
    assert_eq!(field.preedit(), "混");
    assert!(!field.set_preedit("混"), "same preedit is a no-op");
    let _ = field.apply(TextFieldInput::End);
    assert_eq!(
        field.preedit(),
        "混",
        "edits never touch the composition string"
    );
    assert!(field.clear_preedit());
    assert_eq!(field.preedit(), "");
}

#[test]
fn field_visual_splits_runs_and_places_the_caret() {
    // No selection: everything left of the caret is `before`.
    let mut field = TextFieldState::new("proxies");
    let visual = field_visual(&field);
    assert_eq!(visual.before, "proxies");
    assert_eq!(visual.selected, "");
    assert_eq!(visual.after, "");
    assert_eq!(visual.caret_slot, 0);

    // Extend right from Home: selection (0,1), caret sits at its right edge.
    assert!(field.apply(TextFieldInput::Home));
    assert!(field.apply(TextFieldInput::Right(true)));
    let visual = field_visual(&field);
    assert_eq!(visual.before, "");
    assert_eq!(visual.selected, "p");
    assert_eq!(visual.after, "roxies");
    assert_eq!(visual.caret_slot, 1);

    // Extend left from the end: caret at the selection's LEFT edge (slot 0).
    let mut field = TextFieldState::new("proxies");
    let _ = field.apply(TextFieldInput::Left(true));
    let visual = field_visual(&field);
    assert_eq!(visual.before, "proxie");
    assert_eq!(visual.selected, "s");
    assert_eq!(visual.after, "");
    assert_eq!(visual.caret_slot, 0);
}

/// A fast virtual clock: each frame is a blink period and a bit, so the
/// Local clock flips phase every update without real sleeping.
fn advance_blink_clock(mut time: ResMut<Time<Virtual>>) {
    time.advance_by(Duration::from_millis(600));
}

#[test]
fn caret_blink_reaches_both_states_in_place() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("proxies".to_string(), &palette));
        },
    );
    app.add_systems(Update, advance_blink_clock.before(sync_field_carets));
    app.update();

    let world = app.world_mut();
    let mut carets = world.query::<(&TextFieldCaret, Entity)>();
    let mut slots: Vec<(usize, Entity)> = carets
        .iter(world)
        .map(|(caret, entity)| (caret.0, entity))
        .collect();
    slots.sort_unstable();
    assert_eq!(slots.len(), 2, "both caret bars exist");
    assert_eq!(slots[0].0, 0, "slot 0 is the no-selection caret");

    let visibility = |world: &bevy::ecs::world::World, entity: Entity| {
        *world.get::<Visibility>(entity).expect("caret visibility")
    };
    // The active slot (0, the no-selection caret) alternates hidden/visible
    // one period per frame; slot 1 has no cursor and stays hidden. Both
    // blink states are therefore reached, on stable entities.
    let mut slot0_seen = Vec::new();
    for _ in 0..3 {
        let world = app.world_mut();
        slot0_seen.push(visibility(world, slots[0].1));
        assert_eq!(
            visibility(world, slots[1].1),
            Visibility::Hidden,
            "the inactive caret never shows"
        );
        app.update();
    }
    assert!(
        slot0_seen.contains(&Visibility::Hidden) && slot0_seen.contains(&Visibility::Visible),
        "both blink states observed on slot 0: {slot0_seen:?}"
    );
    let world = app.world_mut();
    assert!(
        world.get::<Visibility>(slots[0].1).is_some(),
        "the caret bars kept their entity ids across the blink"
    );
}

#[test]
fn selection_state_injection_paints_the_wash_and_the_runs() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("proxies".to_string(), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut fields = world.query::<(Entity, &TextField)>();
    let (field_entity, field) = fields.iter(world).next().expect("one field root");
    let mut state = field.0.clone();
    assert!(state.apply(TextFieldInput::SelectAll));
    world.entity_mut(field_entity).insert(TextField(state));
    app.update();

    let palette = UiPalette::new(&Theme::dark());
    let world = app.world_mut();
    let mut selected_texts = world.query::<(&TextFieldSelectionText, &Text)>();
    let (_, selected) = selected_texts
        .iter(world)
        .next()
        .expect("selection run mounted");
    assert_eq!(selected.0, "proxies", "the selected run mirrors the text");

    let mut washes = world.query::<(&TextFieldSelection, &BackgroundColor)>();
    let (_, wash) = washes.iter(world).next().expect("selection wash mounted");
    assert_eq!(wash.0, palette.selection_fill());

    let mut afters = world.query::<(&TextFieldAfter, &Text)>();
    let (_, after) = afters.iter(world).next().expect("after run mounted");
    assert_eq!(after.0, "", "nothing right of a select-all");
}

#[test]
fn preedit_state_injection_shows_the_composition_and_underline() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("".to_string(), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut fields = world.query::<(Entity, &TextField)>();
    let (field_entity, field) = fields.iter(world).next().expect("one field root");
    let mut state = field.0.clone();
    assert!(state.set_preedit("混合"));
    world.entity_mut(field_entity).insert(TextField(state));
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<(&TextFieldPreeditText, &Text)>();
    let (_, text) = texts.iter(world).next().expect("preedit run mounted");
    assert_eq!(text.0, "混合", "composition renders as its own run");
}

#[test]
fn theme_flip_rederives_the_wash_and_keeps_the_field() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("proxies".to_string(), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut fields = world.query::<(Entity, &TextField)>();
    let (field_entity, field) = fields.iter(world).next().expect("one field root");
    let mut state = field.0.clone();
    assert!(state.apply(TextFieldInput::SelectAll));
    world.entity_mut(field_entity).insert(TextField(state));
    app.world_mut()
        .commands()
        .trigger(ThemeSwitch(LightDark::Light));
    app.update();

    let light = UiPalette::new(&Theme::light());
    let world = app.world_mut();
    let mut washes = world.query::<(&TextFieldSelection, &BackgroundColor)>();
    let (_, wash) = washes.iter(world).next().expect("wash survives the flip");
    assert_eq!(wash.0, light.selection_fill(), "wash re-derives from light");
    assert!(
        world.get::<TextField>(field_entity).is_some(),
        "the field kept its entity id across the flip"
    );
}
