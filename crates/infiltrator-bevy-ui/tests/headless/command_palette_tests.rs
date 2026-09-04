//! Headless integration tests for Category 1: Global Command Palette (`Ctrl+K`).
//!
//! Asserts:
//! 1. Declarative BSN modal scene mounting with accessibility semantics.
//! 2. Open / Close / Toggle lifecycle observers.
//! 3. Action execution dispatching to `RouteChanged` and `CommandSinkHandle`.

use std::sync::Arc;

use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::scene::CommandsSceneExt;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_palette::{
    CloseCommandPalette, CommandPaletteOverlayRoot, CommandPaletteState,
    ExecuteSelectedPaletteAction, OpenCommandPalette, ToggleCommandPalette,
    command_palette_modal_scene, on_close_command_palette, on_execute_selected_palette_action,
    on_open_command_palette, on_toggle_command_palette,
};
use infiltrator_bevy_ui::route::{ActiveRoute, Route};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;

use crate::support::*;

#[test]
fn test_command_palette_scene_mounting() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.init_resource::<CommandPaletteState>();

    let palette = UiPalette::new(&theme);
    let state = CommandPaletteState::new();

    let scene = command_palette_modal_scene(&palette, &state);
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
fn test_command_palette_observers_open_close_toggle() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.init_resource::<CommandPaletteState>();

    app.add_observer(on_open_command_palette);
    app.add_observer(on_close_command_palette);
    app.add_observer(on_toggle_command_palette);
    app.update();

    assert!(!app.world().resource::<CommandPaletteState>().is_open);

    // Open
    app.world_mut().commands().trigger(OpenCommandPalette);
    app.update();
    assert!(app.world().resource::<CommandPaletteState>().is_open);

    // Toggle -> closes
    app.world_mut().commands().trigger(ToggleCommandPalette);
    app.update();
    assert!(!app.world().resource::<CommandPaletteState>().is_open);

    // Toggle -> opens
    app.world_mut().commands().trigger(ToggleCommandPalette);
    app.update();
    assert!(app.world().resource::<CommandPaletteState>().is_open);

    // Close
    app.world_mut().commands().trigger(CloseCommandPalette);
    app.update();
    assert!(!app.world().resource::<CommandPaletteState>().is_open);
}

#[test]
fn test_command_palette_action_execution_dispatches_route_and_command() {
    let mut app = App::new();
    headless_plugins(&mut app);
    let theme = Theme::dark();
    app.add_plugins(WidgetsPlugin::new(&theme));
    app.init_resource::<CommandPaletteState>();
    app.init_resource::<ActiveRoute>();

    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(
        sink.clone() as Arc<dyn UiCommandSink>
    ));

    app.add_observer(on_execute_selected_palette_action);
    app.update();

    // 1. Test Navigation action: select 'nav.dns'
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

    // 2. Test Command action: select 'maint.clear_logs'
    {
        let mut state = app.world_mut().resource_mut::<CommandPaletteState>();
        state.open();
        state.set_query("清空");
        assert_eq!(
            state.current_selected_action().unwrap().id,
            "maint.clear_logs"
        );
    }

    app.world_mut()
        .commands()
        .trigger(ExecuteSelectedPaletteAction);
    app.update();

    let submitted = sink.submitted();
    assert_eq!(submitted.len(), 1);
    assert_eq!(submitted[0], UiCommand::ClearLogs);
    assert!(!app.world().resource::<CommandPaletteState>().is_open);
}
