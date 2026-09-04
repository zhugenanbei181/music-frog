//! Global keyboard shortcut registration, conflict detection, and one-handed reachability mode.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure state machine dispatching chords to typed `UiCommand` items without direct OS hooks.

use crate::command::UiCommand;
use bevy::ecs::resource::Resource;

/// Standard key modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyModifiers {
    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn ctrl_shift() -> Self {
        Self {
            ctrl: true,
            shift: true,
            ..Default::default()
        }
    }
}

/// A combined keyboard chord (modifiers + primary key).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyboardChord {
    pub key: String,
    pub modifiers: KeyModifiers,
}

impl KeyboardChord {
    pub fn new(key: impl Into<String>, modifiers: KeyModifiers) -> Self {
        Self {
            key: key.into().to_uppercase(),
            modifiers,
        }
    }

    /// Human-readable formatting tailored for desktop OS.
    pub fn display_string(&self, is_macos: bool) -> String {
        let mut parts = Vec::new();
        if is_macos {
            if self.modifiers.ctrl {
                parts.push("⌃");
            }
            if self.modifiers.alt {
                parts.push("⌥");
            }
            if self.modifiers.shift {
                parts.push("⇧");
            }
            if self.modifiers.meta {
                parts.push("⌘");
            }
            parts.push(&self.key);
            parts.concat()
        } else {
            if self.modifiers.ctrl {
                parts.push("Ctrl");
            }
            if self.modifiers.alt {
                parts.push("Alt");
            }
            if self.modifiers.shift {
                parts.push("Shift");
            }
            if self.modifiers.meta {
                parts.push("Super");
            }
            parts.push(&self.key);
            parts.join("+")
        }
    }
}

/// Global registry managing shortcut bindings and conflict detection.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ShortcutRegistry {
    pub bindings: Vec<(KeyboardChord, UiCommand)>,
}

impl ShortcutRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a chord shortcut mapping to a typed command.
    /// Returns Err if the chord is already bound.
    pub fn bind(&mut self, chord: KeyboardChord, command: UiCommand) -> Result<(), String> {
        if let Some((existing, _)) = self.bindings.iter().find(|(c, _)| c == &chord) {
            return Err(format!(
                "Shortcut conflict: {} is already bound",
                existing.display_string(false)
            ));
        }
        self.bindings.push((chord, command));
        Ok(())
    }

    /// Lookup bound command for a chord.
    pub fn lookup(&self, chord: &KeyboardChord) -> Option<UiCommand> {
        self.bindings
            .iter()
            .find(|(c, _)| c == chord)
            .map(|(_, cmd)| cmd.clone())
    }
}

/// Mobile one-handed reachability mode (pulls top half down for thumb access).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ReachabilityMode {
    pub is_active: bool,
    pub offset_fraction: f32,
    pub timer_secs: f32,
    pub timeout_limit_secs: f32,
}

impl Default for ReachabilityMode {
    fn default() -> Self {
        Self {
            is_active: false,
            offset_fraction: 0.35, // pull down 35% of screen height
            timer_secs: 0.0,
            timeout_limit_secs: 8.0,
        }
    }
}

impl ReachabilityMode {
    pub fn toggle(&mut self) {
        self.is_active = !self.is_active;
        self.timer_secs = 0.0;
    }

    pub fn dismiss(&mut self) {
        self.is_active = false;
        self.timer_secs = 0.0;
    }

    /// Advance idle timer: automatically dismisses after timeout_limit_secs.
    pub fn tick(&mut self, dt_secs: f32) {
        if self.is_active {
            self.timer_secs += dt_secs;
            if self.timer_secs >= self.timeout_limit_secs {
                self.dismiss();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_chord_formatting_and_lookup() {
        let mut registry = ShortcutRegistry::new();
        let chord_k = KeyboardChord::new("K", KeyModifiers::ctrl());
        assert_eq!(chord_k.display_string(false), "Ctrl+K");

        assert!(
            registry
                .bind(chord_k.clone(), UiCommand::ClearDnsCache)
                .is_ok()
        );

        // Duplicate bind fails with conflict
        assert!(
            registry
                .bind(chord_k.clone(), UiCommand::ClearLogs)
                .is_err()
        );

        // Lookup succeeds
        assert_eq!(registry.lookup(&chord_k), Some(UiCommand::ClearDnsCache));
    }

    #[test]
    fn test_reachability_mode_lifecycle_and_timeout() {
        let mut reach = ReachabilityMode::default();
        assert!(!reach.is_active);

        reach.toggle();
        assert!(reach.is_active);

        // Advance 5s -> still active
        reach.tick(5.0);
        assert!(reach.is_active);

        // Advance past 8s -> automatically dismissed
        reach.tick(4.0);
        assert!(!reach.is_active);
    }
}
