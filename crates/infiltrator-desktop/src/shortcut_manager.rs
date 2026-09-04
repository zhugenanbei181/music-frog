use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionIntent {
    ToggleSystemProxy,
    ToggleTun,
    SwitchMode,
    TestDelayAll,
    ShowWindow,
    HideWindow,
    ToggleProxy,
    ToggleMainWindow,
    OpenDashboard,
    SwitchNextProfile,
    FlushDnsCache,
    Custom(String),
}

pub type KeyAction = ActionIntent;

impl ActionIntent {
    pub fn as_intent_str(&self) -> &str {
        match self {
            Self::ToggleSystemProxy => "toggle_system_proxy",
            Self::ToggleTun => "toggle_tun",
            Self::SwitchMode => "switch_mode",
            Self::TestDelayAll => "test_delay_all",
            Self::ShowWindow => "show_window",
            Self::HideWindow => "hide_window",
            Self::ToggleProxy => "toggle_proxy",
            Self::ToggleMainWindow => "toggle_main_window",
            Self::OpenDashboard => "open_dashboard",
            Self::SwitchNextProfile => "switch_next_profile",
            Self::FlushDnsCache => "flush_dns_cache",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn from_intent_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "toggle_system_proxy" => Some(Self::ToggleSystemProxy),
            "toggle_tun" => Some(Self::ToggleTun),
            "switch_mode" | "switch_proxy_mode" => Some(Self::SwitchMode),
            "test_delay_all" | "test_all_delays" => Some(Self::TestDelayAll),
            "show_window" => Some(Self::ShowWindow),
            "hide_window" => Some(Self::HideWindow),
            "toggle_proxy" => Some(Self::ToggleProxy),
            "toggle_main_window" => Some(Self::ToggleMainWindow),
            "open_dashboard" => Some(Self::OpenDashboard),
            "switch_next_profile" => Some(Self::SwitchNextProfile),
            "flush_dns_cache" => Some(Self::FlushDnsCache),
            other if !other.is_empty() => Some(Self::Custom(s.to_string())),
            _ => None,
        }
    }
}

impl fmt::Display for ActionIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_intent_str())
    }
}

impl FromStr for ActionIntent {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_intent_str(s).ok_or_else(|| anyhow::anyhow!("Invalid action intent: {}", s))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ModifierKeys {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl ModifierKeys {
    pub fn is_empty(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.meta
    }

    pub fn count(&self) -> usize {
        self.ctrl as usize + self.alt as usize + self.shift as usize + self.meta as usize
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBinding {
    pub action: ActionIntent,
    pub key: String,
    pub modifiers: ModifierKeys,
    pub enabled: bool,
}

impl ShortcutBinding {
    pub fn new(action: ActionIntent, key: impl Into<String>, modifiers: ModifierKeys) -> Self {
        Self {
            action,
            key: key.into().to_uppercase(),
            modifiers,
            enabled: true,
        }
    }

    pub fn accelerator(&self) -> String {
        ShortcutManager::format_accelerator(self)
    }

    pub fn matches(&self, key: &str, modifiers: &ModifierKeys) -> bool {
        self.enabled && self.key.eq_ignore_ascii_case(key) && &self.modifiers == modifiers
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutCollision {
    pub accelerator: String,
    pub key: String,
    pub modifiers: ModifierKeys,
    pub existing_action: ActionIntent,
    pub attempted_action: ActionIntent,
    pub message: String,
}

impl fmt::Display for ShortcutCollision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Shortcut collision on '{}': existing action '{}' conflicts with attempted action '{}'",
            self.accelerator, self.existing_action, self.attempted_action
        )
    }
}

impl std::error::Error for ShortcutCollision {}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConflictReport {
    pub collisions: Vec<ShortcutCollision>,
}

impl ConflictReport {
    pub fn has_conflicts(&self) -> bool {
        !self.collisions.is_empty()
    }

