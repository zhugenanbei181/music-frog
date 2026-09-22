//! Global shortcut bindings for the Bevy shell.
//!
//! The chord grammar, the product defaults, the conflict rules and the
//! persisted shape all live in `infiltrator_contract::shortcuts`. This module
//! owns the Bevy wiring only: a resource mirror of the shared registry, the
//! keyboard source that turns pressed keys into chords, the capture state for
//! rebinding, and dispatch of bound chords into typed [`UiCommand`]s (or the
//! local appearance command).

use bevy::app::{App, Plugin, PreUpdate, Update};
use bevy::ecs::event::Event;
use bevy::ecs::message::{Message, MessageReader, MessageWriter};
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use infiltrator_contract::shortcuts::{
    KeyModifiers, ShortcutAction, ShortcutChord, ShortcutRegistry,
};
use infiltrator_contract::system_toggle::SystemToggle;

use crate::app::SidebarToggleProjection;
use crate::appearance::{SystemAppearance, ThemeMode, resolved_skin};
use crate::command::{CommandSinkHandle, UiCommand};
use bevy::time::{Time, Virtual};
use infiltrator_bevy_widgets::switch::ThemeSwitch;

/// The live binding set (shared registry).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ShortcutBindings(pub ShortcutRegistry);

impl ShortcutBindings {
    pub fn registry(&self) -> &ShortcutRegistry {
        &self.0
    }
}

/// The action currently awaiting a captured chord, if any.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotkeyCapture(pub Option<ShortcutAction>);

/// One pressed key plus its modifiers, forwarded by the keyboard source.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ChordPressed {
    pub key: String,
    pub modifiers: KeyModifiers,
}

/// Request a rebind: the next chord is captured for this action.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BeginChordCapture(pub ShortcutAction);

/// Abandon the in-flight rebind.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelChordCapture;

/// Resolve a chord against the shared registry (pure).
pub fn resolve_action(
    registry: &ShortcutRegistry,
    chord: &ShortcutChord,
) -> Option<ShortcutAction> {
    registry.resolve(chord)
}

/// The chord a raw key event denotes (pure): `None` for a bare modifier.
pub fn chord_from_key(key: KeyCode, modifiers: KeyModifiers) -> Option<ShortcutChord> {
    let character = match key {
        KeyCode::KeyA => 'A',
        KeyCode::KeyB => 'B',
        KeyCode::KeyC => 'C',
        KeyCode::KeyD => 'D',
        KeyCode::KeyE => 'E',
        KeyCode::KeyF => 'F',
        KeyCode::KeyG => 'G',
        KeyCode::KeyH => 'H',
        KeyCode::KeyI => 'I',
        KeyCode::KeyJ => 'J',
        KeyCode::KeyK => 'K',
        KeyCode::KeyL => 'L',
        KeyCode::KeyM => 'M',
        KeyCode::KeyN => 'N',
        KeyCode::KeyO => 'O',
        KeyCode::KeyP => 'P',
        KeyCode::KeyQ => 'Q',
        KeyCode::KeyR => 'R',
        KeyCode::KeyS => 'S',
        KeyCode::KeyT => 'T',
        KeyCode::KeyU => 'U',
        KeyCode::KeyV => 'V',
        KeyCode::KeyW => 'W',
        KeyCode::KeyX => 'X',
        KeyCode::KeyY => 'Y',
        KeyCode::KeyZ => 'Z',
        KeyCode::Digit0 => '0',
        KeyCode::Digit1 => '1',
        KeyCode::Digit2 => '2',
        KeyCode::Digit3 => '3',
        KeyCode::Digit4 => '4',
        KeyCode::Digit5 => '5',
        KeyCode::Digit6 => '6',
        KeyCode::Digit7 => '7',
        KeyCode::Digit8 => '8',
        KeyCode::Digit9 => '9',
        KeyCode::Escape => return None,
        _ => return None,
    };
    Some(ShortcutChord::new(character.to_string(), modifiers))
}

/// Read the platform modifiers out of the live keyboard state (pure).
pub fn modifiers_from_keyboard(keyboard: &ButtonInput<KeyCode>) -> KeyModifiers {
    KeyModifiers {
        ctrl: keyboard.pressed(KeyCode::ControlLeft) || keyboard.pressed(KeyCode::ControlRight),
        shift: keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight),
        alt: keyboard.pressed(KeyCode::AltLeft) || keyboard.pressed(KeyCode::AltRight),
        meta: keyboard.pressed(KeyCode::SuperLeft) || keyboard.pressed(KeyCode::SuperRight),
    }
}

/// Forward every freshly pressed key as a [`ChordPressed`]. The keyboard
/// resource is optional: the headless shell (no input plugin) simply has no
/// key source, and the windowed composition installs `InputPlugin`.
pub fn forward_pressed_chords(
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mut chords: MessageWriter<ChordPressed>,
) {
    let Some(keyboard) = keyboard else {
        return;
    };
    for key in keyboard.get_just_pressed() {
        let modifiers = modifiers_from_keyboard(&keyboard);
        let Some(chord) = chord_from_key(*key, modifiers) else {
            continue;
        };
        chords.write(ChordPressed {
            key: chord.key().to_string(),
            modifiers: chord.modifiers(),
        });
    }
}

