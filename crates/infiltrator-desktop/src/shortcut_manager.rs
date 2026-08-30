use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ModifierKeys {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    ToggleProxy,
    OpenDashboard,
    SwitchNextProfile,
    FlushDnsCache,
    Custom(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBinding {
    pub action: KeyAction,
    pub key: String,
    pub modifiers: ModifierKeys,
    pub enabled: bool,
}

pub struct ShortcutManager {
    bindings: Vec<ShortcutBinding>,
}

impl ShortcutManager {
    pub fn new_default() -> Self {
        Self { bindings: Vec::new() }
    }

    pub fn register(&mut self, binding: ShortcutBinding) -> Result<()> {
        if binding.enabled {
            for existing in &self.bindings {
                if existing.enabled
                    && existing.key.eq_ignore_ascii_case(&binding.key)
                    && existing.modifiers == binding.modifiers
                {
                    bail!("Shortcut collision detected");
                }
            }
        }
        self.bindings.push(binding);
        Ok(())
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

    pub fn parse_accelerator(accel_str: &str, action: KeyAction) -> Result<ShortcutBinding> {
        let mut parts: Vec<&str> = accel_str.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            bail!("Empty accelerator string");
        }
        let key = parts.pop().unwrap().to_uppercase();
        
        let mut modifiers = ModifierKeys {
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };

        for part in parts {
            match part.to_lowercase().as_str() {
                "ctrl" | "control" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "meta" | "win" | "cmd" | "super" => modifiers.meta = true,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_accelerator() {
        let binding = ShortcutManager::parse_accelerator("Ctrl+Alt+S", KeyAction::ToggleProxy).unwrap();
        assert!(binding.modifiers.ctrl);
        assert!(binding.modifiers.alt);
        assert!(!binding.modifiers.shift);
        assert!(!binding.modifiers.meta);
        assert_eq!(binding.key, "S");
        assert_eq!(binding.action, KeyAction::ToggleProxy);
        assert!(binding.enabled);
    }

    #[test]
    fn test_format_accelerator() {
        let binding = ShortcutBinding {
            action: KeyAction::OpenDashboard,
            key: "P".to_string(),
            modifiers: ModifierKeys {
                ctrl: true,
                alt: false,
                shift: true,
                meta: false,
            },
            enabled: true,
        };
        assert_eq!(ShortcutManager::format_accelerator(&binding), "Ctrl+Shift+P");
    }

    #[test]
    fn test_register_and_collision() {
        let mut manager = ShortcutManager::new_default();
        let binding1 = ShortcutManager::parse_accelerator("Ctrl+Shift+P", KeyAction::OpenDashboard).unwrap();
        let binding2 = ShortcutManager::parse_accelerator("Ctrl+Shift+P", KeyAction::ToggleProxy).unwrap();
        
        assert!(manager.register(binding1).is_ok());
        assert!(manager.register(binding2).is_err());
    }

    #[test]
    fn test_disabled_collision() {
        let mut manager = ShortcutManager::new_default();
        let mut binding1 = ShortcutManager::parse_accelerator("Ctrl+Shift+P", KeyAction::OpenDashboard).unwrap();
        binding1.enabled = false;
        let binding2 = ShortcutManager::parse_accelerator("Ctrl+Shift+P", KeyAction::ToggleProxy).unwrap();
        
        assert!(manager.register(binding1).is_ok());
        assert!(manager.register(binding2).is_ok()); // Should not collide since binding1 is disabled
    }

    #[test]
    fn test_new_default() {
        let manager = ShortcutManager::new_default();
        assert_eq!(manager.list_bindings().len(), 0);
    }
}