    pub fn summary(&self) -> String {
        if self.collisions.is_empty() {
            "No shortcut collisions detected".to_string()
        } else {
            let details: Vec<String> = self.collisions.iter().map(|c| c.to_string()).collect();
            format!(
                "Found {} conflict(s):
{}",
                self.collisions.len(),
                details.join(
                    "
"
                )
            )
        }
    }
}

#[derive(Default)]
pub struct ShortcutManager {
    bindings: Vec<ShortcutBinding>,
}

impl ShortcutManager {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    pub fn new_default() -> Self {
        Self::new()
    }

    pub fn find_collision(&self, binding: &ShortcutBinding) -> Option<ShortcutCollision> {
        if !binding.enabled {
            return None;
        }
        for existing in &self.bindings {
            if existing.enabled
                && existing.key.eq_ignore_ascii_case(&binding.key)
                && existing.modifiers == binding.modifiers
            {
                let accel = Self::format_accelerator(binding);
                return Some(ShortcutCollision {
                    accelerator: accel.clone(),
                    key: binding.key.clone(),
                    modifiers: binding.modifiers,
                    existing_action: existing.action.clone(),
                    attempted_action: binding.action.clone(),
                    message: format!(
                        "Key combo '{}' is already registered for '{}'",
                        accel, existing.action
                    ),
                });
            }
        }
        None
    }

    pub fn register(&mut self, binding: ShortcutBinding) -> Result<()> {
        if let Some(collision) = self.find_collision(&binding) {
            bail!("Shortcut collision detected: {}", collision);
        }
        self.bindings.push(binding);
        Ok(())
    }

    pub fn register_or_replace(&mut self, binding: ShortcutBinding) -> Option<ShortcutBinding> {
        if let Some(idx) = self.bindings.iter().position(|b| {
            b.action == binding.action
                || (b.key.eq_ignore_ascii_case(&binding.key) && b.modifiers == binding.modifiers)
        }) {
            let old = self.bindings.remove(idx);
            self.bindings.push(binding);
            Some(old)
        } else {
            self.bindings.push(binding);
            None
        }
    }

    pub fn unregister_action(&mut self, action: &ActionIntent) -> bool {
        let prev_len = self.bindings.len();
        self.bindings.retain(|b| &b.action != action);
        self.bindings.len() < prev_len
    }

    pub fn unregister_accelerator(&mut self, key: &str, modifiers: &ModifierKeys) -> bool {
        let prev_len = self.bindings.len();
        self.bindings
            .retain(|b| !(b.key.eq_ignore_ascii_case(key) && &b.modifiers == modifiers));
        self.bindings.len() < prev_len
    }

