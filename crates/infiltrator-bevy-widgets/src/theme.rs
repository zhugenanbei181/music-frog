//! Neutral design tokens for the Bevy widget layer.
//!
//! Business-agnostic by law: this module knows nothing about infiltrator or
//! mihomo — it is the future extraction candidate shared across projects.
//! The palette values intentionally mirror the iced frontend's iOS design
//! language (iOS blue accent, iOS gray scales, iOS semantic colors) so both
//! surfaces speak one product language without sharing toolkit code.
//!
//! Every product color this layer paints must originate here as a token and
//! reach bevy only through [`crate::palette`] — never as a literal at a call
//! site.

/// One sRGBA token color, channel-exact. The f32 fields are the contract the
/// round-trip test asserts against `Color::srgba`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TokenColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl TokenColor {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }
}

/// Which appearance the tokens resolve for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LightDark {
    #[default]
    Dark,
    Light,
}

/// The resolved token set for one appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub mode: LightDark,
    /// Window backdrop (iced: canvas).
    pub window_bg: TokenColor,
    /// Card / panel fill.
    pub surface: TokenColor,
    /// Idle control fill on a card (slightly recessed).
    pub surface_elevated: TokenColor,
    /// Primary reading ink.
    pub ink: TokenColor,
    /// Dimmed ink (captions, idle labels) — carries its own alpha.
    pub ink_dim: TokenColor,
    /// Accent ink / selected control fill (iOS blue).
    pub accent: TokenColor,
    /// Ink drawn on top of the accent (white in both appearances — the
    /// accent is a mid blue either way).
    pub on_accent: TokenColor,
    /// Accent-tinted container fill (the Overview status banner backdrop).
    /// Measured off the iced reference shots: dark `#0D3971`, light
    /// `#E5E8F8`.
    pub accent_container: TokenColor,
    /// Sidebar rail fill, one step off the window backdrop. Iced reference:
    /// dark `#16181C`, light `#F6F6FA`.
    pub sidebar: TokenColor,
    /// Recessed square behind a semantic icon: the accent at low opacity
    /// over whatever surface it sits on (alpha carries the blend). Alphas
    /// measured off the iced reference tiles (dark ≈ 0.62, light ≈ 0.18 —
    /// the dark rail needs more tint to read at the same contrast).
    pub icon_tile: TokenColor,
    /// Hovered control surface.
    pub hover: TokenColor,
    /// Pressed control surface.
    pub pressed: TokenColor,
    /// Hairline borders.
    pub border: TokenColor,
    pub success: TokenColor,
    pub warning: TokenColor,
    pub danger: TokenColor,
}

impl Theme {
    /// The token set for one appearance — the only constructor runtime
    /// switches may use, so a `ThemeSwitch` can never inject off-token colors.
    pub fn for_mode(mode: LightDark) -> Self {
        match mode {
            LightDark::Dark => Self::dark(),
            LightDark::Light => Self::light(),
        }
    }

    /// Cold-start theme. Matches the iced dark palette; the live switch seam
    /// is [`crate::switch::ThemeSwitch`].
    pub fn dark() -> Self {
        Self {
            mode: LightDark::Dark,
            window_bg: TokenColor::rgb(0.055, 0.063, 0.078), // #0E1014
            surface: TokenColor::rgb(0.129, 0.141, 0.161),   // #212429
            surface_elevated: TokenColor::rgba(1.0, 1.0, 1.0, 0.06),
            ink: TokenColor::rgb(1.0, 1.0, 1.0),
            ink_dim: TokenColor::rgba(0.922, 0.922, 0.961, 0.6),
            accent: TokenColor::rgb(0.039, 0.518, 1.0), // #0A84FF
            on_accent: TokenColor::rgb(1.0, 1.0, 1.0),
            accent_container: TokenColor::rgb(0.051, 0.224, 0.443), // #0D3971 (iced banner, measured)
            sidebar: TokenColor::rgb(0.086, 0.094, 0.110),          // #16181C
            icon_tile: TokenColor::rgba(0.039, 0.518, 1.0, 0.62),   // accent @ 0.62
            hover: TokenColor::rgba(1.0, 1.0, 1.0, 0.08),
            pressed: TokenColor::rgba(1.0, 1.0, 1.0, 0.14),
            border: TokenColor::rgba(1.0, 1.0, 1.0, 0.10),
            success: TokenColor::rgb(0.188, 0.820, 0.345), // #30D158
            warning: TokenColor::rgb(1.0, 0.624, 0.039),   // #FF9F0A
            danger: TokenColor::rgb(1.0, 0.271, 0.227),    // #FF453A
        }
    }

