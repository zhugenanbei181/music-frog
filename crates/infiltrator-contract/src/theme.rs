//! Shared appearance contract: four first-class skins plus a system-follow
//! preference.
//!
//! Both surfaces paint the same four skins (`dark`, `light`, `forest`,
//! `amoled`) and both must honor the `system` preference by resolving it
//! against the live OS appearance. The skin identifiers are the settings-file
//! values, so a typo can no longer silently degrade to dark.

use serde::{Deserialize, Serialize};

/// Canonical camel-free setting name of the system-follow preference.
pub const SYSTEM_SETTING: &str = "system";

/// One paintable appearance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeSkin {
    /// Deep charcoal night appearance.
    #[default]
    Dark,
    /// Soft warm-paper light appearance (anti-glare).
    Light,
    /// Eye-care forest appearance.
    Forest,
    /// Pitch-black OLED appearance.
    Amoled,
}

impl ThemeSkin {
    pub const ALL: [Self; 4] = [Self::Dark, Self::Light, Self::Forest, Self::Amoled];

    /// Canonical settings value.
    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Forest => "forest",
            Self::Amoled => "amoled",
        }
    }

    /// Parse a settings value, accepting the historical aliases the Iced
    /// surface already honored (`eyeforest`, `pitch-black`, …).
    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "dark" | "night" => Some(Self::Dark),
            "light" | "day" => Some(Self::Light),
            "forest" | "eyeforest" | "eye-forest" => Some(Self::Forest),
            "amoled" | "black" | "pitch-black" | "pitch_black" | "pitchblack" => Some(Self::Amoled),
            _ => None,
        }
    }

    /// i18n key of the skin's display name (Iced table; Bevy maps its own).
    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Dark => "theme_dark",
            Self::Light => "theme_light",
            Self::Forest => "theme_forest",
            Self::Amoled => "theme_amoled",
        }
    }

    /// Whether the skin paints a dark canvas.
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::Amoled)
    }

    pub const fn to_index(self) -> usize {
        match self {
            Self::Dark => 0,
            Self::Light => 1,
            Self::Forest => 2,
            Self::Amoled => 3,
        }
    }

    pub const fn from_index(index: usize) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Forest,
            3 => Self::Amoled,
            _ => Self::Dark,
        }
    }

    /// The next skin in the cycle the shell's theme affordances walk.
    pub const fn next(self) -> Self {
        Self::ALL[(self.to_index() + 1) % Self::ALL.len()]
    }
}

/// The user's stored appearance preference: a fixed skin or "follow the OS".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    /// Resolve against the live OS appearance on every change.
    #[default]
    System,
    /// A pinned skin, independent of the OS.
    Fixed(ThemeSkin),
}

impl ThemePreference {
    /// Parse the settings value; unknown values honestly fall back to
    /// [`ThemePreference::System`] (the historical default) instead of
    /// pretending to be dark.
    pub fn from_setting(value: &str) -> Self {
        Self::parse_strict(value).unwrap_or(Self::System)
    }

    /// Strict parse used by the settings write path: `None` when the value is
    /// neither `system` nor a known skin, so a typo is rejected instead of
    /// silently stored.
    pub fn parse_strict(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.eq_ignore_ascii_case(SYSTEM_SETTING) {
            return Some(Self::System);
        }
        ThemeSkin::from_setting(trimmed).map(Self::Fixed)
    }

    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::System => SYSTEM_SETTING,
            Self::Fixed(skin) => skin.as_setting(),
        }
    }

    /// Whether this preference must re-resolve when the OS appearance changes.
    pub const fn follows_system(self) -> bool {
        matches!(self, Self::System)
    }

    /// The skin to paint for a given OS appearance.
    pub const fn resolve(self, system_prefers_dark: bool) -> ThemeSkin {
        match self {
            Self::System => {
                if system_prefers_dark {
                    ThemeSkin::Dark
                } else {
                    ThemeSkin::Light
                }
            }
            Self::Fixed(skin) => skin,
        }
    }

    /// The pinned skin, when the preference is not system-following.
    pub const fn fixed(self) -> Option<ThemeSkin> {
        match self {
            Self::System => None,
            Self::Fixed(skin) => Some(skin),
        }
    }

    /// Cycle the preference through `system → dark → light → forest → amoled`.
    pub const fn next(self) -> Self {
        match self {
            Self::System => Self::Fixed(ThemeSkin::Dark),
            Self::Fixed(skin) => Self::Fixed(skin.next()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_settings_round_trip_with_aliases() {
        for skin in ThemeSkin::ALL {
            assert_eq!(ThemeSkin::from_setting(skin.as_setting()), Some(skin));
            assert_eq!(ThemeSkin::from_index(skin.to_index()), skin);
        }
        assert_eq!(
            ThemeSkin::from_setting("EyeForest"),
            Some(ThemeSkin::Forest)
        );
        assert_eq!(
            ThemeSkin::from_setting("pitch_black"),
            Some(ThemeSkin::Amoled)
        );
        assert_eq!(ThemeSkin::from_setting("solarized"), None);
    }

    #[test]
    fn system_preference_resolves_to_the_os_appearance() {
        let preference = ThemePreference::from_setting("system");
        assert!(preference.follows_system());
        assert_eq!(preference.resolve(true), ThemeSkin::Dark);
        assert_eq!(preference.resolve(false), ThemeSkin::Light);
        assert_eq!(preference.fixed(), None);
    }

    #[test]
    fn fixed_preference_ignores_the_os_appearance() {
        let preference = ThemePreference::from_setting("amoled");
        assert!(!preference.follows_system());
        assert_eq!(preference.resolve(true), ThemeSkin::Amoled);
        assert_eq!(preference.resolve(false), ThemeSkin::Amoled);
        assert_eq!(preference.as_setting(), "amoled");
    }

    #[test]
    fn unknown_and_empty_settings_fall_back_to_system() {
        assert_eq!(ThemePreference::from_setting(""), ThemePreference::System);
        assert_eq!(
            ThemePreference::from_setting("  SYSTEM "),
            ThemePreference::System
        );
        assert_eq!(
            ThemePreference::from_setting("banana"),
            ThemePreference::System
        );
    }

    #[test]
    fn strict_parse_rejects_unknown_values() {
        assert_eq!(
            ThemePreference::parse_strict("system"),
            Some(ThemePreference::System)
        );
        assert_eq!(
            ThemePreference::parse_strict("forest"),
            Some(ThemePreference::Fixed(ThemeSkin::Forest))
        );
        assert_eq!(ThemePreference::parse_strict("banana"), None);
        assert_eq!(ThemePreference::parse_strict(""), None);
    }

    #[test]
    fn preference_cycle_visits_every_skin() {
        let mut preference = ThemePreference::System;
        let mut visited = Vec::new();
        for _ in 0..ThemeSkin::ALL.len() {
            preference = preference.next();
            visited.push(preference.fixed().expect("fixed after first cycle"));
        }
        assert_eq!(visited, ThemeSkin::ALL.to_vec());
        assert_eq!(preference.next().fixed(), Some(ThemeSkin::Dark));
    }
}
