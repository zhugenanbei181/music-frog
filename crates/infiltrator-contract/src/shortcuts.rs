//! Shared global-shortcut contract: chords, bindings, conflict detection and
//! the default shell binding set.
//!
//! Both surfaces must agree on three things: what a chord *is* (modifiers +
//! key, with one canonical accelerator rendering), which shell actions are
//! bindable, and what happens when two actions claim the same chord. The
//! registry below is the single source for all three; surfaces only map
//! [`ShortcutAction`] onto their own dispatch enum.

use serde::{Deserialize, Serialize};

/// The shell actions a global chord may dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutAction {
    /// Flip the OS system proxy.
    ToggleSystemProxy,
    /// Flip the TUN virtual adapter.
    ToggleTun,
    /// Show/hide the Mini HUD floating form.
    ToggleMiniHud,
    /// Open the keyboard Command Palette.
    OpenCommandPalette,
    /// Cycle the appearance preference.
    CycleTheme,
}

impl ShortcutAction {
    pub const ALL: [Self; 5] = [
        Self::ToggleSystemProxy,
        Self::ToggleTun,
        Self::ToggleMiniHud,
        Self::OpenCommandPalette,
        Self::CycleTheme,
    ];

    /// Stable identifier used by settings keys (`shortcut.<id>`) and tests.
    pub const fn id(self) -> &'static str {
        match self {
            Self::ToggleSystemProxy => "toggle_system_proxy",
            Self::ToggleTun => "toggle_tun",
            Self::ToggleMiniHud => "toggle_mini_hud",
            Self::OpenCommandPalette => "open_command_palette",
            Self::CycleTheme => "cycle_theme",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "toggle_system_proxy" => Some(Self::ToggleSystemProxy),
            "toggle_tun" => Some(Self::ToggleTun),
            "toggle_mini_hud" => Some(Self::ToggleMiniHud),
            "open_command_palette" => Some(Self::OpenCommandPalette),
            "cycle_theme" => Some(Self::CycleTheme),
            _ => None,
        }
    }

    /// i18n key of the action's display name (Iced table; Bevy maps its own).
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::ToggleSystemProxy => "hotkey_system_proxy",
            Self::ToggleTun => "hotkey_tun_mode",
            Self::ToggleMiniHud => "hotkey_mini_hud",
            Self::OpenCommandPalette => "hotkey_command_palette",
            Self::CycleTheme => "hotkey_cycle_theme",
        }
    }

    /// The action's product default binding.
    pub fn default_binding(self) -> ShortcutBinding {
        let chord = match self {
            Self::ToggleSystemProxy => ShortcutChord::new("P", KeyModifiers::ctrl_alt()),
            Self::ToggleTun => ShortcutChord::new("T", KeyModifiers::ctrl_alt()),
            Self::ToggleMiniHud => ShortcutChord::new("M", KeyModifiers::ctrl_alt()),
            Self::OpenCommandPalette => ShortcutChord::new("K", KeyModifiers::ctrl()),
            Self::CycleTheme => ShortcutChord::new("D", KeyModifiers::ctrl_alt()),
        };
        ShortcutBinding::new(self, chord)
    }
}

/// Keyboard modifier state for one chord.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub meta: bool,
}

impl KeyModifiers {
    pub const fn ctrl() -> Self {
        Self {
            ctrl: true,
            shift: false,
            alt: false,
            meta: false,
        }
    }

    pub const fn ctrl_alt() -> Self {
        Self {
            ctrl: true,
            shift: false,
            alt: true,
            meta: false,
        }
    }

    pub const fn is_empty(self) -> bool {
        !self.ctrl && !self.shift && !self.alt && !self.meta
    }

    pub const fn count(self) -> usize {
        self.ctrl as usize + self.shift as usize + self.alt as usize + self.meta as usize
    }
}

/// One key plus its modifiers. The key is stored uppercase, so chord equality
/// never depends on the surface's key-case convention.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShortcutChord {
    key: String,
    modifiers: KeyModifiers,
}

impl ShortcutChord {
    pub fn new(key: impl Into<String>, modifiers: KeyModifiers) -> Self {
        Self {
            key: key.into().trim().to_uppercase(),
            modifiers,
        }
    }

    pub fn ctrl_key(key: impl Into<String>) -> Self {
        Self::new(key, KeyModifiers::ctrl())
    }

