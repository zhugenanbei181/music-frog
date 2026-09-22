//! Neutral design tokens for the Bevy widget layer.
//!
//! Business-agnostic by law: this module knows nothing about infiltrator or
//! mihomo — it is the future extraction candidate shared across projects.
//! The palette mirrors the shared product token contract
//! (`infiltrator_contract::design_tokens::skin_core`, DUAL-15-14) which the
//! Iced shell consumes directly; this crate cannot depend on contract, so the
//! mirror is enforced by `tests/headless/design_token_tests.rs` and the
//! numeric source scan in `scripts/quality/multimodal-shell-guard.py`.
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

/// Which appearance the tokens resolve for. The four skins mirror
/// `infiltrator_contract::theme::ThemeSkin` (the widget layer is
/// business-agnostic by charter and cannot depend on the contract crate);
/// `crates/infiltrator-bevy-ui/tests/headless/theme_parity_tests.rs` asserts
/// the two vocabularies never drift.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeSkin {
    #[default]
    Dark,
    Light,
    /// Eye-care forest appearance (mirrors the Iced `FOREST` token set).
    Forest,
    /// Pitch-black OLED appearance (mirrors the Iced `AMOLED` token set).
    Amoled,
}

impl ThemeSkin {
    pub const ALL: [Self; 4] = [Self::Dark, Self::Light, Self::Forest, Self::Amoled];

    /// Canonical settings value (must equal the contract's spelling).
    pub const fn as_setting(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Forest => "forest",
            Self::Amoled => "amoled",
        }
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

    /// Whether the skin paints a dark canvas (icon tiles and hover washes
    /// tint more strongly on dark tokens).
    pub const fn is_dark(self) -> bool {
        matches!(self, Self::Dark | Self::Amoled)
    }

    /// Next skin in the shell's appearance cycle.
    pub const fn next(self) -> Self {
        Self::ALL[(self.to_index() + 1) % Self::ALL.len()]
    }
}