    pub fn set_enabled(&mut self, action: &ActionIntent, enabled: bool) -> bool {
        if let Some(binding) = self.bindings.iter_mut().find(|b| &b.action == action) {
            binding.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn get_binding(&self, action: &ActionIntent) -> Option<&ShortcutBinding> {
        self.bindings.iter().find(|b| &b.action == action)
    }

    pub fn detect_all_conflicts(&self) -> ConflictReport {
        let mut collisions = Vec::new();
        for i in 0..self.bindings.len() {
            if !self.bindings[i].enabled {
                continue;
            }
            for j in (i + 1)..self.bindings.len() {
                if !self.bindings[j].enabled {
                    continue;
                }
                if self.bindings[i]
                    .key
                    .eq_ignore_ascii_case(&self.bindings[j].key)
                    && self.bindings[i].modifiers == self.bindings[j].modifiers
                {
                    let accel = Self::format_accelerator(&self.bindings[j]);
                    collisions.push(ShortcutCollision {
                        accelerator: accel,
                        key: self.bindings[j].key.clone(),
                        modifiers: self.bindings[j].modifiers,
                        existing_action: self.bindings[i].action.clone(),
                        attempted_action: self.bindings[j].action.clone(),
                        message: format!(
                            "Conflict between '{}' and '{}'",
                            self.bindings[i].action, self.bindings[j].action
                        ),
                    });
                }
            }
        }
        ConflictReport { collisions }
    }

    pub fn format_accelerator(binding: &ShortcutBinding) -> String {
        let mut parts = Vec::new();
        if binding.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if binding.modifiers.alt {
            parts.push("Alt");
        }
        if binding.modifiers.shift {
            parts.push("Shift");
        }
        if binding.modifiers.meta {
            parts.push("Meta");
        }
        parts.push(binding.key.as_str());
        parts.join("+")
    }

    pub fn format_accelerator_super(binding: &ShortcutBinding) -> String {
        let mut parts = Vec::new();
        if binding.modifiers.ctrl {
            parts.push("Ctrl");
        }
        if binding.modifiers.alt {
            parts.push("Alt");
        }
        if binding.modifiers.shift {
            parts.push("Shift");
        }
        if binding.modifiers.meta {
            parts.push("Super");
        }
        parts.push(binding.key.as_str());
        parts.join("+")
    }

    pub fn normalize_key(raw_key: &str) -> Result<String> {
        let trimmed = raw_key.trim();
        if trimmed.is_empty() {
            bail!("Empty key string");
        }
        let lower = trimmed.to_ascii_lowercase();
        let normalized = match lower.as_str() {
            "space" => "Space",
            "enter" | "return" => "Enter",
            "tab" => "Tab",
            "esc" | "escape" => "Escape",
            "backspace" => "Backspace",
            "delete" | "del" => "Delete",
            "insert" | "ins" => "Insert",
            "home" => "Home",
            "end" => "End",
            "pageup" | "pgup" => "PageUp",
            "pagedown" | "pgdn" => "PageDown",
            "up" => "Up",
            "down" => "Down",
            "left" => "Left",
            "right" => "Right",
            "minus" | "-" => "-",
            "equal" | "plus" | "=" | "+" => "+",
            "comma" | "," => ",",
            "period" | "." => ".",
            "slash" | "/" => "/",
            "backslash" | "\\" => "\\",
            "semicolon" | ";" => ";",
            "quote" | "'" => "'",
            "grave" | "backquote" | "`" => "`",
            k if k.len() == 1 => {
                return Ok(trimmed.to_ascii_uppercase());
            }
            k if k.starts_with('f') && k[1..].parse::<u8>().is_ok() => {
                return Ok(format!("F{}", &k[1..]));
            }
            _ => trimmed,
        };
        Ok(normalized.to_string())
    }

    pub fn parse_accelerator(accel_str: &str, action: ActionIntent) -> Result<ShortcutBinding> {
        let trimmed = accel_str.trim();
        if trimmed.is_empty() {
            bail!("Empty accelerator string");
        }

        let mut parts: Vec<&str> = trimmed.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            bail!("Empty accelerator string");
        }

        let key_str = parts.pop().unwrap();
        if key_str.is_empty() {
            bail!("Missing key in accelerator: {}", accel_str);
        }

        let key = Self::normalize_key(key_str)?;

        let mut modifiers = ModifierKeys::default();

        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" | "ctl" => modifiers.ctrl = true,
                "alt" | "option" | "opt" => modifiers.alt = true,
                "shift" | "shft" => modifiers.shift = true,
                "meta" | "win" | "cmd" | "command" | "super" | "windows" => modifiers.meta = true,
                _ => bail!("Unknown modifier: {}", part),
            }
        }

        Ok(ShortcutBinding {
            action,
            key,
            modifiers,
            enabled: true,
        })
    }

    pub fn list_bindings(&self) -> &[ShortcutBinding] {
        &self.bindings
    }

    pub fn list_bindings_mut(&mut self) -> &mut [ShortcutBinding] {
        &mut self.bindings
    }

    pub fn match_action(&self, key: &str, modifiers: &ModifierKeys) -> Option<&ActionIntent> {
        self.bindings
            .iter()
            .find(|b| b.matches(key, modifiers))
            .map(|b| &b.action)
    }

    pub fn match_intent(&self, key: &str, modifiers: &ModifierKeys) -> Option<ActionIntent> {
        self.match_action(key, modifiers).cloned()
    }

