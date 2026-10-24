//! Design tokens for the Infiltrator desktop shell.
//!
//! The canonical numbers live in `infiltrator_contract::design_tokens`
//! (DUAL-15-14); this module resolves them for Iced — the core palette, the
//! interaction palette (scrim / hover / pressed / focus ring / disabled ink),
//! the spacing and radius ladders and the hairline. This module still owns the
//! surface-only decoration (shadows, accent-tinted button derivations) because
//! those are not programmable on both toolkits.
//! Page views and shared components must never hardcode a `Color` — take
//! [`tokens`] (resolved from the active [`iced::Theme`]) and read from the
//! returned [`Tokens`] instead, so light, dark, eye-care forest and amoled
//! black stay equally first-class.
//!
//! Reference aesthetics:
//! - Light: soft warm-paper light with low-glare card surfaces.
//! - Dark: deep charcoal/pine night with gentle, eye-friendly contrast.
//! - Forest: taskmanager-inspired eye-care forest green theme (EyeForest).
//! - AMOLED: pitch-black OLED appearance with high-contrast surfaces.

use iced::font::{Family, Weight};
use iced::gradient::{ColorStop, Linear};
use iced::theme::Palette;
use iced::{Color, Gradient, Radians, Shadow, Theme};

use infiltrator_contract::design_tokens::{metrics, radius, space};
use infiltrator_contract::theme::ThemeSkin;

/// Spacing scale (logical pixels), from the shared contract ladder. Use these
/// instead of raw numbers so rhythm stays consistent across pages.
pub const SP_XS: f32 = space::XS;
pub const SP_SM: f32 = space::SM;
pub const SP_MD: f32 = space::MD;
pub const SP_LG: f32 = space::LG;
pub const SP_XL: f32 = space::XL;
pub const SP_XXL: f32 = space::XXL;

/// Corner radius scale (logical pixels), mirroring the shared contract ladder.
pub const R_XS: f32 = radius::XS;
pub const R_SM: f32 = radius::SM;
pub const R_CONTROL: f32 = radius::CONTROL;
pub const R_CARD: f32 = radius::CARD;
pub const R_MODAL: f32 = radius::MODAL;
pub const R_PILL: f32 = radius::PILL;
/// Alias for [`R_PILL`]: fully rounded pill capsule.
pub const R_CHIP: f32 = R_PILL;

/// Hairline border width (logical pixels), from the shared contract metrics
/// (`metrics::HAIRLINE`). Every 1px border in the shell draws with this
/// constant so the Iced hairline and the Bevy widget mirror can never drift.
pub const HAIRLINE: f32 = metrics::HAIRLINE;

/// Shared typography scale (logical pixels), mirroring `design_tokens::type_scale`.
pub mod type_scale {
    use infiltrator_contract::design_tokens::type_scale;

    pub const DISPLAY: f32 = type_scale::DISPLAY;
    pub const TITLE: f32 = type_scale::TITLE;
    pub const HEADING: f32 = type_scale::HEADING;
    pub const BODY: f32 = type_scale::BODY;
    pub const CAPTION: f32 = type_scale::CAPTION;
    pub const TAG: f32 = type_scale::TAG;
    pub const MONO: f32 = type_scale::MONO;
}

/// Font used for latency / bytes / speed numerals. JetBrains Mono has
/// tabular (monospaced) digits by default, so live-updating values do not
/// jitter horizontally. Bundled in `assets/fonts` (SIL OFL 1.1).
pub const MONO: iced::Font = iced::Font {
    family: Family::Name("JetBrains Mono"),
    ..iced::Font::DEFAULT
};

/// Semibold face of the bundled Inter family, for titles and emphasized
/// labels. Prefer this over `Weight::Bold` for a Clash-Party-like hierarchy.
pub const FONT_SEMIBOLD: iced::Font = iced::Font {
    weight: Weight::Semibold,
    ..iced::Font::DEFAULT
};

/// Medium face of the bundled Inter family, for buttons and controls.
pub const FONT_MEDIUM: iced::Font = iced::Font {
    weight: Weight::Medium,
    ..iced::Font::DEFAULT
};

/// The four skin constants, composed from the shared contract palettes.
pub mod skins;

/// Dual-layer physical shadow: key light (contact crisp shadow) + ambient light (soft diffuse aura).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DualShadow {
    /// Contact key light: crisp silhouette, small blur, tight offset.
    pub key: Shadow,
    /// Ambient diffuse light: wide blur, subtle alpha, soft ambient falloff.
    pub ambient: Shadow,
}

impl DualShadow {
    pub const fn new(key: Shadow, ambient: Shadow) -> Self {
        Self { key, ambient }
    }
}