/// Observer for [`BeginChordCapture`].
pub fn on_begin_chord_capture(
    capture: On<BeginChordCapture>,
    mut hotkey_capture: ResMut<HotkeyCapture>,
) {
    hotkey_capture.0 = Some(capture.event().0);
}

/// Observer for [`CancelChordCapture`].
pub fn on_cancel_chord_capture(
    _cancel: On<CancelChordCapture>,
    mut hotkey_capture: ResMut<HotkeyCapture>,
) {
    hotkey_capture.0 = None;
}

/// Resolve a pressed chord: capture it for the pending action, or dispatch it.
#[allow(clippy::too_many_arguments)]
pub fn dispatch_chords(
    mut chords: MessageReader<ChordPressed>,
    mut bindings: ResMut<ShortcutBindings>,
    mut hotkey_capture: ResMut<HotkeyCapture>,
    toggles: Res<SidebarToggleProjection>,
    mut theme: ResMut<ThemeMode>,
    appearance: Res<SystemAppearance>,
    time: Res<Time<Virtual>>,
    mut toast_gate: ResMut<crate::toast::ToastPolicyGate>,
    mut toast_spawns: MessageWriter<infiltrator_bevy_widgets::toast::ToastSpawnEvent>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    for pressed in chords.read() {
        let chord = ShortcutChord::new(pressed.key.clone(), pressed.modifiers);
        if let Some(action) = hotkey_capture.0 {
            hotkey_capture.0 = None;
            // The shared registry is the conflict authority: a chord owned by
            // another action keeps the old binding, surfaces a toast, and is
            // not submitted.
            if let Some(conflict) = bindings.0.find_conflict(action, &chord) {
                toast_gate.push(
                    &mut toast_spawns,
                    infiltrator_bevy_widgets::toast::ToastKind::Warning,
                    &conflict.message,
                    5.0,
                    crate::toast::now_ms(&time),
                );
                continue;
            }
            let _ = bindings.0.bind_or_replace(action, chord.clone());
            if let Some(sink) = sink.as_deref() {
                sink.submit(UiCommand::UpdateSetting {
                    key: format!("shortcut.{}", action.id()),
                    value: chord.display_string(false),
                });
            }
            continue;
        }

        let Some(action) = resolve_action(&bindings.0, &chord) else {
            continue;
        };
        match action {
            ShortcutAction::ToggleSystemProxy => {
                let enabled = !toggles.0.state(SystemToggle::SystemProxy).is_enabled();
                if let Some(sink) = sink.as_deref() {
                    sink.submit(UiCommand::SetSystemProxy { enabled });
                }
            }
            ShortcutAction::ToggleTun => {
                let enabled = !toggles.0.state(SystemToggle::Tun).is_enabled();
                if let Some(sink) = sink.as_deref() {
                    sink.submit(UiCommand::ToggleTun { enabled });
                }
            }
            ShortcutAction::CycleTheme => {
                let next = theme.0.next();
                theme.0 = next;
                commands.trigger(ThemeSwitch(resolved_skin(next, appearance.0)));
            }
            ShortcutAction::OpenCommandPalette => {
                commands.trigger(crate::command_palette::ToggleCommandPalette);
            }
            ShortcutAction::ToggleMiniHud => {
                commands.trigger(crate::mini_hud::ToggleMiniHud);
            }
        }
    }
}

/// Register the shortcut resource, observers, and systems.
pub struct ShortcutsPlugin;

impl Plugin for ShortcutsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShortcutBindings>();
        app.init_resource::<HotkeyCapture>();
        app.add_message::<ChordPressed>();
        app.add_observer(on_begin_chord_capture);
        app.add_observer(on_cancel_chord_capture);
        // Read the freshly pressed keys *after* the input plugin has applied
        // this frame's keyboard messages (it clears the just-pressed set).
        app.add_systems(
            PreUpdate,
            forward_pressed_chords.after(bevy::input::InputSystems),
        );
        app.add_systems(Update, dispatch_chords);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_mapping_covers_letters_digits_and_skips_modifiers() {
        let chord = chord_from_key(KeyCode::KeyK, KeyModifiers::ctrl()).expect("letter chord");
        assert_eq!(chord, ShortcutChord::ctrl_key("K"));
        assert_eq!(
            chord_from_key(KeyCode::Escape, KeyModifiers::default()),
            None
        );
        assert_eq!(
            chord_from_key(KeyCode::Digit1, KeyModifiers::default()),
            Some(ShortcutChord::new("1", KeyModifiers::default()))
        );
    }

    #[test]
    fn default_bindings_resolve_the_product_chords() {
        let bindings = ShortcutBindings::default();
        assert_eq!(bindings.0.bindings().len(), ShortcutAction::ALL.len());
        assert_eq!(
            resolve_action(&bindings.0, &ShortcutChord::ctrl_key("K")),
            Some(ShortcutAction::OpenCommandPalette)
        );
        assert_eq!(
            resolve_action(&bindings.0, &ShortcutChord::ctrl_alt_key("M")),
            Some(ShortcutAction::ToggleMiniHud)
        );
    }
}