/// The resolved token set for one appearance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub mode: ThemeSkin,
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
    pub fn for_mode(mode: ThemeSkin) -> Self {
        match mode {
            ThemeSkin::Dark => Self::dark(),
            ThemeSkin::Light => Self::light(),
            ThemeSkin::Forest => Self::forest(),
            ThemeSkin::Amoled => Self::amoled(),
        }
    }

    /// Cold-start theme. Matches the iced dark palette; the live switch seam
    /// is [`crate::switch::ThemeSwitch`].
    pub fn dark() -> Self {
        Self {
            mode: ThemeSkin::Dark,
            window_bg: TokenColor::rgb(0.082, 0.094, 0.102), // #15181A
            surface: TokenColor::rgb(0.145, 0.161, 0.173),   // #25292C
            surface_elevated: TokenColor::rgba(1.0, 1.0, 1.0, 0.07),
            ink: TokenColor::rgb(0.93, 0.95, 0.94), // #EDEFEF
            ink_dim: TokenColor::rgba(0.88, 0.90, 0.92, 0.65),
            accent: TokenColor::rgb(0.12, 0.56, 0.96), // #1E8FF5
            on_accent: TokenColor::rgb(1.0, 1.0, 1.0),
            accent_container: TokenColor::rgb(0.051, 0.224, 0.443), // #0D3971 (iced banner, measured)
            sidebar: TokenColor::rgb(0.106, 0.118, 0.125),          // #1B1E20
            icon_tile: TokenColor::rgba(0.12, 0.56, 0.96, 0.62),    // accent @ 0.62
            hover: TokenColor::rgba(1.0, 1.0, 1.0, 0.08),
            pressed: TokenColor::rgba(1.0, 1.0, 1.0, 0.14),
            border: TokenColor::rgba(0.85, 0.90, 0.95, 0.10),
            success: TokenColor::rgb(0.24, 0.78, 0.44), // #3DC770
            warning: TokenColor::rgb(0.96, 0.62, 0.15), // #F59E26
            danger: TokenColor::rgb(0.96, 0.35, 0.32),  // #F55952
        }
    }

    pub fn light() -> Self {
        Self {
            mode: ThemeSkin::Light,
            window_bg: TokenColor::rgb(0.953, 0.957, 0.957), // #F3F4F4
            surface: TokenColor::rgb(0.988, 0.992, 0.988),   // #FCFDFC
            surface_elevated: TokenColor::rgba(0.0, 0.0, 0.0, 0.05),
            ink: TokenColor::rgb(0.12, 0.15, 0.14), // #1E2623
            ink_dim: TokenColor::rgba(0.24, 0.28, 0.26, 0.65),
            accent: TokenColor::rgb(0.04, 0.44, 0.88), // #0A70E0
            on_accent: TokenColor::rgb(1.0, 1.0, 1.0),
            accent_container: TokenColor::rgb(0.898, 0.910, 0.973), // #E5E8F8 (iced banner, measured)
            sidebar: TokenColor::rgb(0.965, 0.969, 0.969),          // #F6F7F7
            icon_tile: TokenColor::rgba(0.04, 0.44, 0.88, 0.18),    // accent @ 0.18
            hover: TokenColor::rgba(0.0, 0.0, 0.0, 0.06),
            pressed: TokenColor::rgba(0.0, 0.0, 0.0, 0.12),
            border: TokenColor::rgba(0.18, 0.22, 0.20, 0.10),
            success: TokenColor::rgb(0.18, 0.68, 0.38), // #2EAD61
            warning: TokenColor::rgb(0.88, 0.52, 0.05), // #E0850D
            danger: TokenColor::rgb(0.88, 0.24, 0.22),  // #E03D38
        }
    }

    /// Eye-care forest appearance. Values are measured from the Iced
    /// reference token set (`infiltrator-iced/src/view/theme.rs` `FOREST`)
    /// so both surfaces paint the same product language.
    pub fn forest() -> Self {
        Self {
            mode: ThemeSkin::Forest,
            window_bg: TokenColor::rgb(0.937, 0.961, 0.925), // #EFF5EC canvas
            surface: TokenColor::rgb(0.973, 0.984, 0.961),   // #F8FBF5 card
            surface_elevated: TokenColor::rgba(0.341, 0.439, 0.353, 0.09),
            ink: TokenColor::rgb(0.122, 0.208, 0.145), // #1F3525
            ink_dim: TokenColor::rgba(0.341, 0.439, 0.353, 0.75),
            accent: TokenColor::rgb(0.188, 0.435, 0.306), // #306F4E
            on_accent: TokenColor::rgb(1.0, 1.0, 1.0),
            accent_container: TokenColor::rgba(0.188, 0.435, 0.306, 0.14),
            sidebar: TokenColor::rgb(0.851, 0.910, 0.843), // #D9E8D7
            icon_tile: TokenColor::rgba(0.188, 0.435, 0.306, 0.18),
            hover: TokenColor::rgba(0.122, 0.208, 0.145, 0.06),
            pressed: TokenColor::rgba(0.122, 0.208, 0.145, 0.12),
            border: TokenColor::rgba(0.341, 0.439, 0.353, 0.22),
            success: TokenColor::rgb(0.243, 0.490, 0.314), // #3E7D50
            warning: TokenColor::rgb(0.663, 0.439, 0.157), // #A97028
            danger: TokenColor::rgb(0.702, 0.231, 0.275),  // #B33B46
        }
    }

    /// Pitch-black OLED appearance. Values are measured from the Iced
    /// reference token set (`AMOLED`) for the same reason as [`Self::forest`].
    pub fn amoled() -> Self {
        Self {
            mode: ThemeSkin::Amoled,
            window_bg: TokenColor::rgb(0.0, 0.0, 0.0), // #000000
            surface: TokenColor::rgb(0.086, 0.098, 0.110), // #16191C
            surface_elevated: TokenColor::rgba(1.0, 1.0, 1.0, 0.09),
            ink: TokenColor::rgb(0.973, 0.980, 0.988), // #F8FAFC
            ink_dim: TokenColor::rgba(0.90, 0.92, 0.94, 0.68),
            accent: TokenColor::rgb(0.12, 0.56, 0.96), // #1E8FF5
            on_accent: TokenColor::rgb(1.0, 1.0, 1.0),
            accent_container: TokenColor::rgba(0.12, 0.56, 0.96, 0.18),
            sidebar: TokenColor::rgb(0.051, 0.059, 0.067), // #0D0F11
            icon_tile: TokenColor::rgba(0.12, 0.56, 0.96, 0.62),
            hover: TokenColor::rgba(1.0, 1.0, 1.0, 0.08),
            pressed: TokenColor::rgba(1.0, 1.0, 1.0, 0.14),
            border: TokenColor::rgba(0.85, 0.90, 0.95, 0.12),
            success: TokenColor::rgb(0.063, 0.725, 0.506), // #10B981
            warning: TokenColor::rgb(0.96, 0.62, 0.15),    // #F59E26
            danger: TokenColor::rgb(0.96, 0.35, 0.32),     // #F55952
        }
    }
}