/// A soft two-stop linear gradient (#120): `top` fading into `bottom` along
/// `angle`. The official cap is eight stops; themed washes stay at two so
/// the wgpu and tiny-skia renderers composite them identically.
const fn soft_wash(angle: Radians, top: Color, bottom: Color) -> Gradient {
    Gradient::Linear(Linear {
        angle,
        stops: [
            Some(ColorStop {
                offset: 0.0,
                color: top,
            }),
            Some(ColorStop {
                offset: 1.0,
                color: bottom,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        ],
    })
}

/// The typed elevation ladder (#121): surfaces ask by tier, so the shadow
/// physics (blur, alpha, offset) has exactly one authority per skin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    /// A resting in-flow card.
    Card,
    /// A hovered / lifted card: one tier up from [`Elevation::Card`].
    Lifted,
    /// A floating popover, menu, or modal dialog.
    Floating,
}

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
    /// Dual-layer composite shadow for cards (Key Light + Ambient Light).
    pub card_dual_shadow: DualShadow,
    /// Dual-layer composite shadow for floating modals & dialogs (Key Light + Ambient Light).
    pub floating_dual_shadow: DualShadow,
    /// Top-bar / hero wash (#120): a soft two-stop vertical gradient from the
    /// sidebar tone into the canvas tone.
    pub hero_wash: Gradient,
    /// Control wash (#120): a subtle top sheen over the control surface.
    pub control_wash: Gradient,
    /// Hairline highlight on gradient surfaces (#119): the card hairline
    /// blended toward white.
    pub highlight_border: Color,
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
    /// Tertiary / disabled ink (hints, placeholders, disabled labels). From
    /// the shared interaction palette so both surfaces dim alike.
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
    /// Neutral control hover wash (shared interaction token).
    pub hover: Color,
    /// Neutral control pressed wash; always stronger than [`Self::hover`].
    pub pressed: Color,
    /// Keyboard focus ring around a focused control.
    pub focus_ring: Color,
    /// Modal / overlay backdrop painted over page content.
    pub scrim: Color,
    /// Toast / HUD bubble: background, text, muted text, hairline border.
    pub overlay: Color,
    pub overlay_text: Color,
    pub overlay_text_muted: Color,
    pub overlay_border: Color,
    /// Toggle switch: off-state track and knob.
    pub switch_track: Color,
    pub switch_knob: Color,
}

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
        Palette {
            background: skins::FOREST.canvas,
            text: skins::FOREST.text_primary,
            primary: skins::FOREST.accent,
            success: skins::FOREST.success,
            danger: skins::FOREST.danger,
            warning: skins::FOREST.warning,
        },
    )
}

/// Construct the iced custom Theme representing AMOLED Black.
pub fn amoled_theme() -> Theme {
    Theme::custom(
        "AMOLED".to_string(),
        Palette {
            background: skins::AMOLED.canvas,
            text: skins::AMOLED.text_primary,
            primary: skins::AMOLED.accent,
            success: skins::AMOLED.success,
            danger: skins::AMOLED.danger,
            warning: skins::AMOLED.warning,
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
impl Tokens {
    /// The dual-layer shadow of one elevation tier (#121): the single
    /// authority every dual-layer surface reads its physics from.
    pub const fn elevation_shadow(&self, tier: Elevation) -> DualShadow {
        match tier {
            Elevation::Card => self.card_dual_shadow,
            Elevation::Lifted | Elevation::Floating => self.floating_dual_shadow,
        }
    }
}

pub fn tokens(theme: &Theme) -> &'static Tokens {
    if is_forest(theme) {
        return &skins::FOREST;
    }
    if is_amoled(theme) {
        return &skins::AMOLED;
    }
    match theme {
        Theme::Light
        | Theme::SolarizedLight
        | Theme::GruvboxLight
        | Theme::CatppuccinLatte
        | Theme::TokyoNightLight
        | Theme::KanagawaLotus => &skins::LIGHT,
        _ => &skins::DARK,
    }
}

/// Canonical string identifier for a theme ("light", "dark", "forest", "amoled").
pub fn theme_to_name(theme: &Theme) -> &'static str {
    theme_for_skin_name(theme).as_setting()
}

/// Which shared skin a painted [`Theme`] corresponds to.
pub fn theme_for_skin_name(theme: &Theme) -> ThemeSkin {
    if is_forest(theme) {
        ThemeSkin::Forest
    } else if is_amoled(theme) {
        ThemeSkin::Amoled
    } else if matches!(theme, Theme::Light) {
        ThemeSkin::Light
    } else {
        ThemeSkin::Dark
    }
}

/// Paint one shared skin. The single mapping from the shared vocabulary to
/// the Iced toolkit theme.
pub fn theme_for_skin(skin: ThemeSkin) -> Theme {
    match skin {
        ThemeSkin::Dark => Theme::Dark,
        ThemeSkin::Light => Theme::Light,
        ThemeSkin::Forest => forest_theme(),
        ThemeSkin::Amoled => amoled_theme(),
    }
}

/// Parse a theme identifier string into an iced Theme. The shared contract
/// owns the accepted spellings; an unknown value honestly falls back to the
/// cold-start dark skin.
pub fn theme_from_name(value: &str) -> Theme {
    match ThemeSkin::from_setting(value) {
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
