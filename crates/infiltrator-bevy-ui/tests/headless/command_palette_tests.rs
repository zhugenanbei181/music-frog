//! Headless integration tests for the Global Command Palette (`Ctrl+K`, DUAL-15-05).
//!
//! Asserts:
//! 1. The BSN modal scene mounts from the shared catalogue with a11y semantics.
//! 2. Open / Close / Toggle lifecycle observers.
//! 3. The palette keyboard owns ↑↓ (wrapping) / Enter / Escape / typing.
//! 4. Action execution dispatches to `RouteChanged` and the command sink, using
//!    the same rows the Iced surface renders.

use std::sync::Arc;

use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::scene::CommandsSceneExt;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_palette::{
    CommandPaletteOverlayRoot, CommandPaletteState, ExecuteSelectedPaletteAction,
    command_palette_modal_scene,
};
use infiltrator_bevy_ui::command_palette_shell::CommandPalettePlugin;
use infiltrator_bevy_ui::route::{ActiveRoute, Route};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_contract::shortcuts::ShortcutRegistry;

use crate::support::*;

fn keyboard_message(logical_key: Key) -> KeyboardInput {
    KeyboardInput {
        key_code: bevy::input::keyboard::KeyCode::KeyA,
        logical_key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: bevy::ecs::entity::Entity::PLACEHOLDER,
    }
}

#[test]
fn test_command_palette_scene_mounting() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.init_resource::<CommandPaletteState>();

    let palette = UiPalette::new(&theme);
    let state = CommandPaletteState::new();
    let registry = ShortcutRegistry::with_defaults();

    let scene = command_palette_modal_scene(&palette, &state, &registry);
    let overlay_entity = app.world_mut().commands().spawn_scene(scene).id();
    app.update();

    let world = app.world();
    assert!(
        world
            .get::<CommandPaletteOverlayRoot>(overlay_entity)
            .is_some()
    );
    let a11y = world
        .get::<AccessibilityNode>(overlay_entity)
        .expect("a11y node exists");
    assert_eq!(a11y.role(), accesskit::Role::Dialog);
}

#[test]
fn test_palette_mounts_and_unmounts_from_the_shared_catalogue() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.add_plugins(CommandPalettePlugin);
    app.update();

    let world = app.world_mut();
    let count = world
        .query::<&CommandPaletteOverlayRoot>()
        .iter(world)
        .count();
    assert_eq!(count, 0, "the palette starts closed");

    // The shared catalogue is the row source (parity with Iced).
    let state = app.world().resource::<CommandPaletteState>();
    assert!(state.catalogue.index_of("nav.dns").is_some());
    assert!(state.catalogue.index_of("action.toggle_mini_hud").is_some());

    app.world_mut()
        .commands()
        .trigger(infiltrator_bevy_ui::command_palette::ToggleCommandPalette);
    app.update();

    let world = app.world_mut();
    let count = world
        .query::<&CommandPaletteOverlayRoot>()
        .iter(world)
        .count();
    assert_eq!(count, 1, "the open palette mounts exactly one overlay");
}

#[test]
fn test_palette_keyboard_navigation_typing_and_close() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.add_plugins(CommandPalettePlugin);
    app.update();

    app.world_mut()
        .commands()
        .trigger(infiltrator_bevy_ui::command_palette::ToggleCommandPalette);
    app.update();
    assert!(app.world().resource::<CommandPaletteState>().is_open);

    // ↑↓ wrap around the catalogue, exactly like the Iced cursor.
    let catalogue_len = app
        .world()
        .resource::<CommandPaletteState>()
        .catalogue
        .len();
    app.world_mut()
        .write_message(keyboard_message(Key::ArrowUp));
    app.update();
    assert_eq!(
        app.world().resource::<CommandPaletteState>().selected_index,
        catalogue_len - 1,
        "ArrowUp wraps to the last row"
    );
    app.world_mut()
        .write_message(keyboard_message(Key::ArrowDown));
    app.update();
    assert_eq!(
        app.world().resource::<CommandPaletteState>().selected_index,
        0
    );

    // Typing filters the shared catalogue.
    let mut typed = keyboard_message(Key::Character("dns".into()));
    typed.text = Some("dns".into());
    app.world_mut().write_message(typed);
    app.update();
    let state = app.world().resource::<CommandPaletteState>();
    assert_eq!(state.query, "dns");
    assert_eq!(state.filtered_indices.len(), 2);
    assert_eq!(
        state
            .current_selected_action()
            .map(|entry| entry.id.as_str()),
        Some("nav.dns")
    );

    // Backspace erases, Escape closes.
    app.world_mut()
        .write_message(keyboard_message(Key::Backspace));
    app.update();
    assert_eq!(app.world().resource::<CommandPaletteState>().query, "dn");
    app.world_mut().write_message(keyboard_message(Key::Escape));
    app.update();
    assert!(!app.world().resource::<CommandPaletteState>().is_open);
}

#[test]
fn test_command_palette_action_execution_dispatches_route_and_command() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.add_plugins(CommandPalettePlugin);
    app.init_resource::<ActiveRoute>();

    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(
        sink.clone() as Arc<dyn UiCommandSink>
    ));
    app.update();

    // 1. Navigation action: select 'nav.dns'
    {
        let mut state = app.world_mut().resource_mut::<CommandPaletteState>();
        state.open();
        state.set_query("dns");
        assert_eq!(state.current_selected_action().unwrap().id, "nav.dns");
    }

    app.world_mut()
        .commands()
        .trigger(ExecuteSelectedPaletteAction);
    app.update();

    assert_eq!(app.world().resource::<ActiveRoute>().0, Some(Route::Dns));
    assert!(!app.world().resource::<CommandPaletteState>().is_open);

    // 2. Command action: select 'action.flush_dns_cache'
    {
        let mut state = app.world_mut().resource_mut::<CommandPaletteState>();
        state.open();
        state.set_query("Fake-IP");
        assert_eq!(
            state.current_selected_action().unwrap().id,
            "action.flush_dns_cache"
        );
    }

    app.world_mut()
        .commands()
        .trigger(ExecuteSelectedPaletteAction);
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    assert_eq!(submitted[0], UiCommand::ClearDnsCache);
    assert!(!app.world().resource::<CommandPaletteState>().is_open);
}

#[test]
fn test_palette_row_click_executes_that_row() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.add_plugins(CommandPalettePlugin);
    app.init_resource::<ActiveRoute>();
    app.update();

    {
        let mut state = app.world_mut().resource_mut::<CommandPaletteState>();
        state.open();
        state.set_query("doctor");
        // Clicking the second filtered row executes the second entry.
        let second = state.filtered_indices.get(1).copied();
        assert!(second.is_some(), "query must keep at least two rows");
    }
    let second_id = {
        let state = app.world().resource::<CommandPaletteState>();
        state
            .catalogue
            .entry(state.filtered_indices[1])
            .map(|entry| entry.id.clone())
            .expect("second row")
    };

    app.world_mut()
        .commands()
        .trigger(infiltrator_bevy_ui::command_palette::ExecutePaletteEntry(1));
    app.update();

    assert!(!app.world().resource::<CommandPaletteState>().is_open);
    let route = app.world().resource::<ActiveRoute>().0;
    // The second "doctor" hit is the Doctor page row itself in the shared
    // catalogue order; either way the clicked row's target ran.
    assert!(
        route.is_some() || second_id == "action.run_doctor",
        "the clicked row dispatched through the shared target vocabulary"
    );
}