    pub fn light() -> Self {
        Self {
            mode: LightDark::Light,
            window_bg: TokenColor::rgb(0.949, 0.949, 0.969), // #F2F2F7
            surface: TokenColor::rgb(1.0, 1.0, 1.0),         // #FFFFFF
            surface_elevated: TokenColor::rgb(0.949, 0.949, 0.969),
            ink: TokenColor::rgb(0.110, 0.110, 0.118), // #1C1C1E
            ink_dim: TokenColor::rgba(0.235, 0.235, 0.263, 0.6),
            accent: TokenColor::rgb(0.0, 0.478, 1.0), // #007AFF
            on_accent: TokenColor::rgb(1.0, 1.0, 1.0),
            accent_container: TokenColor::rgb(0.898, 0.910, 0.973), // #E5E8F8 (iced banner, measured)
            sidebar: TokenColor::rgb(0.965, 0.965, 0.980),          // #F6F6FA
            icon_tile: TokenColor::rgba(0.0, 0.478, 1.0, 0.18),     // accent @ 0.18
            hover: TokenColor::rgba(0.0, 0.0, 0.0, 0.06),
            pressed: TokenColor::rgba(0.0, 0.0, 0.0, 0.12),
            border: TokenColor::rgba(0.0, 0.0, 0.0, 0.08),
            success: TokenColor::rgb(0.204, 0.780, 0.349), // #34C759
            warning: TokenColor::rgb(1.0, 0.584, 0.0),     // #FF9500
            danger: TokenColor::rgb(1.0, 0.231, 0.188),    // #FF3B30
        }
    }
}

/// Spacing scale (px), mirroring the iced spacing ladder.
pub mod space {
    pub const S4: f32 = 4.0;
    pub const S8: f32 = 8.0;
    pub const S12: f32 = 12.0;
    pub const S16: f32 = 16.0;
}

/// Corner radius scale (px).
pub mod radius {
    pub const CARD: f32 = 12.0;
    pub const CONTROL: f32 = 8.0;
}

/// Control metrics (px).
pub mod metrics {
    pub const CONTROL_HEIGHT: f32 = 36.0;
    /// Square of a checkbox, radio ring or slider thumb (px).
    pub const CONTROL_SQUARE: f32 = 18.0;
    /// Slider track thickness (px).
    pub const TRACK_HEIGHT: f32 = 4.0;
    /// Hairline border width (px).
    pub const HAIRLINE: f32 = 1.0;
    /// Text-field caret bar width (px) — a 2px slab, the classic hairline-plus.
    pub const CARET_WIDTH: f32 = 2.0;
}

/// Runtime timing tokens (seconds).
pub mod timing {
    /// Text-field caret blink half-period: the caret is shown for this long,
    /// hidden for this long. The classic terminal cadence.
    pub const CARET_BLINK_SECS: f32 = 0.53;
}

/// Type scale (px font sizes). The faces themselves are embedded OFL fonts
/// served by [`crate::fonts`].
pub mod type_scale {
    /// One step above [`HEADING`]: the Overview banner's state word (the
    /// iced reference draws it larger than a panel title). Adding a rung
    /// keeps the page titles at 20 — global relcales are off the table.
    pub const DISPLAY: f32 = 22.0;
    pub const HEADING: f32 = 20.0;
    pub const BODY: f32 = 15.0;
    pub const CAPTION: f32 = 12.0;
    pub const MONO: f32 = 13.0;
}