/// Spacing scale (px), mirroring the shared contract ladder
/// (`infiltrator_contract::design_tokens::space`: XS=4, SM=8, MD=12, LG=16,
/// XL=20, XXL=24). This crate is business-agnostic by charter and cannot
/// depend on contract, so the mirror is enforced by
/// `tests/headless/design_token_tests.rs` and the numeric scan in
/// `scripts/quality/multimodal-shell-guard.py`.
pub mod space {
    pub const S2: f32 = 2.0;
    pub const S4: f32 = 4.0;
    pub const S6: f32 = 6.0;
    pub const S8: f32 = 8.0;
    pub const S12: f32 = 12.0;
    pub const S16: f32 = 16.0;
    pub const S20: f32 = 20.0;
    pub const S24: f32 = 24.0;
    pub const S32: f32 = 32.0;
}

/// Corner radius scale (px), mirroring `design_tokens::radius`
/// (CARD=16, CONTROL=10). [`SHEET_TOP`] is the surface-only sheet radius.
pub mod radius {
    pub const CARD: f32 = 16.0;
    pub const CONTROL: f32 = 10.0;
    pub const SHEET_TOP: f32 = 16.0;
}

/// Control metrics (px).
pub mod metrics {
    pub const CONTROL_HEIGHT: f32 = 36.0;
    pub const CONTROL_HEIGHT_COMPACT: f32 = 28.0;
    pub const CONTROL_HEIGHT_COMFORTABLE: f32 = 36.0;
    /// Square of a checkbox, radio ring or slider thumb (px).
    pub const CONTROL_SQUARE: f32 = 18.0;
    /// Slider track thickness (px).
    pub const TRACK_HEIGHT: f32 = 4.0;
    /// Hairline border width (px).
    pub const HAIRLINE: f32 = 1.0;
    /// Text-field caret bar width (px) — a 2px slab, the classic hairline-plus.
    pub const CARET_WIDTH: f32 = 2.0;
    /// Minimum mobile touch target dimension (px) for touch accessibility.
    pub const MIN_TOUCH_TARGET: f32 = 48.0;
}

/// Responsive layout breakpoints (px).
///
/// These numbers MIRROR the authoritative values in
/// `infiltrator_contract::responsive_viewport` (`COMPACT_MAX_PX` /
/// `MEDIUM_MAX_PX` / `EXPANDED_MAX_PX`). This crate is business-agnostic by
/// charter and must not depend on contract, so the mirror is enforced by
/// `scripts/quality/responsive-parity-guard.py` rather than by the compiler.
/// See `docs/RESPONSIVE_PARITY_LEDGER.md`.
pub mod breakpoint {
    /// Compact breakpoint boundary: < 600px width (smartphones portrait, narrow splits).
    pub const COMPACT_MAX_PX: f32 = 600.0;
    /// Medium breakpoint boundary: 600px <= width < 840px (tablets, foldables, small desktop).
    pub const MEDIUM_MAX_PX: f32 = 840.0;
    /// Expanded breakpoint boundary: 840px <= width < 1200px (desktop, laptop standard).
    pub const EXPANDED_MAX_PX: f32 = 1200.0;

    /// Backwards-compatible alias for compact breakpoint boundary (600.0 px).
    pub const MOBILE_PX: f32 = COMPACT_MAX_PX;
    /// Backwards-compatible alias for medium breakpoint boundary (840.0 px).
    pub const TABLET_PX: f32 = MEDIUM_MAX_PX;
    /// Backwards-compatible alias for expanded breakpoint boundary (1200.0 px).
    pub const DESKTOP_PX: f32 = EXPANDED_MAX_PX;
}

