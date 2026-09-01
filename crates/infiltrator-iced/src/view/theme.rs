//! Design tokens for the Infiltrator desktop shell.
//!
//! Single source of truth for every color / spacing / radius decision in the
//! UI. Page views and shared components must never hardcode a `Color` — take
//! [`tokens`] (resolved from the active [`iced::Theme`]) and read from the
//! returned [`Tokens`] instead, so light and dark stay equally first-class.
//!
//! Reference aesthetic: modern Electron clash dashboards (Clash Party) —
//! light-gray canvas, white rounded cards with hairline border + soft shadow,
//! iOS-blue accent, generous whitespace; mirrored in dark mode.

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

/// Semantic color roles for one appearance (light or dark).
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
    /// Interactive tint of `accent` (e.g. selected nav row, soft badges).
    pub accent: Color,
    pub accent_soft: Color,
    /// Color of text/icons drawn on top of `accent`.
    pub on_accent: Color,
    /// Text color for accent `BadgeKind` pills (11px semibold on a tinted
    /// accent wash). Light keeps the brand accent; dark needs a lifted blue
    /// because #0A84FF on a 14-16% accent wash over the dark sidebar fails
    /// contrast (WCAG) for text that small.
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

/// iOS system colors, light appearance.
pub const LIGHT: Tokens = Tokens {
    canvas: Color::from_rgb(0.949, 0.949, 0.969),  // #F2F2F7
    sidebar: Color::from_rgb(0.965, 0.965, 0.980), // #F6F6FA
    card_bg: Color::WHITE,
    card_border: Color {
        a: 0.10,
        ..Color::BLACK
    },
    card_shadow: Shadow {
        color: Color {
            a: 0.06,
            ..Color::from_rgb(0.06, 0.09, 0.16)
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    },
    accent: Color::from_rgb(0.0, 0.478, 1.0), // #007AFF iOS blue
    accent_soft: Color {
        a: 0.12,
        ..Color::from_rgb(0.0, 0.478, 1.0)
    },
    on_accent: Color::WHITE,
    // Light appearance keeps the brand accent on badges (existing look).
    badge_accent: Color::from_rgb(0.0, 0.478, 1.0), // #007AFF
    text_primary: Color::from_rgb(0.110, 0.110, 0.118), // #1C1C1E
    text_secondary: Color {
        a: 0.60,
        ..Color::from_rgb(0.235, 0.235, 0.263)
    }, // #3C3C43 @60%
    text_tertiary: Color {
        a: 0.30,
        ..Color::from_rgb(0.235, 0.235, 0.263)
    },
    success: Color::from_rgb(0.204, 0.780, 0.349), // #34C759
    warning: Color::from_rgb(1.0, 0.584, 0.0),     // #FF9500
    danger: Color::from_rgb(1.0, 0.231, 0.188),    // #FF3B30
    chip_bg: Color {
        a: 0.06,
        ..Color::BLACK
    },
    divider: Color {
        a: 0.08,
        ..Color::BLACK
    },
    sidebar_text: Color::from_rgb(0.110, 0.110, 0.118),
    sidebar_text_muted: Color {
        a: 0.55,
        ..Color::from_rgb(0.235, 0.235, 0.263)
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
    }, // iOS gray
    switch_knob: Color::WHITE,
};

/// iOS system colors, dark appearance.
pub const DARK: Tokens = Tokens {
    canvas: Color::from_rgb(0.055, 0.063, 0.078),  // #0E1014
    sidebar: Color::from_rgb(0.086, 0.094, 0.110), // #16181C
    card_bg: Color::from_rgb(0.129, 0.141, 0.161), // #212429
    card_border: Color {
        a: 0.08,
        ..Color::WHITE
    },
    card_shadow: Shadow {
        color: Color {
            a: 0.35,
            ..Color::BLACK
        },
        offset: Vector::new(0.0, 1.0),
        blur_radius: 3.0,
    },
    accent: Color::from_rgb(0.039, 0.518, 1.0), // #0A84FF iOS blue (dark)
    accent_soft: Color {
        a: 0.16,
        ..Color::from_rgb(0.039, 0.518, 1.0)
    },
    on_accent: Color::WHITE,
    // Dark appearance lifts the badge blue: #0A84FF text on the 14-16%
    // accent wash over dark surfaces computes to ~3.6-4.2:1 — below the
    // 4.5:1 WCAG AA bar for 11px text. #6CB0FF on the same wash is >5:1.
    badge_accent: Color::from_rgb(0.424, 0.690, 1.0), // #6CB0FF
    text_primary: Color::WHITE,
    text_secondary: Color {
        a: 0.60,
        ..Color::from_rgb(0.922, 0.922, 0.961)
    }, // #EBEBF5 @60%
    text_tertiary: Color {
        a: 0.30,
        ..Color::from_rgb(0.922, 0.922, 0.961)
    },
    success: Color::from_rgb(0.188, 0.820, 0.345), // #30D158
    warning: Color::from_rgb(1.0, 0.624, 0.039),   // #FF9F0A
    danger: Color::from_rgb(1.0, 0.271, 0.227),    // #FF453A
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
    }, // #1C1C1E
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
    }, // iOS gray
    switch_knob: Color::WHITE,
};

/// Resolve the token set for the active theme.
///
/// Every iced light-family theme maps to [`LIGHT`], everything else (Dark,
/// Nord, Dracula, ...) maps to [`DARK`]. The app itself only ever switches
/// between `Theme::Light` and `Theme::Dark`.
pub fn tokens(theme: &Theme) -> &'static Tokens {
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