    pub fn clear(&mut self) {
        self.bindings.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_accelerator_standard_and_aliases() {
        let b1 = ShortcutManager::parse_accelerator("Ctrl+Alt+S", ActionIntent::ToggleSystemProxy)
            .unwrap();
        assert!(b1.modifiers.ctrl && b1.modifiers.alt && !b1.modifiers.shift && !b1.modifiers.meta);
        assert_eq!(b1.key, "S");
        assert_eq!(b1.action, ActionIntent::ToggleSystemProxy);

        let b2 =
            ShortcutManager::parse_accelerator("Super+Shift+X", ActionIntent::ToggleTun).unwrap();
        assert!(!b2.modifiers.ctrl && !b2.modifiers.alt && b2.modifiers.shift && b2.modifiers.meta);
        assert_eq!(b2.key, "X");
        assert_eq!(b2.action, ActionIntent::ToggleTun);

        let b3 = ShortcutManager::parse_accelerator("Command+Option+P", ActionIntent::SwitchMode)
            .unwrap();
        assert!(!b3.modifiers.ctrl && b3.modifiers.alt && !b3.modifiers.shift && b3.modifiers.meta);
        assert_eq!(b3.key, "P");
        assert_eq!(b3.action, ActionIntent::SwitchMode);

        let b4 = ShortcutManager::parse_accelerator("Ctrl+Shift+F5", ActionIntent::TestDelayAll)
            .unwrap();
        assert!(b4.modifiers.ctrl && b4.modifiers.shift);
        assert_eq!(b4.key, "F5");

        let b5 =
            ShortcutManager::parse_accelerator("Win+Alt+Space", ActionIntent::ShowWindow).unwrap();
        assert!(b5.modifiers.meta && b5.modifiers.alt);
        assert_eq!(b5.key, "Space");
    }

    #[test]
    fn test_parse_invalid_accelerators() {
        assert!(ShortcutManager::parse_accelerator("", ActionIntent::ShowWindow).is_err());
        assert!(ShortcutManager::parse_accelerator("   ", ActionIntent::ShowWindow).is_err());
        assert!(ShortcutManager::parse_accelerator("Ctrl+Alt+", ActionIntent::ShowWindow).is_err());
        assert!(
            ShortcutManager::parse_accelerator("InvalidMod+S", ActionIntent::ShowWindow).is_err()
        );
    }

    #[test]
    fn test_format_accelerators() {
        let b = ShortcutBinding {
            action: ActionIntent::ShowWindow,
            key: "P".to_string(),
            modifiers: ModifierKeys {
                ctrl: true,
                alt: false,
                shift: true,
                meta: false,
            },
            enabled: true,
        };
        assert_eq!(ShortcutManager::format_accelerator(&b), "Ctrl+Shift+P");

        let b_super = ShortcutBinding {
            action: ActionIntent::HideWindow,
            key: "X".to_string(),
            modifiers: ModifierKeys {
                ctrl: false,
                alt: false,
                shift: true,
                meta: true,
            },
            enabled: true,
        };
        assert_eq!(
            ShortcutManager::format_accelerator_super(&b_super),
            "Shift+Super+X"
        );
    }

    #[test]
    fn test_register_and_collision_reporting() {
        let mut manager = ShortcutManager::new_default();
        let b1 = ShortcutManager::parse_accelerator("Ctrl+Shift+P", ActionIntent::OpenDashboard)
            .unwrap();
        let b2 =
            ShortcutManager::parse_accelerator("Ctrl+Shift+P", ActionIntent::ToggleProxy).unwrap();

        assert!(manager.register(b1.clone()).is_ok());
        let err = manager.register(b2.clone());
        assert!(err.is_err());
        let err_msg = err.unwrap_err().to_string();
        assert!(err_msg.contains("Shortcut collision detected"));

        let collision = manager.find_collision(&b2).unwrap();
        assert_eq!(collision.accelerator, "Ctrl+Shift+P");
        assert_eq!(collision.existing_action, ActionIntent::OpenDashboard);
        assert_eq!(collision.attempted_action, ActionIntent::ToggleProxy);

        let mut b3 =
            ShortcutManager::parse_accelerator("Ctrl+Shift+P", ActionIntent::ToggleTun).unwrap();
        b3.enabled = false;
        assert!(manager.register(b3).is_ok());
    }

    #[test]
    fn test_register_or_replace_and_unregistration() {
        let mut manager = ShortcutManager::new();
        let b1 = ShortcutManager::parse_accelerator("Ctrl+Alt+S", ActionIntent::ToggleSystemProxy)
            .unwrap();
        assert!(manager.register_or_replace(b1).is_none());

        let b2 = ShortcutManager::parse_accelerator("Ctrl+Alt+S", ActionIntent::ToggleTun).unwrap();
        let replaced = manager.register_or_replace(b2).unwrap();
        assert_eq!(replaced.action, ActionIntent::ToggleSystemProxy);
        assert_eq!(manager.list_bindings().len(), 1);

        assert!(manager.unregister_action(&ActionIntent::ToggleTun));
        assert_eq!(manager.list_bindings().len(), 0);
    }

    #[test]
    fn test_match_action_and_intent() {
        let mut manager = ShortcutManager::new();
        let b = ShortcutManager::parse_accelerator("Ctrl+Alt+P", ActionIntent::SwitchMode).unwrap();
        manager.register(b).unwrap();

        let mods = ModifierKeys {
            ctrl: true,
            alt: true,
            shift: false,
            meta: false,
        };
        assert_eq!(
            manager.match_action("P", &mods),
            Some(&ActionIntent::SwitchMode)
        );
        assert_eq!(
            manager.match_intent("P", &mods),
            Some(ActionIntent::SwitchMode)
        );
        assert_eq!(manager.match_action("Q", &mods), None);

        manager.set_enabled(&ActionIntent::SwitchMode, false);
        assert_eq!(manager.match_action("P", &mods), None);
    }

    #[test]
    fn test_action_intent_strings_and_parsing() {
        assert_eq!(
            ActionIntent::ToggleSystemProxy.as_intent_str(),
            "toggle_system_proxy"
        );
        assert_eq!(ActionIntent::ToggleTun.as_intent_str(), "toggle_tun");
        assert_eq!(ActionIntent::SwitchMode.as_intent_str(), "switch_mode");
        assert_eq!(ActionIntent::TestDelayAll.as_intent_str(), "test_delay_all");
        assert_eq!(ActionIntent::ShowWindow.as_intent_str(), "show_window");
        assert_eq!(ActionIntent::HideWindow.as_intent_str(), "hide_window");

        assert_eq!(
            ActionIntent::from_intent_str("toggle_system_proxy"),
            Some(ActionIntent::ToggleSystemProxy)
        );
        assert_eq!(
            ActionIntent::from_intent_str("toggle_tun"),
            Some(ActionIntent::ToggleTun)
        );
        assert_eq!(
            ActionIntent::from_intent_str("switch_proxy_mode"),
            Some(ActionIntent::SwitchMode)
        );
        assert_eq!(
            ActionIntent::from_intent_str("test_all_delays"),
            Some(ActionIntent::TestDelayAll)
        );
        assert_eq!(
            "show_window".parse::<ActionIntent>().unwrap(),
            ActionIntent::ShowWindow
        );
    }

    #[test]
    fn test_detect_all_conflicts_and_report() {
        let mut manager = ShortcutManager::new();
        manager.bindings.push(ShortcutBinding::new(
            ActionIntent::ShowWindow,
            "K",
            ModifierKeys {
                ctrl: true,
                alt: false,
                shift: false,
                meta: false,
            },
        ));
        manager.bindings.push(ShortcutBinding::new(
            ActionIntent::HideWindow,
            "K",
            ModifierKeys {
                ctrl: true,
                alt: false,
                shift: false,
                meta: false,
            },
        ));

        let report = manager.detect_all_conflicts();
        assert!(report.has_conflicts());
        assert_eq!(report.collisions.len(), 1);
        let summary = report.summary();
        assert!(summary.contains("Found 1 conflict"));
    }
}
