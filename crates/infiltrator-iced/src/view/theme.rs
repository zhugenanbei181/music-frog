//! Design tokens for the Infiltrator desktop shell.
//!
//! Single source of truth for every color / spacing / radius decision in the
//! UI. Page views and shared components must never hardcode a `Color` — take
//! [`tokens`] (resolved from the active [`iced::Theme`]) and read from the
//! returned [`Tokens`] instead, so light, dark, eye-care forest and amoled black stay
//! equally first-class.
//!
//! Reference aesthetics:
//! - Light: soft warm-paper light with low-glare card surfaces.
//! - Dark: deep charcoal/pine night with gentle, eye-friendly contrast.
//! - Forest: taskmanager-inspired eye-care forest green theme (EyeForest).
//! - AMOLED: pitch-black OLED appearance with high-contrast surfaces.

use iced::{Color, Shadow, Theme, Vector};

/// Spacing scale (logical pixels). Use these instead of raw numbers so
/// rhythm stays consistent across pages.
pub const SP_XS: f32 = 4.0;
pub const SP_SM: f32 = 8.0;
pub const SP_MD: f32 = 12.0;
pub const SP_LG: f32 = 16.0;
pub const SP_XL: f32 = 20.0;
pub const SP_XXL: f32 = 24.0;

/// Corner radius scale (logical pixels). [`R_CHIP`] is a "fully rounded"
/// pill: any value ≥ half the chip height renders as a capsule.
pub const R_CARD: f32 = 16.0;
pub const R_CONTROL: f32 = 10.0;
pub const R_CHIP: f32 = 999.0;

/// Font used for latency / bytes / speed numerals. JetBrains Mono has
/// tabular (monospaced) digits by default, so live-updating values do not
/// jitter horizontally. Bundled in `assets/fonts` (SIL OFL 1.1).
pub const MONO: iced::Font = iced::Font {
    family: iced::font::Family::Name("JetBrains Mono"),
    ..iced::Font::DEFAULT
};

/// Semibold face of the bundled Inter family, for titles and emphasized
/// labels. Prefer this over `Weight::Bold` for a Clash-Party-like hierarchy.
pub const FONT_SEMIBOLD: iced::Font = iced::Font {
    weight: iced::font::Weight::Semibold,
    ..iced::Font::DEFAULT
};

/// Medium face of the bundled Inter family, for buttons and controls.
pub const FONT_MEDIUM: iced::Font = iced::Font {
    weight: iced::font::Weight::Medium,
    ..iced::Font::DEFAULT
};

/// Semantic color roles for one appearance (light, dark, forest, or amoled).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    /// Window/page background behind all cards.
    pub canvas: Color,
    /// Sidebar background (slightly offset from the canvas).
    pub sidebar: Color,
    /// Card surface.
    pub card_bg: Color,
    /// 1px hairline around cards.
    pub card_border: Color,
    /// Soft drop shadow under cards.
    pub card_shadow: Shadow,
    /// Elevated drop shadow for floating popovers, menus, and modal dialogs.
    pub floating_shadow: Shadow,
    /// Interactive tint of `accent` (e.g. selected nav row, soft badges).
    pub accent: Color,
    pub accent_soft: Color,
    /// Color of text/icons drawn on top of `accent`.
    pub on_accent: Color,
    /// Text color for accent `BadgeKind` pills (11px semibold on a tinted
    /// accent wash).
    pub badge_accent: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    /// Background of small pill chips (protocol tags, counts).
    pub chip_bg: Color,
    /// Hairline separators inside cards/lists.
    pub divider: Color,
    /// Sidebar row hover / inactive text are derived from these.
    pub sidebar_text: Color,
    pub sidebar_text_muted: Color,
    /// Elevated control surface (segmented control track, inputs).
    pub control_bg: Color,
    /// Toast / HUD bubble: background, text, muted text, hairline border.
    pub overlay: Color,
    pub overlay_text: Color,
    pub overlay_text_muted: Color,
    pub overlay_border: Color,
    /// Toggle switch: off-state track and knob.
    pub switch_track: Color,
    pub switch_knob: Color,
}

