//! Design tokens for the Infiltrator desktop shell.
//!
//! The canonical numbers live in `infiltrator_contract::design_tokens`
//! (DUAL-15-14); this module resolves them for Iced and owns the surface-only
//! decoration (shadows, hover washes, tertiary ink). Page views and shared
//! components must never hardcode a `Color` — take [`tokens`] (resolved from
//! the active [`iced::Theme`]) and read from the returned [`Tokens`] instead,
//! so light, dark, eye-care forest and amoled black stay equally first-class.
//!
//! Reference aesthetics:
//! - Light: soft warm-paper light with low-glare card surfaces.
//! - Dark: deep charcoal/pine night with gentle, eye-friendly contrast.
//! - Forest: taskmanager-inspired eye-care forest green theme (EyeForest).
//! - AMOLED: pitch-black OLED appearance with high-contrast surfaces.

use iced::{Color, Shadow, Theme, Vector};
use infiltrator_contract::design_tokens::{RgbaToken, SkinCorePalette, skin_core};
use infiltrator_contract::theme::ThemeSkin;

/// Resolve a shared token into an Iced color.
const fn token_color(token: RgbaToken) -> Color {
    Color {
        r: token.r,
        g: token.g,
        b: token.b,
        a: token.a,
    }
}

const LIGHT_CORE: SkinCorePalette = skin_core(ThemeSkin::Light);
const DARK_CORE: SkinCorePalette = skin_core(ThemeSkin::Dark);
const FOREST_CORE: SkinCorePalette = skin_core(ThemeSkin::Forest);
const AMOLED_CORE: SkinCorePalette = skin_core(ThemeSkin::Amoled);

/// Spacing scale (logical pixels), from the shared contract ladder. Use these
/// instead of raw numbers so rhythm stays consistent across pages.
pub const SP_XS: f32 = infiltrator_contract::design_tokens::space::XS;
pub const SP_SM: f32 = infiltrator_contract::design_tokens::space::SM;
pub const SP_MD: f32 = infiltrator_contract::design_tokens::space::MD;
pub const SP_LG: f32 = infiltrator_contract::design_tokens::space::LG;
pub const SP_XL: f32 = infiltrator_contract::design_tokens::space::XL;
pub const SP_XXL: f32 = infiltrator_contract::design_tokens::space::XXL;