/// Standardized 4-tier responsive layout breakpoint category.
#[derive(
    bevy::ecs::resource::Resource, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Breakpoint {
    /// Compact layout: width < 600px (smartphones portrait, split screen).
    Compact,
    /// Medium layout: 600px <= width < 840px (tablets, foldables, compact desktop).
    Medium,
    /// Expanded layout: 840px <= width < 1200px (standard desktop, laptop).
    #[default]
    Expanded,
    /// Ultra layout: width >= 1200px (ultrawide monitors, 2K/4K displays).
    Ultra,
}

impl Breakpoint {
    /// Compact boundary: 600.0 px.
    pub const COMPACT_MAX_PX: f32 = breakpoint::COMPACT_MAX_PX;
    /// Medium boundary: 840.0 px.
    pub const MEDIUM_MAX_PX: f32 = breakpoint::MEDIUM_MAX_PX;
    /// Expanded boundary: 1200.0 px.
    pub const EXPANDED_MAX_PX: f32 = breakpoint::EXPANDED_MAX_PX;

    /// Backwards-compatible alias for mobile/compact boundary (600.0 px).
    #[allow(non_upper_case_globals)]
    pub const MOBILE: Breakpoint = Breakpoint::Compact;
    #[allow(non_upper_case_globals)]
    pub const DESKTOP: Breakpoint = Breakpoint::Expanded;
    #[allow(non_upper_case_globals)]
    pub const TABLET: Breakpoint = Breakpoint::Medium;
    #[allow(non_upper_case_globals)]
    pub const Mobile: Breakpoint = Breakpoint::Compact;
    #[allow(non_upper_case_globals)]
    pub const Desktop: Breakpoint = Breakpoint::Expanded;
    #[allow(non_upper_case_globals)]
    pub const Tablet: Breakpoint = Breakpoint::Medium;
    pub const MOBILE_PX: f32 = breakpoint::MOBILE_PX;
    /// Backwards-compatible alias for tablet/medium boundary (840.0 px).
    pub const TABLET_PX: f32 = breakpoint::TABLET_PX;
    /// Backwards-compatible alias for desktop/expanded boundary (1200.0 px).
    pub const DESKTOP_PX: f32 = breakpoint::DESKTOP_PX;

    /// Classify a window or viewport width in pixels into a 4-tier [`Breakpoint`].
    pub fn from_width(width_px: f32) -> Self {
        if width_px < Self::COMPACT_MAX_PX {
            Breakpoint::Compact
        } else if width_px < Self::MEDIUM_MAX_PX {
            Breakpoint::Medium
        } else if width_px < Self::EXPANDED_MAX_PX {
            Breakpoint::Expanded
        } else {
            Breakpoint::Ultra
        }
    }

    /// Whether this breakpoint represents compact layout (<600px).
    pub fn is_compact(&self) -> bool {
        matches!(self, Breakpoint::Compact)
    }

    /// Whether this breakpoint represents medium layout (600px..840px).
    pub fn is_medium(&self) -> bool {
        matches!(self, Breakpoint::Medium)
    }

    /// Whether this breakpoint represents expanded layout (840px..1200px).
    pub fn is_expanded(&self) -> bool {
        matches!(self, Breakpoint::Expanded)
    }

    /// Whether this breakpoint represents ultra layout (>=1200px).
    pub fn is_ultra(&self) -> bool {
        matches!(self, Breakpoint::Ultra)
    }

    /// Backwards-compatible helper: whether this breakpoint represents mobile compact layout (<600px).
    pub fn is_mobile(&self) -> bool {
        self.is_compact()
    }

    /// Backwards-compatible helper: whether this breakpoint represents tablet medium layout (600px..840px).
    pub fn is_tablet(&self) -> bool {
        self.is_medium()
    }

    /// Backwards-compatible helper: whether this breakpoint represents desktop layout (>=840px).
    pub fn is_desktop(&self) -> bool {
        matches!(self, Breakpoint::Expanded | Breakpoint::Ultra)
    }

    /// Recommended sidebar width in pixels for this breakpoint.
    /// Returns `None` for compact mode (sidebar collapsed into bottom nav),
    /// `Some(72.0)` for medium (slim rail mode),
    /// `Some(240.0)` for expanded (standard sidebar),
    /// and `Some(280.0)` for ultra (wide sidebar).
    pub fn sidebar_width_px(&self) -> Option<f32> {
        match self {
            Breakpoint::Compact => None,
            Breakpoint::Medium => Some(72.0),
            Breakpoint::Expanded => Some(240.0),
            Breakpoint::Ultra => Some(280.0),
        }
    }