/// Soft warm-paper light appearance (anti-glare).
pub const LIGHT: Tokens = Tokens {
    canvas: Color::from_rgb(0.953, 0.957, 0.957),  // #F3F4F4
    sidebar: Color::from_rgb(0.965, 0.969, 0.969), // #F6F7F7
    card_bg: Color::from_rgb(0.988, 0.992, 0.988), // #FCFDFC
    card_border: Color {
        a: 0.10,
        ..Color::from_rgb(0.18, 0.22, 0.20)
    },
    card_shadow: Shadow {
        color: Color {
            a: 0.05,
            ..Color::from_rgb(0.08, 0.12, 0.10)
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.14,
            ..Color::from_rgb(0.08, 0.12, 0.10)
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 12.0,
    },
    accent: Color::from_rgb(0.04, 0.44, 0.88), // #0A70E0
    accent_soft: Color {
        a: 0.12,
        ..Color::from_rgb(0.04, 0.44, 0.88)
    },
    on_accent: Color::WHITE,
    badge_accent: Color::from_rgb(0.04, 0.44, 0.88),
    text_primary: Color::from_rgb(0.12, 0.15, 0.14), // #1E2623
    text_secondary: Color {
        a: 0.65,
        ..Color::from_rgb(0.24, 0.28, 0.26)
    },
    text_tertiary: Color {
        a: 0.38,
        ..Color::from_rgb(0.24, 0.28, 0.26)
    },
    success: Color::from_rgb(0.18, 0.68, 0.38), // #2EAD61
    warning: Color::from_rgb(0.88, 0.52, 0.05), // #E0850D
    danger: Color::from_rgb(0.88, 0.24, 0.22),  // #E03D38
    chip_bg: Color {
        a: 0.06,
        ..Color::BLACK
    },
    divider: Color {
        a: 0.08,
        ..Color::BLACK
    },
    sidebar_text: Color::from_rgb(0.12, 0.15, 0.14),
    sidebar_text_muted: Color {
        a: 0.55,
        ..Color::from_rgb(0.24, 0.28, 0.26)
    },
    control_bg: Color {
        a: 0.05,
        ..Color::BLACK
    },
    overlay: Color {
        a: 0.90,
        ..Color::BLACK
    },
    overlay_text: Color::WHITE,
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    overlay_border: Color {
        a: 0.25,
        ..Color::WHITE
    },
    switch_track: Color {
        a: 0.32,
        ..Color::from_rgb(0.47, 0.47, 0.50)
    },
    switch_knob: Color::WHITE,
};

/// Soft dark appearance (deep balanced charcoal).
pub const DARK: Tokens = Tokens {
    canvas: Color::from_rgb(0.082, 0.094, 0.102),  // #15181A
    sidebar: Color::from_rgb(0.106, 0.118, 0.125), // #1B1E20
    card_bg: Color::from_rgb(0.145, 0.161, 0.173), // #25292C
    card_border: Color {
        a: 0.10,
        ..Color::from_rgb(0.85, 0.90, 0.95)
    },
    card_shadow: Shadow {
        color: Color {
            a: 0.32,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 4.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.55,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 14.0,
    },
    accent: Color::from_rgb(0.12, 0.56, 0.96), // #1E8FF5
    accent_soft: Color {
        a: 0.16,
        ..Color::from_rgb(0.12, 0.56, 0.96)
    },
    on_accent: Color::WHITE,
    badge_accent: Color::from_rgb(0.46, 0.72, 1.0), // #76B8FF
    text_primary: Color::from_rgb(0.93, 0.95, 0.94), // #EDEFEF
    text_secondary: Color {
        a: 0.65,
        ..Color::from_rgb(0.88, 0.90, 0.92)
    },
    text_tertiary: Color {
        a: 0.35,
        ..Color::from_rgb(0.88, 0.90, 0.92)
    },
    success: Color::from_rgb(0.24, 0.78, 0.44), // #3DC770
    warning: Color::from_rgb(0.96, 0.62, 0.15), // #F59E26
    danger: Color::from_rgb(0.96, 0.35, 0.32),  // #F55952
    chip_bg: Color {
        a: 0.10,
        ..Color::WHITE
    },
    divider: Color {
        a: 0.10,
        ..Color::WHITE
    },
    sidebar_text: Color::WHITE,
    sidebar_text_muted: Color {
        a: 0.45,
        ..Color::WHITE
    },
    control_bg: Color {
        a: 0.07,
        ..Color::WHITE
    },
    overlay: Color {
        a: 0.92,
        ..Color::from_rgb(0.11, 0.11, 0.12)
    },
    overlay_text: Color::WHITE,
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    overlay_border: Color {
        a: 0.20,
        ..Color::WHITE
    },
    switch_track: Color {
        a: 0.32,
        ..Color::from_rgb(0.47, 0.47, 0.50)
    },
    switch_knob: Color::WHITE,
};

/// Eye-care forest appearance (EyeForest, TaskForest-inspired).
pub const FOREST: Tokens = Tokens {
    canvas: Color::from_rgb(0.937, 0.961, 0.925),  // #EFF5EC
    sidebar: Color::from_rgb(0.851, 0.910, 0.843), // #D9E8D7
    card_bg: Color::from_rgb(0.973, 0.984, 0.961), // #F8FBF5
    card_border: Color {
        a: 0.22,
        ..Color::from_rgb(0.341, 0.439, 0.353) // #57705A @ 22%
    },
    card_shadow: Shadow {
        color: Color {
            a: 0.06,
            ..Color::from_rgb(0.12, 0.21, 0.14)
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.16,
            ..Color::from_rgb(0.12, 0.21, 0.14)
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 12.0,
    },
    accent: Color::from_rgb(0.188, 0.435, 0.306), // #306F4E
    accent_soft: Color {
        a: 0.14,
        ..Color::from_rgb(0.188, 0.435, 0.306)
    },
    on_accent: Color::WHITE,
    badge_accent: Color::from_rgb(0.188, 0.435, 0.306),
    text_primary: Color::from_rgb(0.122, 0.208, 0.145), // #1F3525
    text_secondary: Color {
        a: 0.75,
        ..Color::from_rgb(0.341, 0.439, 0.353) // #57705A @ 75%
    },
    text_tertiary: Color {
        a: 0.45,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    success: Color::from_rgb(0.243, 0.490, 0.314), // #3E7D50
    warning: Color::from_rgb(0.663, 0.439, 0.157), // #A97028
    danger: Color::from_rgb(0.702, 0.231, 0.275),  // #B33B46
    chip_bg: Color {
        a: 0.12,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    divider: Color {
        a: 0.16,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    sidebar_text: Color::from_rgb(0.122, 0.208, 0.145),
    sidebar_text_muted: Color {
        a: 0.65,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    control_bg: Color {
        a: 0.09,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    overlay: Color {
        a: 0.94,
        ..Color::from_rgb(0.122, 0.208, 0.145)
    },
    overlay_text: Color::from_rgb(0.973, 0.984, 0.961),
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::from_rgb(0.973, 0.984, 0.961)
    },
    overlay_border: Color {
        a: 0.25,
        ..Color::from_rgb(0.973, 0.984, 0.961)
    },
    switch_track: Color {
        a: 0.35,
        ..Color::from_rgb(0.40, 0.48, 0.42)
    },
    switch_knob: Color::WHITE,
};

/// Pure pitch-black appearance optimized for OLED displays and battery savings.
pub const AMOLED: Tokens = Tokens {
    canvas: Color::from_rgb(0.0, 0.0, 0.0),        // #000000
    sidebar: Color::from_rgb(0.051, 0.059, 0.067), // #0D0F11
    card_bg: Color::from_rgb(0.086, 0.098, 0.110), // #16191C
    card_border: Color {
        a: 0.12,
        ..Color::from_rgb(0.85, 0.90, 0.95)
    },
    card_shadow: Shadow {
        color: Color {
            a: 0.45,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 4.0,
    },
    floating_shadow: Shadow {
        color: Color {
            a: 0.75,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 4.0),
        blur_radius: 16.0,
    },
    accent: Color::from_rgb(0.12, 0.56, 0.96), // #1E8FF5
    accent_soft: Color {
        a: 0.18,
        ..Color::from_rgb(0.12, 0.56, 0.96)
    },
    on_accent: Color::WHITE,
    badge_accent: Color::from_rgb(0.46, 0.72, 1.0), // #76B8FF
    text_primary: Color::from_rgb(0.973, 0.980, 0.988), // #F8FAFC
    text_secondary: Color {
        a: 0.68,
        ..Color::from_rgb(0.90, 0.92, 0.94)
    },
    text_tertiary: Color {
        a: 0.38,
        ..Color::from_rgb(0.90, 0.92, 0.94)
    },
    success: Color::from_rgb(0.063, 0.725, 0.506), // #10B981
    warning: Color::from_rgb(0.96, 0.62, 0.15),   // #F59E26
    danger: Color::from_rgb(0.96, 0.35, 0.32),    // #F55952
    chip_bg: Color {
        a: 0.12,
        ..Color::WHITE
    },
    divider: Color {
        a: 0.12,
        ..Color::WHITE
    },
    sidebar_text: Color::from_rgb(0.973, 0.980, 0.988),
    sidebar_text_muted: Color {
        a: 0.50,
        ..Color::WHITE
    },
    control_bg: Color {
        a: 0.09,
        ..Color::WHITE
    },
    overlay: Color {
        a: 0.95,
        ..Color::from_rgb(0.05, 0.06, 0.07)
    },
    overlay_text: Color::from_rgb(0.973, 0.980, 0.988),
    overlay_text_muted: Color {
        a: 0.75,
        ..Color::WHITE
    },
    overlay_border: Color {
        a: 0.22,
        ..Color::WHITE
    },
    switch_track: Color {
        a: 0.35,
        ..Color::from_rgb(0.47, 0.47, 0.50)
    },
    switch_knob: Color::WHITE,
};

/// Predefined accent color presets for custom branding and personalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccentPreset {
    /// Default Infiltrator blue (#0A70E0 in light, #1E8FF5 in dark/amoled).
    #[default]
    Blue,
    /// Vivid emerald green (#10B981).
    Emerald,
    /// Vibrant violet/purple (#8B5CF6).
    Purple,
    /// Warm amber gold (#F59E0B).
    Amber,
    /// Bold crimson red (#EF4444).
    Crimson,
    /// Electric cyan (#06B6D4).
    Cyan,
    /// Passionate rose pink (#F43F5E).
    Rose,
}

impl AccentPreset {
    /// Array of all accent presets for UI enumeration.
    pub const ALL: [AccentPreset; 7] = [
        AccentPreset::Blue,
        AccentPreset::Emerald,
        AccentPreset::Purple,
        AccentPreset::Amber,
        AccentPreset::Crimson,
        AccentPreset::Cyan,
        AccentPreset::Rose,
    ];

    /// Canonical lowercase identifier for the preset.
    pub const fn as_str(&self) -> &'static str {
        match self {
            AccentPreset::Blue => "blue",
            AccentPreset::Emerald => "emerald",
            AccentPreset::Purple => "purple",
            AccentPreset::Amber => "amber",
            AccentPreset::Crimson => "crimson",
            AccentPreset::Cyan => "cyan",
            AccentPreset::Rose => "rose",
        }
    }

    /// Parse an accent preset name or alias.
    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "blue" | "default" | "ocean" => Some(AccentPreset::Blue),
            "emerald" | "green" => Some(AccentPreset::Emerald),
            "purple" | "violet" => Some(AccentPreset::Purple),
            "amber" | "yellow" | "gold" | "orange" => Some(AccentPreset::Amber),
            "crimson" | "red" | "ruby" => Some(AccentPreset::Crimson),
            "cyan" | "teal" | "sky" => Some(AccentPreset::Cyan),
            "rose" | "pink" => Some(AccentPreset::Rose),
            _ => None,
        }
    }

    /// Primary accent color resolved for light vs dark appearance.
    pub const fn color(&self, is_dark: bool) -> Color {
        match self {
            AccentPreset::Blue => {
                if is_dark {
                    Color::from_rgb(0.12, 0.56, 0.96) // #1E8FF5
                } else {
                    Color::from_rgb(0.04, 0.44, 0.88) // #0A70E0
                }
            }
            AccentPreset::Emerald => Color::from_rgb(0.063, 0.725, 0.506), // #10B981
            AccentPreset::Purple => Color::from_rgb(0.545, 0.361, 0.965),  // #8B5CF6
            AccentPreset::Amber => Color::from_rgb(0.961, 0.620, 0.043),   // #F59E0B
            AccentPreset::Crimson => Color::from_rgb(0.937, 0.267, 0.267), // #EF4444
            AccentPreset::Cyan => Color::from_rgb(0.024, 0.714, 0.831),    // #06B6D4
            AccentPreset::Rose => Color::from_rgb(0.957, 0.247, 0.369),    // #F43F5E
        }
    }

    /// Badge / soft tint color for the preset.
    pub const fn badge_color(&self, is_dark: bool) -> Color {
        match self {
            AccentPreset::Blue => {
                if is_dark {
                    Color::from_rgb(0.46, 0.72, 1.0)
                } else {
                    Color::from_rgb(0.04, 0.44, 0.88)
                }
            }
            _ => self.color(is_dark),
        }
    }
}

impl Tokens {
    /// Return a copy of these tokens with a custom accent color applied.
    pub fn with_accent(&self, accent: Color) -> Self {
        let is_dark = self.canvas.r < 0.3 && self.canvas.g < 0.3 && self.canvas.b < 0.3;
        let mut tokens = *self;
        tokens.accent = accent;
        tokens.accent_soft = Color {
            a: if is_dark { 0.18 } else { 0.12 },
            ..accent
        };
        tokens.badge_accent = accent;
        tokens
    }

    /// Return a copy of these tokens with a predefined accent preset applied.
    pub fn with_accent_preset(&self, preset: AccentPreset) -> Self {
        let is_dark = self.canvas.r < 0.3 && self.canvas.g < 0.3 && self.canvas.b < 0.3;
        let mut tokens = *self;
        tokens.accent = preset.color(is_dark);
        tokens.accent_soft = Color {
            a: if is_dark { 0.18 } else { 0.12 },
            ..tokens.accent
        };
        tokens.badge_accent = preset.badge_color(is_dark);
        tokens
    }
}

/// Construct the iced custom Theme representing Forest.
pub fn forest_theme() -> Theme {
    Theme::custom(
        "Forest".to_string(),
        iced::theme::Palette {
            background: FOREST.canvas,
            text: FOREST.text_primary,
            primary: FOREST.accent,
            success: FOREST.success,
            danger: FOREST.danger,
            warning: FOREST.warning,
        },
    )
}

/// Construct the iced custom Theme representing AMOLED Black.
pub fn amoled_theme() -> Theme {
    Theme::custom(
        "AMOLED".to_string(),
        iced::theme::Palette {
            background: AMOLED.canvas,
            text: AMOLED.text_primary,
            primary: AMOLED.accent,
            success: AMOLED.success,
            danger: AMOLED.danger,
            warning: AMOLED.warning,
        },
    )
}

/// Whether the active theme is the Forest theme.
pub fn is_forest(theme: &Theme) -> bool {
    match theme {
        Theme::Custom(custom) => {
            let name = format!("{custom}").to_ascii_lowercase();
            name == "forest" || name == "eyeforest" || name == "eye-forest"
        }
        _ => false,
    }
}

/// Whether the active theme is the AMOLED pitch black theme.
pub fn is_amoled(theme: &Theme) -> bool {
    match theme {
        Theme::Custom(custom) => {
            let name = format!("{custom}").to_ascii_lowercase();
            name == "amoled"
                || name == "black"
                || name == "pitch-black"
                || name == "pitch_black"
                || name == "pitchblack"
        }
        _ => false,
    }
}

/// Resolve the token set for the active theme.
pub fn tokens(theme: &Theme) -> &'static Tokens {
    if is_forest(theme) {
        return &FOREST;
    }
    if is_amoled(theme) {
        return &AMOLED;
    }
    match theme {
        Theme::Light
        | Theme::SolarizedLight
        | Theme::GruvboxLight
        | Theme::CatppuccinLatte
        | Theme::TokyoNightLight
        | Theme::KanagawaLotus => &LIGHT,
        _ => &DARK,
    }
}

/// Canonical string identifier for a theme ("light", "dark", "forest", "amoled").
pub fn theme_to_name(theme: &Theme) -> &'static str {
    if is_forest(theme) {
        "forest"
    } else if is_amoled(theme) {
        "amoled"
    } else if matches!(theme, Theme::Light) {
        "light"
    } else {
        "dark"
    }
}

/// Parse a theme identifier string into an iced Theme.
pub fn theme_from_name(value: &str) -> Theme {
    match value.trim().to_ascii_lowercase().as_str() {
        "forest" | "eyeforest" | "eye-forest" => forest_theme(),
        "amoled" | "black" | "pitch-black" | "pitch_black" | "pitchblack" => amoled_theme(),
        "light" => Theme::Light,
        _ => Theme::Dark,
    }
}

/// Latency quality tiers used to color delay numerals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyTier {
    /// No measurement yet (or the node does not report one).
    Untested,
    /// ≤ 200 ms — green.
    Good,
    /// ≤ 500 ms — orange.
    Mid,
    /// > 500 ms — red.
    Bad,
}

/// Classify a delay measurement in milliseconds.
pub fn latency_tier(ms: Option<u32>) -> LatencyTier {
    match ms {
        None => LatencyTier::Untested,
        Some(ms) if ms <= 200 => LatencyTier::Good,
        Some(ms) if ms <= 500 => LatencyTier::Mid,
        Some(_) => LatencyTier::Bad,
    }
}

/// Color for a delay measurement under the given token set.
pub fn latency_color(t: &Tokens, ms: Option<u32>) -> Color {
    match latency_tier(ms) {
        LatencyTier::Untested => t.text_tertiary,
        LatencyTier::Good => t.success,
        LatencyTier::Mid => t.warning,
        LatencyTier::Bad => t.danger,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_theme_names_and_detection() {
        let light = theme_from_name("light");
        let dark = theme_from_name("dark");
        let forest = theme_from_name("forest");
        let amoled = theme_from_name("amoled");
        let black = theme_from_name("black");

        assert_eq!(theme_to_name(&light), "light");
        assert_eq!(theme_to_name(&dark), "dark");
        assert_eq!(theme_to_name(&forest), "forest");
        assert_eq!(theme_to_name(&amoled), "amoled");
        assert_eq!(theme_to_name(&black), "amoled");

        assert!(!is_forest(&light));
        assert!(!is_forest(&dark));
        assert!(is_forest(&forest));
        assert!(!is_forest(&amoled));

        assert!(!is_amoled(&light));
        assert!(!is_amoled(&dark));
        assert!(!is_amoled(&forest));
        assert!(is_amoled(&amoled));
        assert!(is_amoled(&black));
    }

    #[test]
    fn test_tokens_resolution() {
        let light = theme_from_name("light");
        let dark = theme_from_name("dark");
        let forest = theme_from_name("forest");
        let amoled = theme_from_name("amoled");

        assert_eq!(tokens(&light).canvas, LIGHT.canvas);
        assert_eq!(tokens(&dark).canvas, DARK.canvas);
        assert_eq!(tokens(&forest).canvas, FOREST.canvas);
        assert_eq!(tokens(&amoled).canvas, AMOLED.canvas);
        assert_eq!(tokens(&amoled).canvas, Color::from_rgb(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_floating_shadows() {
        const {
            assert!(LIGHT.floating_shadow.blur_radius > LIGHT.card_shadow.blur_radius);
            assert!(DARK.floating_shadow.blur_radius > DARK.card_shadow.blur_radius);
            assert!(FOREST.floating_shadow.blur_radius > FOREST.card_shadow.blur_radius);
            assert!(AMOLED.floating_shadow.blur_radius > AMOLED.card_shadow.blur_radius);
        }

        const {
            assert!(LIGHT.floating_shadow.color.a > LIGHT.card_shadow.color.a);
            assert!(DARK.floating_shadow.color.a > DARK.card_shadow.color.a);
            assert!(FOREST.floating_shadow.color.a > FOREST.card_shadow.color.a);
            assert!(AMOLED.floating_shadow.color.a > AMOLED.card_shadow.color.a);
        }
    }

    #[test]
    fn test_accent_presets() {
        for preset in AccentPreset::ALL {
            let name = preset.as_str();
            assert_eq!(AccentPreset::from_name(name), Some(preset));

            let modified = DARK.with_accent_preset(preset);
            assert_eq!(modified.accent, preset.color(true));
        }

        let custom_color = Color::from_rgb(0.5, 0.2, 0.8);
        let custom_tokens = AMOLED.with_accent(custom_color);
        assert_eq!(custom_tokens.accent, custom_color);
        assert_eq!(custom_tokens.badge_accent, custom_color);
    }
}