    pub fn ctrl_alt_key(key: impl Into<String>) -> Self {
        Self::new(key, KeyModifiers::ctrl_alt())
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub const fn modifiers(&self) -> KeyModifiers {
        self.modifiers
    }

    /// Parse an accelerator string (`Ctrl+Alt+P`, `⌘⇧K`, `ctrl+k`).
    /// Returns `None` when the string carries no key token or a multi-letter
    /// key token.
    pub fn parse(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        let mut modifiers = KeyModifiers::default();
        let mut key: Option<String> = None;
        for token in split_accelerator(trimmed) {
            if token.is_empty() {
                continue;
            }
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "⌃" => modifiers.ctrl = true,
                "alt" | "option" | "opt" | "⌥" => modifiers.alt = true,
                "shift" | "⇧" => modifiers.shift = true,
                "meta" | "cmd" | "command" | "super" | "win" | "windows" | "⌘" => {
                    modifiers.meta = true
                }
                other => {
                    if key.is_some() || other.chars().count() != 1 {
                        return None;
                    }
                    key = Some(other.to_uppercase());
                }
            }
        }
        Some(Self {
            key: key?,
            modifiers,
        })
    }

    /// Whether a raw key event matches this chord. On macOS the platform
    /// command modifier (⌘) is folded into the product's Ctrl slot, so ⌘K and
    /// Ctrl+K are the same chord on their respective platforms.
    pub fn matches(&self, key: &str, modifiers: KeyModifiers, is_macos: bool) -> bool {
        let mut observed = modifiers;
        if is_macos && observed.meta {
            observed.ctrl = true;
            observed.meta = false;
        }
        self.key.eq_ignore_ascii_case(key.trim()) && self.modifiers == observed
    }

    /// Canonical accelerator rendering (⌃⌥⇧⌘ on macOS, `Ctrl+Alt+…` elsewhere).
    pub fn display_string(&self, is_macos: bool) -> String {
        let mut parts: Vec<&str> = Vec::new();
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
            format!("{}{}", parts.concat(), self.key)
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

/// Split an accelerator into tokens: `+`-separated, or macOS glyph runs.
fn split_accelerator(value: &str) -> Vec<String> {
    if value.contains('+') {
        return value
            .split('+')
            .map(|part| part.trim().to_string())
            .collect();
    }
    let mut tokens = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if matches!(character, '⌃' | '⌥' | '⇧' | '⌘') {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            tokens.push(character.to_string());
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// One action's binding.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShortcutBinding {
    pub action: ShortcutAction,
    pub chord: ShortcutChord,
    pub enabled: bool,
}

impl ShortcutBinding {
    pub fn new(action: ShortcutAction, chord: ShortcutChord) -> Self {
        Self {
            action,
            chord,
            enabled: true,
        }
    }

    /// Canonical accelerator for this binding.
    pub fn accelerator(&self, is_macos: bool) -> String {
        self.chord.display_string(is_macos)
    }
}

/// A chord claimed by a different action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutConflict {
    pub chord: ShortcutChord,
    pub existing: ShortcutAction,
    pub attempted: ShortcutAction,
    pub message: String,
}

/// The whole binding set, with conflict-free mutation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortcutRegistry {
    bindings: Vec<ShortcutBinding>,
}

impl Default for ShortcutRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl ShortcutRegistry {
    /// Empty registry (used when a surface wants to build its own set).
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// The product default set, one binding per [`ShortcutAction`].
    pub fn with_defaults() -> Self {
        Self {
            bindings: ShortcutAction::ALL
                .into_iter()
                .map(ShortcutAction::default_binding)
                .collect(),
        }
    }

    /// Build a registry from stored bindings (the caller usually follows with
    /// [`Self::normalize`] to repair missing or duplicate actions).
    pub fn from_bindings(bindings: Vec<ShortcutBinding>) -> Self {
        Self { bindings }
    }

    pub fn bindings(&self) -> &[ShortcutBinding] {
        &self.bindings
    }

    pub fn get(&self, action: ShortcutAction) -> Option<&ShortcutBinding> {
        self.bindings.iter().find(|b| b.action == action)
    }

    /// Whether the action is bound *and* enabled.
    pub fn is_active(&self, action: ShortcutAction) -> bool {
        self.get(action).map(|b| b.enabled).unwrap_or(false)
    }

    /// The action a chord dispatches, ignoring disabled bindings.
    pub fn resolve(&self, chord: &ShortcutChord) -> Option<ShortcutAction> {
        self.bindings
            .iter()
            .find(|b| b.enabled && &b.chord == chord)
            .map(|b| b.action)
    }