/// Corner radius scale (logical pixels). [`R_CHIP`] is a "fully rounded"
/// pill: any value ≥ half the chip height renders as a capsule.
pub const R_CARD: f32 = infiltrator_contract::design_tokens::radius::CARD;
pub const R_CONTROL: f32 = infiltrator_contract::design_tokens::radius::CONTROL;
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
    canvas: token_color(LIGHT_CORE.canvas),
    sidebar: token_color(LIGHT_CORE.sidebar),
    card_bg: token_color(LIGHT_CORE.card),
    card_border: token_color(LIGHT_CORE.card_border),
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
    accent: token_color(LIGHT_CORE.accent),
    accent_soft: Color {
        a: 0.12,
        ..token_color(LIGHT_CORE.accent)
    },
    on_accent: token_color(LIGHT_CORE.on_accent),
    badge_accent: Color::from_rgb(0.04, 0.44, 0.88),
    text_primary: token_color(LIGHT_CORE.ink),
    text_secondary: token_color(LIGHT_CORE.ink_dim),
    text_tertiary: Color {
        a: 0.38,
        ..Color::from_rgb(0.24, 0.28, 0.26)
    },
    success: token_color(LIGHT_CORE.success),
    warning: token_color(LIGHT_CORE.warning),
    danger: token_color(LIGHT_CORE.danger),
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
    control_bg: token_color(LIGHT_CORE.control_bg),
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
    canvas: token_color(DARK_CORE.canvas),
    sidebar: token_color(DARK_CORE.sidebar),
    card_bg: token_color(DARK_CORE.card),
    card_border: token_color(DARK_CORE.card_border),
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
    accent: token_color(DARK_CORE.accent),
    accent_soft: Color {
        a: 0.16,
        ..token_color(DARK_CORE.accent)
    },
    on_accent: token_color(DARK_CORE.on_accent),
    badge_accent: Color::from_rgb(0.46, 0.72, 1.0), // #76B8FF
    text_primary: token_color(DARK_CORE.ink),
    text_secondary: token_color(DARK_CORE.ink_dim),
    text_tertiary: Color {
        a: 0.35,
        ..Color::from_rgb(0.88, 0.90, 0.92)
    },
    success: token_color(DARK_CORE.success),
    warning: token_color(DARK_CORE.warning),
    danger: token_color(DARK_CORE.danger),
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
    control_bg: token_color(DARK_CORE.control_bg),
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
    canvas: token_color(FOREST_CORE.canvas),
    sidebar: token_color(FOREST_CORE.sidebar),
    card_bg: token_color(FOREST_CORE.card),
    card_border: token_color(FOREST_CORE.card_border),
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
    accent: token_color(FOREST_CORE.accent),
    accent_soft: Color {
        a: 0.14,
        ..token_color(FOREST_CORE.accent)
    },
    on_accent: token_color(FOREST_CORE.on_accent),
    badge_accent: Color::from_rgb(0.188, 0.435, 0.306),
    text_primary: token_color(FOREST_CORE.ink),
    text_secondary: token_color(FOREST_CORE.ink_dim),
    text_tertiary: Color {
        a: 0.45,
        ..Color::from_rgb(0.341, 0.439, 0.353)
    },
    success: token_color(FOREST_CORE.success),
    warning: token_color(FOREST_CORE.warning),
    danger: token_color(FOREST_CORE.danger),
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
    control_bg: token_color(FOREST_CORE.control_bg),
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
    canvas: token_color(AMOLED_CORE.canvas),
    sidebar: token_color(AMOLED_CORE.sidebar),
    card_bg: token_color(AMOLED_CORE.card),
    card_border: token_color(AMOLED_CORE.card_border),
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
    accent: token_color(AMOLED_CORE.accent),
    accent_soft: Color {
        a: 0.18,
        ..token_color(AMOLED_CORE.accent)
    },
    on_accent: token_color(AMOLED_CORE.on_accent),
    badge_accent: Color::from_rgb(0.46, 0.72, 1.0), // #76B8FF
    text_primary: token_color(AMOLED_CORE.ink),
    text_secondary: token_color(AMOLED_CORE.ink_dim),
    text_tertiary: Color {
        a: 0.38,
        ..Color::from_rgb(0.90, 0.92, 0.94)
    },
    success: token_color(AMOLED_CORE.success),
    warning: token_color(AMOLED_CORE.warning),
    danger: token_color(AMOLED_CORE.danger),
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
    control_bg: token_color(AMOLED_CORE.control_bg),
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
    theme_for_skin_name(theme).as_setting()
}

/// Which shared skin a painted [`Theme`] corresponds to.
pub fn theme_for_skin_name(theme: &Theme) -> infiltrator_contract::theme::ThemeSkin {
    if is_forest(theme) {
        infiltrator_contract::theme::ThemeSkin::Forest
    } else if is_amoled(theme) {
        infiltrator_contract::theme::ThemeSkin::Amoled
    } else if matches!(theme, Theme::Light) {
        infiltrator_contract::theme::ThemeSkin::Light
    } else {
        infiltrator_contract::theme::ThemeSkin::Dark
    }
}

/// Paint one shared skin. The single mapping from the shared vocabulary to
/// the Iced toolkit theme.
pub fn theme_for_skin(skin: infiltrator_contract::theme::ThemeSkin) -> Theme {
    match skin {
        infiltrator_contract::theme::ThemeSkin::Dark => Theme::Dark,
        infiltrator_contract::theme::ThemeSkin::Light => Theme::Light,
        infiltrator_contract::theme::ThemeSkin::Forest => forest_theme(),
        infiltrator_contract::theme::ThemeSkin::Amoled => amoled_theme(),
    }
}

/// Parse a theme identifier string into an iced Theme. The shared contract
/// owns the accepted spellings; an unknown value honestly falls back to the
/// cold-start dark skin.
pub fn theme_from_name(value: &str) -> Theme {
    match infiltrator_contract::theme::ThemeSkin::from_setting(value) {
        Some(skin) => theme_for_skin(skin),
        None => Theme::Dark,
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
#[path = "../../tests/gui/view_theme_tests.rs"]
mod tests;
