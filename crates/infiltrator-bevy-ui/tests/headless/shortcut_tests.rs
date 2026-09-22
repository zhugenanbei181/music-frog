//! Headless tests for the Bevy global-shortcut wiring (DUAL-15-06): real
//! keyboard chords resolve through the shared registry into typed
//! `UiCommand`s, and a capture rebind goes out through the shared settings
//! command path.

use std::sync::Arc;

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::input::ButtonState;
use bevy::input::InputPlugin;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey};
use bevy::scene::ScenePlugin;
use bevy::window::Window;
use infiltrator_bevy_ui::app::{ShellPlugin, SidebarToggleProjection};
use infiltrator_bevy_ui::command::{CommandSinkHandle, DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::shortcuts::{
    BeginChordCapture, ChordPressed, HotkeyCapture, ShortcutBindings,
};
use infiltrator_contract::shortcuts::{KeyModifiers, ShortcutAction, ShortcutChord};
use infiltrator_contract::system_toggle::SystemToggle;

fn mounted_app() -> (App, Arc<DemoCommandSink>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin, InputPlugin));
    app.add_plugins(ShellPlugin::new(
        infiltrator_contract::theme::ThemePreference::Fixed(
            infiltrator_contract::theme::ThemeSkin::Dark,
        ),
    ));
    let sink = Arc::new(DemoCommandSink::accepting());
    app.insert_resource(CommandSinkHandle(sink.clone()));
    app.update();
    (app, sink)
}

/// Press a chord through the real keyboard message path, then release it.
fn press(app: &mut App, key: KeyCode, modifiers: &[KeyCode]) {
    let window = app.world_mut().spawn(Window::default()).id();
    {
        let world = app.world_mut();
        for modifier in modifiers {
            world.write_message(KeyboardInput {
                key_code: *modifier,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window,
            });
        }
        world.write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
    }
    app.update();
    {
        let world = app.world_mut();
        world.write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state: ButtonState::Released,
            text: None,
            repeat: false,
            window,
        });
        for modifier in modifiers {
            world.write_message(KeyboardInput {
                key_code: *modifier,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state: ButtonState::Released,
                text: None,
                repeat: false,
                window,
            });
        }
    }
    app.update();
    app.world_mut().entity_mut(window).despawn();
}

#[test]
fn product_defaults_bind_every_action() {
    let (app, _) = mounted_app();
    let bindings = app.world().resource::<ShortcutBindings>();
    assert_eq!(
        bindings.registry().bindings().len(),
        ShortcutAction::ALL.len()
    );
    assert_eq!(
        bindings
            .registry()
            .get(ShortcutAction::ToggleSystemProxy)
            .unwrap()
            .chord,
        ShortcutChord::ctrl_alt_key("P")
    );
}

#[test]
fn a_bound_chord_reaches_the_command_sink() {
    let (mut app, sink) = mounted_app();
    {
        let mut toggles = app.world_mut().resource_mut::<SidebarToggleProjection>();
        toggles.0 = toggles.0.clone().with_pending(SystemToggle::Tun, false);
    }
    press(
        &mut app,
        KeyCode::KeyT,
        &[KeyCode::ControlLeft, KeyCode::AltLeft],
    );
    assert!(
        sink.submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::ToggleTun { enabled: true })),
        "Ctrl+Alt+T must submit the TUN toggle command: {:?}",
        sink.submitted()
    );
}

#[test]
fn an_unbound_chord_submits_nothing() {
    let (mut app, sink) = mounted_app();
    press(&mut app, KeyCode::KeyQ, &[KeyCode::ControlLeft]);
    assert!(sink.submitted().is_empty(), "Ctrl+Q is unbound");
}

#[test]
fn the_theme_chord_advances_the_shared_preference() {
    let (mut app, _) = mounted_app();
    let before = app
        .world()
        .resource::<infiltrator_bevy_ui::appearance::ThemeMode>()
        .0;
    press(
        &mut app,
        KeyCode::KeyD,
        &[KeyCode::ControlLeft, KeyCode::AltLeft],
    );
    let after = app
        .world()
        .resource::<infiltrator_bevy_ui::appearance::ThemeMode>()
        .0;
    assert_eq!(after, before.next(), "Ctrl+Alt+D advances the preference");
}

#[test]
fn capture_rebinds_through_the_shared_settings_command() {
    let (mut app, sink) = mounted_app();
    app.world_mut()
        .commands()
        .trigger(BeginChordCapture(ShortcutAction::ToggleMiniHud));
    app.update();
    assert_eq!(
        app.world().resource::<HotkeyCapture>().0,
        Some(ShortcutAction::ToggleMiniHud)
    );

    press(
        &mut app,
        KeyCode::KeyH,
        &[KeyCode::ControlLeft, KeyCode::AltLeft],
    );
    assert_eq!(app.world().resource::<HotkeyCapture>().0, None);
    assert_eq!(
        app.world()
            .resource::<ShortcutBindings>()
            .registry()
            .get(ShortcutAction::ToggleMiniHud)
            .unwrap()
            .chord,
        ShortcutChord::ctrl_alt_key("H")
    );
    assert!(
        sink.submitted().iter().any(|command| matches!(
            command,
            UiCommand::UpdateSetting { key, value }
                if key == "shortcut.toggle_mini_hud" && value == "Ctrl+Alt+H"
        )),
        "the capture is persisted through the shared settings path: {:?}",
        sink.submitted()
    );
}

#[test]
fn capture_refuses_a_chord_owned_by_another_action() {
    let (mut app, sink) = mounted_app();
    app.world_mut()
        .commands()
        .trigger(BeginChordCapture(ShortcutAction::ToggleMiniHud));
    app.update();
    // Ctrl+Alt+T belongs to the TUN toggle.
    press(
        &mut app,
        KeyCode::KeyT,
        &[KeyCode::ControlLeft, KeyCode::AltLeft],
    );
    assert_eq!(
        app.world()
            .resource::<ShortcutBindings>()
            .registry()
            .get(ShortcutAction::ToggleMiniHud)
            .unwrap()
            .chord,
        ShortcutChord::ctrl_alt_key("M")
    );
    assert!(
        !sink
            .submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::UpdateSetting { .. })),
        "a conflicting capture must not be submitted"
    );
}

#[test]
fn a_chord_pressed_for_the_mini_hud_toggles_the_mounted_overlay() {
    let (mut app, sink) = mounted_app();
    assert!(
        !app.world()
            .resource::<infiltrator_bevy_ui::mini_hud::MiniHudMode>()
            .0
    );
    press(
        &mut app,
        KeyCode::KeyM,
        &[KeyCode::ControlLeft, KeyCode::AltLeft],
    );
    // The shortcut flips the mounted HUD; it is a local view command, so no
    // sink command goes out (the global-chord dispatch is not a settings write).
    assert!(
        app.world()
            .resource::<infiltrator_bevy_ui::mini_hud::MiniHudMode>()
            .0
    );
    assert!(sink.submitted().is_empty());
    let _ = ChordPressed {
        key: "M".to_string(),
        modifiers: KeyModifiers::ctrl_alt(),
    };
}