    /// The chord currently claimed by another *enabled* action.
    pub fn find_conflict(
        &self,
        action: ShortcutAction,
        chord: &ShortcutChord,
    ) -> Option<ShortcutConflict> {
        self.bindings
            .iter()
            .find(|b| b.enabled && b.action != action && &b.chord == chord)
            .map(|b| ShortcutConflict {
                chord: chord.clone(),
                existing: b.action,
                attempted: action,
                message: format!(
                    "{} is already bound to {}",
                    chord.display_string(false),
                    b.action.id()
                ),
            })
    }

    /// Bind (or rebind) an action's chord; a chord owned by another action is
    /// rejected with the conflict.
    pub fn bind_or_replace(
        &mut self,
        action: ShortcutAction,
        chord: ShortcutChord,
    ) -> Result<Option<ShortcutBinding>, ShortcutConflict> {
        if let Some(conflict) = self.find_conflict(action, &chord) {
            return Err(conflict);
        }
        let previous = self
            .bindings
            .iter()
            .position(|b| b.action == action)
            .map(|index| self.bindings.remove(index));
        self.bindings.push(ShortcutBinding::new(action, chord));
        Ok(previous)
    }

    pub fn set_enabled(&mut self, action: ShortcutAction, enabled: bool) -> bool {
        match self.bindings.iter_mut().find(|b| b.action == action) {
            Some(binding) => {
                binding.enabled = enabled;
                true
            }
            None => false,
        }
    }

    pub fn unbind(&mut self, action: ShortcutAction) -> bool {
        let previous = self.bindings.len();
        self.bindings.retain(|b| b.action != action);
        self.bindings.len() != previous
    }

    /// All collisions among enabled bindings (a registry loaded from an older
    /// settings file may carry duplicates the mutators would refuse).
    pub fn detect_conflicts(&self) -> Vec<ShortcutConflict> {
        let mut conflicts = Vec::new();
        for (index, binding) in self.bindings.iter().enumerate() {
            if !binding.enabled {
                continue;
            }
            for other in self.bindings.iter().skip(index + 1) {
                if other.enabled && other.chord == binding.chord {
                    conflicts.push(ShortcutConflict {
                        chord: binding.chord.clone(),
                        existing: binding.action,
                        attempted: other.action,
                        message: format!(
                            "{} is bound to both {} and {}",
                            binding.chord.display_string(false),
                            binding.action.id(),
                            other.action.id()
                        ),
                    });
                }
            }
        }
        conflicts
    }

    /// Restore `action` to its product default chord and re-enable it.
    pub fn reset_action(&mut self, action: ShortcutAction) -> Result<(), ShortcutConflict> {
        self.bind_or_replace(action, action.default_binding().chord)?;
        self.set_enabled(action, true);
        Ok(())
    }

    /// Restore the whole product default set.
    pub fn reset_all(&mut self) {
        *self = Self::with_defaults();
    }