    /// Default grid column count recommended for this breakpoint.
    pub fn default_grid_columns(&self) -> usize {
        match self {
            Breakpoint::Compact => 1,
            Breakpoint::Medium => 2,
            Breakpoint::Expanded => 3,
            Breakpoint::Ultra => 4,
        }
    }
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
    /// keeps the page titles at 20 — global rescales are off the table.
    pub const DISPLAY: f32 = 22.0;
    pub const HEADING: f32 = 20.0;
    pub const BODY: f32 = 15.0;
    pub const CAPTION: f32 = 12.0;
    pub const MONO: f32 = 13.0;
}

/// WCAG 2.1 relative luminance and color contrast ratio calculations.
pub mod contrast {
    use super::TokenColor;

    /// Calculate linearized channel value per sRGB W3C formula.
    fn linearize_channel(c: f32) -> f32 {
        let c_norm = c.clamp(0.0, 1.0);
        if c_norm <= 0.04045 {
            c_norm / 12.92
        } else {
            ((c_norm + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Compute relative luminance of a color per WCAG 2.1 specification [0.0..1.0].
    pub fn relative_luminance(color: TokenColor) -> f32 {
        let r = linearize_channel(color.r);
        let g = linearize_channel(color.g);
        let b = linearize_channel(color.b);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Compute contrast ratio between two colors [1.0..21.0].
    pub fn contrast_ratio(c1: TokenColor, c2: TokenColor) -> f32 {
        let l1 = relative_luminance(c1);
        let l2 = relative_luminance(c2);
        let lighter = l1.max(l2);
        let darker = l1.min(l2);
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Check if contrast meets WCAG 2.1 AA level for standard text (>= 4.5:1).
    pub fn is_wcag_aa(contrast: f32) -> bool {
        contrast >= 4.5
    }

    /// Check if contrast meets WCAG 2.1 AAA enhanced accessibility level (>= 7.0:1).
    pub fn is_wcag_aaa(contrast: f32) -> bool {
        contrast >= 7.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wcag_contrast_black_and_white() {
        let black = TokenColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let white = TokenColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };

        let ratio = contrast::contrast_ratio(white, black);
        assert!((ratio - 21.0).abs() < 0.1);
        assert!(contrast::is_wcag_aa(ratio));
        assert!(contrast::is_wcag_aaa(ratio));

        let same_ratio = contrast::contrast_ratio(white, white);
        assert!((same_ratio - 1.0).abs() < 1e-4);
        assert!(!contrast::is_wcag_aa(same_ratio));
    }

    #[test]
    fn every_skin_has_its_own_token_set_and_setting_name() {
        for skin in ThemeSkin::ALL {
            let theme = Theme::for_mode(skin);
            assert_eq!(theme.mode, skin);
            assert_eq!(
                Theme::for_mode(ThemeSkin::from_index(skin.to_index())).mode,
                skin
            );
            assert!(theme.window_bg.r.is_finite());
        }
        assert_eq!(ThemeSkin::Dark.as_setting(), "dark");
        assert_eq!(ThemeSkin::Light.as_setting(), "light");
        assert_eq!(ThemeSkin::Forest.as_setting(), "forest");
        assert_eq!(ThemeSkin::Amoled.as_setting(), "amoled");
        assert!(ThemeSkin::Dark.is_dark());
        assert!(ThemeSkin::Amoled.is_dark());
        assert!(!ThemeSkin::Light.is_dark());
        assert!(!ThemeSkin::Forest.is_dark());
    }

    #[test]
    fn forest_and_amoled_have_distinct_canvases() {
        assert_ne!(Theme::forest().window_bg, Theme::light().window_bg);
        assert_ne!(Theme::forest().window_bg, Theme::dark().window_bg);
        assert_eq!(Theme::amoled().window_bg, TokenColor::rgb(0.0, 0.0, 0.0));
        assert_eq!(ThemeSkin::Dark.next(), ThemeSkin::Light);
        assert_eq!(ThemeSkin::Amoled.next(), ThemeSkin::Dark);
    }
}
