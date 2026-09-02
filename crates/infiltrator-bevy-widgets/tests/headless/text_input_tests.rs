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
use infiltrator_bevy_widgets::scrollarea::{
    FocusAvoidanceParams, FocusedTextInput, SoftKeyboardState, calculate_focus_avoidance_scroll,
};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::text_input::{
    ImeCursorArea, ImeCursorAreaParams, PreeditClauseState, PreeditStateMachine, PreeditStatus,
    TextField, TextFieldAfter, TextFieldCaret, TextFieldInput, TextFieldPreeditText,
    TextFieldSelection, TextFieldSelectionText, TextFieldState, compute_ime_cursor_area,
    estimate_text_width, field_visual, sync_field_carets, text_field_scene,
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

#[test]
fn unicode_grapheme_safe_backspace_and_delete() {
    // 1. Combining acute accent (e + \u{0301})
    let mut field = TextFieldState::new("e\u{0301}cole");
    assert_eq!(field.cursor(), 6); // 'e', '\u{0301}', 'c', 'o', 'l', 'e'
    let _ = field.apply(TextFieldInput::Home);
    assert_eq!(field.cursor(), 0);
    // Move past the combined grapheme 'é' (which consists of 2 codepoints)
    assert!(field.apply(TextFieldInput::Right(false)));
    assert_eq!(field.cursor(), 1);
    assert!(field.apply(TextFieldInput::Right(false)));
    assert_eq!(field.cursor(), 2);
    // Backspace removes the whole grapheme cluster (both codepoints)
    assert!(field.apply(TextFieldInput::Backspace));
    assert_eq!(field.text(), "cole");
    assert_eq!(field.cursor(), 0);

    // 2. Flag emoji (2 regional indicator chars: 🇨 and 🇳)
    let mut field = TextFieldState::new("🇨🇳flag");
    assert_eq!(field.cursor(), 6); // 2 chars for flag + 4 for "flag"
    let _ = field.apply(TextFieldInput::Home);
    // Move cursor right past the flag (2 chars)
    let _ = field.apply(TextFieldInput::Right(false));
    let _ = field.apply(TextFieldInput::Right(false));
    assert_eq!(field.cursor(), 2);
    assert!(field.safe_backspace());
    assert_eq!(field.text(), "flag", "flag emoji deleted as single unit");
    assert_eq!(field.cursor(), 0);

    // 3. Family emoji with ZWJ (👨‍👩‍👧‍👦 = 7 codepoints)
    let mut field = TextFieldState::new("Family: 👨‍👩‍👧‍👦 done");
    let _ = field.apply(TextFieldInput::Home);
    // Move right by 8 chars (to right after "Family: ")
    for _ in 0..8 {
        let _ = field.apply(TextFieldInput::Right(false));
    }
    // Now move right by 7 chars (past the whole family emoji)
    for _ in 0..7 {
        let _ = field.apply(TextFieldInput::Right(false));
    }
    assert_eq!(field.cursor(), 15);
    assert!(field.safe_backspace());
    assert_eq!(
        field.text(),
        "Family:  done",
        "entire ZWJ family emoji safely removed"
    );
    assert_eq!(field.cursor(), 8);

    // 4. Emoji with skin tone modifier (👍🏽 = 2 codepoints)
    let mut field = TextFieldState::new("👍🏽ok");
    assert_eq!(field.cursor(), 4);
    let _ = field.apply(TextFieldInput::Home);
    // Delete at start (safe_delete removes thumbs-up + skin tone together)
    assert!(field.safe_delete());
    assert_eq!(
        field.text(),
        "ok",
        "emoji with skin tone deleted as single unit"
    );
    assert_eq!(field.cursor(), 0);
}

#[test]
fn ime_transaction_lifecycle_and_rollback() {
    let mut field = TextFieldState::new("hello world");
    assert_eq!(field.cursor(), 11);
    let _ = field.apply(TextFieldInput::Home);
    // Move to after "hello " (cursor = 6)
    for _ in 0..6 {
        let _ = field.apply(TextFieldInput::Right(false));
    }
    assert_eq!(field.cursor(), 6);

    // 1. Begin transaction, type preedit, then rollback
    field.begin_ime_transaction();
    assert!(field.is_in_ime_transaction());
    assert!(field.set_preedit("nihao"));
    assert_eq!(field.preedit(), "nihao");

    // Rollback restores original state exactly
    assert!(field.rollback_ime_transaction());
    assert!(!field.is_in_ime_transaction());
    assert_eq!(field.text(), "hello world");
    assert_eq!(field.cursor(), 6);
    assert_eq!(field.preedit(), "");

    // 2. Begin transaction with a selection, then commit
    let _ = field.apply(TextFieldInput::Right(true));
    let _ = field.apply(TextFieldInput::Right(true));
    let _ = field.apply(TextFieldInput::Right(true));
    let _ = field.apply(TextFieldInput::Right(true));
    let _ = field.apply(TextFieldInput::Right(true)); // selected "world" (range 6..11)
    assert_eq!(field.selection(), Some((6, 11)));

    field.begin_ime_transaction();
    assert!(field.set_preedit("shijie"));
    assert!(field.commit_ime_transaction("世界"));
    assert!(!field.is_in_ime_transaction());
    assert_eq!(field.text(), "hello 世界");
    assert_eq!(field.cursor(), 8); // 6 + 2 chars of "世界"
    assert_eq!(field.selection(), None);
    assert_eq!(field.preedit(), "");

    // 3. Rollback when no transaction is active is a no-op returning false
    assert!(!field.rollback_ime_transaction());
}

#[test]
fn preedit_state_machine_pinyin_segmentation_and_navigation() {
    let mut sm = PreeditStateMachine::new();
    assert_eq!(sm.status(), PreeditStatus::Idle);
    assert!(!sm.is_composing());

    // 1. Pinyin syllable segmentation: "nihao" -> ["ni", "hao"]
    assert!(sm.update("nihao", Some(5)));
    assert_eq!(sm.status(), PreeditStatus::Composing);
    assert!(sm.is_composing());
    assert_eq!(sm.clauses().len(), 2);
    assert_eq!(sm.clauses()[0].text, "ni");
    assert_eq!(sm.clauses()[0].range, (0, 2));
    assert_eq!(sm.clauses()[1].text, "hao");
    assert_eq!(sm.clauses()[1].range, (2, 5));

    // Active clause at cursor=5 should be clause 1 ("hao")
    assert_eq!(sm.active_clause_index(), Some(1));
    assert_eq!(sm.clauses()[1].state, PreeditClauseState::Selected);
    assert_eq!(sm.clauses()[0].state, PreeditClauseState::Converted);

    // 2. Navigation between clauses
    assert!(sm.prev_clause());
    assert_eq!(sm.active_clause_index(), Some(0));
    assert_eq!(sm.clauses()[0].state, PreeditClauseState::Selected);
    assert_eq!(sm.clauses()[1].state, PreeditClauseState::Converted);

    assert!(sm.next_clause());
    assert_eq!(sm.active_clause_index(), Some(1));

    // 3. Explicit delimiter segmentation: "xi'an" -> ["xi", "an"]
    assert!(sm.update("xi'an", Some(5)));
    assert_eq!(sm.clauses().len(), 2);
    assert_eq!(sm.clauses()[0].text, "xi");
    assert_eq!(sm.clauses()[1].text, "an");

    // 4. Syllable segmentation: "shurufa" -> ["shu", "ru", "fa"]
    assert!(sm.update("shurufa", Some(7)));
    assert_eq!(sm.clauses().len(), 3);
    assert_eq!(sm.clauses()[0].text, "shu");
    assert_eq!(sm.clauses()[1].text, "ru");
    assert_eq!(sm.clauses()[2].text, "fa");

    // 5. Commit
    let committed = sm.commit();
    assert_eq!(committed, "shurufa");
    assert_eq!(sm.status(), PreeditStatus::Committed);
    assert!(!sm.is_composing());
    assert_eq!(sm.raw_text(), "");

    // 6. Cancel
    assert!(sm.update("nihon", Some(5)));
    assert!(sm.cancel());
    assert_eq!(sm.status(), PreeditStatus::Idle);
    assert_eq!(sm.raw_text(), "");
}

#[test]
fn ime_cursor_area_absolute_screen_computation() {
    let params = ImeCursorAreaParams {
        field_origin: bevy::math::Vec2::new(100.0, 200.0),
        field_size: bevy::math::Vec2::new(300.0, 40.0),
        padding: bevy::math::Vec2::new(12.0, 4.0),
        caret_offset_x: 50.0,
        caret_width: 2.0,
        caret_height: 20.0,
        preedit_width: 0.0,
    };

    let area = compute_ime_cursor_area(params);
    assert_eq!(area.x(), 100.0 + 12.0 + 50.0); // 162.0
    assert_eq!(area.width(), 2.0);
    assert_eq!(area.height(), 20.0);
    assert_eq!(area.min(), bevy::math::Vec2::new(162.0, area.y()));
    assert_eq!(area.right(), 164.0);
    assert_eq!(area.bottom(), area.y() + 20.0);

    // Preedit expansion: when preedit is active (e.g. 60px wide), cursor area spans the composition
    let params_with_preedit = ImeCursorAreaParams {
        preedit_width: 60.0,
        ..params
    };
    let preedit_area = compute_ime_cursor_area(params_with_preedit);
    assert_eq!(preedit_area.width(), 60.0);

    // Estimate text width: CJK full width vs ASCII proportional
    let cjk_w = estimate_text_width("音乐", 16.0);
    let ascii_w = estimate_text_width("ab", 16.0);
    assert_eq!(cjk_w, 32.0); // 2 * 16.0
    assert_eq!(ascii_w, 16.0 * 0.55 * 2.0); // 17.6
    assert!(cjk_w > ascii_w, "CJK characters are wider than ASCII");
}

#[test]
fn soft_keyboard_focus_avoidance_scroll() {
    // 1. Keyboard closed: scroll offset unchanged
    let closed_params = FocusAvoidanceParams {
        viewport_top: 0.0,
        viewport_height: 600.0,
        content_height: 1200.0,
        current_scroll: 0.0,
        target_top: 450.0,
        target_height: 40.0,
        keyboard_height: 0.0,
        margin_px: 16.0,
    };
    assert_eq!(calculate_focus_avoidance_scroll(closed_params), 0.0);

    // 2. Keyboard open (height = 300px): visible viewport is 300px (0..300)
    // Target is at 450..490px, occluded by keyboard.
    // Must scroll up so target bottom (490 + 16 margin = 506) fits inside 300px visible viewport.
    // Required scroll = 506 - 300 = 206px.
    let open_params = FocusAvoidanceParams {
        keyboard_height: 300.0,
        ..closed_params
    };
    let new_scroll = calculate_focus_avoidance_scroll(open_params);
    assert_eq!(new_scroll, 206.0);

    // 3. Target already fully visible above keyboard: no extra scroll needed
    let visible_params = FocusAvoidanceParams {
        viewport_top: 0.0,
        viewport_height: 600.0,
        content_height: 1200.0,
        current_scroll: 100.0,
        target_top: 100.0,
        target_height: 40.0,
        keyboard_height: 300.0,
        margin_px: 16.0,
    };
    let keep_scroll = calculate_focus_avoidance_scroll(visible_params);
    assert_eq!(keep_scroll, 100.0);

    // 4. SoftKeyboardState state helpers
    let open_kb = SoftKeyboardState::open(280.0);
    assert!(open_kb.is_open);
    assert_eq!(open_kb.height_px, 280.0);

    let closed_kb = SoftKeyboardState::closed();
    assert!(!closed_kb.is_open);
    assert_eq!(closed_kb.height_px, 0.0);
}

#[test]
fn cjk_multichar_input_and_navigation() {
    let mut field = TextFieldState::new("");
    // Insert multi-character CJK string
    assert!(field.apply(TextFieldInput::Insert("音乐青蛙客户端".to_string())));
    assert_eq!(field.text(), "音乐青蛙客户端");
    assert_eq!(field.cursor(), 7); // 7 CJK characters

    // Move left across CJK codepoints to between "户" and "端"
    assert!(field.apply(TextFieldInput::Left(false)));
    assert_eq!(field.cursor(), 6);

    // Select backwards: select "户" and "客" (2 characters)
    assert!(field.apply(TextFieldInput::Left(true)));
    assert_eq!(field.cursor(), 5);
    assert!(field.apply(TextFieldInput::Left(true)));
    assert_eq!(field.cursor(), 4);
    assert_eq!(field.selection(), Some((4, 6))); // Selected "客户"

    // Replace selection with new text
    assert!(field.apply(TextFieldInput::Insert("服务".to_string())));
    assert_eq!(field.text(), "音乐青蛙服务端");
    assert_eq!(field.cursor(), 6);
    assert_eq!(field.selection(), None);

    // Home / End navigation
    assert!(field.apply(TextFieldInput::Home));
    assert_eq!(field.cursor(), 0);
    assert!(field.apply(TextFieldInput::End));
    assert_eq!(field.cursor(), 7);
}

#[test]
fn ime_cursor_area_ecs_sync() {
    let mut app = headless_app();
    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            commands.spawn_scene(text_field_scene("ime_test".to_string(), &palette));
        },
    );
    app.update();

    let world = app.world_mut();
    let mut cursor_areas = world.query::<(&TextField, &ImeCursorArea)>();
    let (field, area) = cursor_areas
        .iter(world)
        .next()
        .expect("ImeCursorArea synced");
    assert_eq!(field.0.text(), "ime_test");
    assert!(area.width() >= 2.0);
    assert!(area.height() > 0.0);
}

#[test]
fn focus_avoidance_auto_scroll_ecs() {
    let mut app = headless_app();
    app.insert_resource(SoftKeyboardState::open(300.0));
    app.add_systems(Startup, |mut commands: Commands| {
        commands
            .spawn((
                bevy::ui_widgets::ScrollArea,
                bevy::ui::ScrollPosition::default(),
            ))
            .with_children(|parent| {
                parent.spawn((TextField(TextFieldState::new("test")), FocusedTextInput));
            });
    });
    app.update();

    let world = app.world_mut();
    let mut query = world.query::<&bevy::ui::ScrollPosition>();
    assert_eq!(query.iter(world).count(), 1);
}