    /// Drop duplicate actions from a deserialized set and add any missing
    /// product defaults whose chord is still free (settings-file upgrades).
    /// A custom binding that occupies another action's default chord wins;
    /// that action is left unbound until the user resets it.
    pub fn normalize(mut self) -> Self {
        let mut normalized = Self::empty();
        for binding in self.bindings.drain(..) {
            if normalized.get(binding.action).is_none() {
                normalized.bindings.push(binding);
            }
        }
        for action in ShortcutAction::ALL {
            if normalized.get(action).is_some() {
                continue;
            }
            let default_chord = action.default_binding().chord;
            if normalized.find_conflict(action, &default_chord).is_none() {
                normalized.bindings.push(action.default_binding());
            }
        }
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_set_binds_every_action_without_conflicts() {
        let registry = ShortcutRegistry::with_defaults();
        assert_eq!(registry.bindings().len(), ShortcutAction::ALL.len());
        for action in ShortcutAction::ALL {
            assert!(registry.is_active(action), "{action:?} must be active");
        }
        assert!(registry.detect_conflicts().is_empty());
        assert_eq!(
            registry.resolve(&ShortcutChord::ctrl_key("K")),
            Some(ShortcutAction::OpenCommandPalette)
        );
    }

    #[test]
    fn rebinding_to_an_occupied_chord_is_rejected() {
        let mut registry = ShortcutRegistry::with_defaults();
        let occupied = ShortcutChord::ctrl_alt_key("T");
        let conflict = registry
            .bind_or_replace(ShortcutAction::ToggleMiniHud, occupied)
            .expect_err("chord is owned by the TUN toggle");
        assert_eq!(conflict.existing, ShortcutAction::ToggleTun);
        assert_eq!(conflict.attempted, ShortcutAction::ToggleMiniHud);
        assert!(conflict.message.contains("Ctrl+Alt+T"));
        assert_eq!(
            registry.get(ShortcutAction::ToggleMiniHud).unwrap().chord,
            ShortcutChord::ctrl_alt_key("M")
        );
    }

    #[test]
    fn rebinding_the_same_action_is_a_replace() {
        let mut registry = ShortcutRegistry::with_defaults();
        let previous = registry
            .bind_or_replace(
                ShortcutAction::ToggleTun,
                ShortcutChord::new("y", KeyModifiers::ctrl_alt()),
            )
            .expect("free chord");
        assert_eq!(
            previous.map(|binding| binding.chord),
            Some(ShortcutChord::ctrl_alt_key("T"))
        );
        assert_eq!(registry.bindings().len(), ShortcutAction::ALL.len());
        assert!(registry.detect_conflicts().is_empty());
    }

    #[test]
    fn disabled_bindings_do_not_resolve() {
        let mut registry = ShortcutRegistry::with_defaults();
        assert!(registry.set_enabled(ShortcutAction::ToggleMiniHud, false));
        assert_eq!(registry.resolve(&ShortcutChord::ctrl_alt_key("M")), None);
        assert!(
            registry
                .find_conflict(
                    ShortcutAction::CycleTheme,
                    &ShortcutChord::ctrl_alt_key("M")
                )
                .is_none()
        );
    }

    #[test]
    fn chords_parse_from_accelerator_strings() {
        let chord = ShortcutChord::parse("Ctrl+Alt+P").expect("parse");
        assert_eq!(chord, ShortcutChord::ctrl_alt_key("P"));
        assert!(chord.matches("p", KeyModifiers::ctrl_alt(), false));
        assert!(!chord.matches("p", KeyModifiers::ctrl(), false));
        let macos = ShortcutChord::parse("⌘K").expect("parse macos");
        assert_eq!(macos.key(), "K");
        assert!(macos.modifiers().meta);
        assert!(ShortcutChord::parse("Ctrl+").is_none());
        assert!(ShortcutChord::parse("Ctrl+Alt+Delete").is_none());
    }

    #[test]
    fn macos_command_modifier_folds_into_the_ctrl_slot() {
        let chord = ShortcutChord::ctrl_key("K");
        let command_only = KeyModifiers {
            meta: true,
            ..KeyModifiers::default()
        };
        assert!(chord.matches("k", command_only, true));
        assert!(!chord.matches("k", command_only, false));
    }

    #[test]
    fn accelerator_rendering_agrees_with_parse() {
        for action in ShortcutAction::ALL {
            let chord = action.default_binding().chord;
            assert_eq!(
                ShortcutChord::parse(&chord.display_string(false)),
                Some(chord)
            );
        }
    }

    #[test]
    fn normalize_repairs_duplicate_and_missing_actions() {
        let mut registry = ShortcutRegistry::empty();
        registry.bindings.push(ShortcutBinding::new(
            ShortcutAction::ToggleTun,
            ShortcutChord::ctrl_key("K"),
        ));
        registry.bindings.push(ShortcutBinding::new(
            ShortcutAction::ToggleTun,
            ShortcutChord::ctrl_key("J"),
        ));
        let normalized = registry.normalize();
        // The duplicate action keeps its first binding and no conflict is
        // introduced by backfilling the missing product defaults.
        assert_eq!(
            normalized.get(ShortcutAction::ToggleTun).unwrap().chord,
            ShortcutChord::ctrl_key("K")
        );
        assert!(normalized.detect_conflicts().is_empty());
        // The palette default (Ctrl+K) is taken by the custom binding above,
        // so the unbound action is left for the user to reset rather than
        // silently shadowing the custom chord.
        assert!(normalized.get(ShortcutAction::OpenCommandPalette).is_none());
        assert!(normalized.get(ShortcutAction::ToggleMiniHud).is_some());
        assert_eq!(normalized.bindings().len(), ShortcutAction::ALL.len() - 1);
    }

    #[test]
    fn reset_action_restores_the_product_default() {
        let mut registry = ShortcutRegistry::with_defaults();
        registry
            .bind_or_replace(
                ShortcutAction::CycleTheme,
                ShortcutChord::new("Q", KeyModifiers::ctrl_alt()),
            )
            .expect("free chord");
        registry.set_enabled(ShortcutAction::CycleTheme, false);
        registry
            .reset_action(ShortcutAction::CycleTheme)
            .expect("reset");
        let binding = registry.get(ShortcutAction::CycleTheme).expect("bound");
        assert_eq!(binding.chord, ShortcutChord::ctrl_alt_key("D"));
        assert!(binding.enabled);
    }
}
