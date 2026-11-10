//! The single place theme tokens become bevy values.
//!
//! [`UiPalette`] is the resolved, bevy-typed snapshot of one [`Theme`].
//! Scenes read it as a `Res<UiPalette>`; nothing else in this crate (or in
//! the frontend crate) is allowed to construct a `Color` from raw numbers.

use crate::theme::{
    REM_BASE, Theme, TokenColor, metrics, mobile_theme_tokens, radius, space, timing,
};
use bevy::color::{Alpha, Color, LinearRgba};
use bevy::ecs::resource::Resource;
use bevy::ui::{Val, ValNum};
use zhuge_ui_tokens::Rgba;

/// Convert a baseline-pixel dimension into a `Val::Rem` so it scales with
/// the root font size (`RemSize`). This is the ONLY sanctioned way to turn a
/// dimension token into a bevy value: `px(...)` is reserved for the runtime
/// measured values and the hairline/caret/touch-floor whitelist documented in
/// `crates/infiltrator-bevy-ui/docs/BEVY_020_UI_CAPABILITIES.md` ①. Mirrors
/// bevy's own `rem()` ergonomics (integer and float literals both accepted);
/// the division by [`REM_BASE`] is what makes the value baseline-relative.
pub fn rem<T: ValNum>(px_at_baseline: T) -> Val {
    Val::Rem(px_at_baseline.val_num_f32() / REM_BASE)
}

/// Resolve one token to a bevy color, channel-exact (asserted by the
/// headless round-trip test).
pub fn theme_color(token: TokenColor) -> Color {
    Color::srgba(token.r, token.g, token.b, token.a)
}

/// The alpha of a semantic ink's soft wash: the ink over the surface it
/// sits on.
pub const SEMANTIC_WASH_ALPHA: f32 = 0.16;

/// The alpha of the wash a **selected navigation row** is painted with.
///
/// The reference states this once: FlClash's sidebar draws its selected
/// destination with `selectedFill = onSurface.withOpacity(0.08)` across the
/// full row plus a narrow primary indicator bar
/// (`plugins/prism_ui/lib/sidebar.dart`), and this repo's Slint surface
/// resolved the same fact into `LumenTheme.nav-selected-fill:
/// text-primary.with_alpha(8%)`. A solid accent capsule was the old Bevy
/// reading: it made the selected destination the loudest thing in the column
/// and turned the row's empty right side — which the desktop reference
/// records as breathing room — into filled pill geometry.
pub const NAV_SELECTION_WASH_ALPHA: f32 = 0.08;

/// The resolved token palette, injected as a resource by
/// [`crate::WidgetsPlugin`].
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct UiPalette {
    pub window_clear: Color,
    pub surface: Color,
    pub surface_elevated: Color,
    pub ink: Color,
    pub ink_dim: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub accent_container: Color,
    pub sidebar: Color,
    pub icon_tile: Color,
    pub hover_bg: Color,
    pub pressed_bg: Color,
    /// Modal / overlay backdrop (shared contract `scrim`).
    pub scrim: Color,
    /// Keyboard focus ring around the focused control.
    pub focus_ring: Color,
    /// Disabled / tertiary ink.
    pub disabled_ink: Color,
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub card_radius_px: f32,
    /// The radius of a **centered** dialog: the contract's modal rung.
    /// A panel-style card keeps [`Self::card_radius_px`].
    pub modal_radius_px: f32,
    pub control_radius_px: f32,
    pub control_height_px: f32,
    pub control_square_px: f32,
    pub track_height_px: f32,
    pub hairline_px: f32,
    pub caret_width_px: f32,
    pub bottom_sheet_radius_px: f32,
    pub touch_target_compact_px: f32,
    pub touch_target_comfortable_px: f32,
    pub bottom_nav_height_px: f32,
    pub top_app_bar_height_px: f32,
    pub screen_margin_mobile_px: f32,
    pub screen_margin_tablet_px: f32,
    pub bottom_nav_bg: Color,
    pub bottom_nav_indicator: Color,
    pub touch_ripple: Color,
    pub swipe_action: Color,
    pub swipe_destructive: Color,
}

impl UiPalette {
    pub fn new(theme: &Theme) -> Self {
        let mobile = mobile_theme_tokens(theme.mode);
        Self {
            window_clear: theme_color(theme.window_bg),
            surface: theme_color(theme.surface),
            surface_elevated: theme_color(theme.surface_elevated),
            ink: theme_color(theme.ink),
            ink_dim: theme_color(theme.ink_dim),
            accent: theme_color(theme.accent),
            on_accent: theme_color(theme.on_accent),
            accent_container: theme_color(theme.accent_container),
            sidebar: theme_color(theme.sidebar),
            icon_tile: theme_color(theme.icon_tile),
            hover_bg: theme_color(theme.hover),
            pressed_bg: theme_color(theme.pressed),
            scrim: theme_color(theme.scrim),
            focus_ring: theme_color(theme.focus_ring),
            disabled_ink: theme_color(theme.disabled_ink),
            border: theme_color(theme.border),
            success: theme_color(theme.success),
            warning: theme_color(theme.warning),
            danger: theme_color(theme.danger),
            card_radius_px: radius::CARD,
            modal_radius_px: radius::MODAL,
            control_radius_px: radius::CONTROL,
            control_height_px: metrics::CONTROL_HEIGHT,
            control_square_px: metrics::CONTROL_SQUARE,
            track_height_px: metrics::TRACK_HEIGHT,
            hairline_px: metrics::HAIRLINE,
            caret_width_px: metrics::CARET_WIDTH,
            bottom_sheet_radius_px: radius::BOTTOM_SHEET,
            touch_target_compact_px: metrics::TOUCH_TARGET_COMPACT,
            touch_target_comfortable_px: metrics::TOUCH_TARGET_COMFORTABLE,
            bottom_nav_height_px: metrics::BOTTOM_NAV_HEIGHT,
            top_app_bar_height_px: metrics::TOP_APP_BAR_HEIGHT,
            screen_margin_mobile_px: space::SCREEN_MARGIN_MOBILE,
            screen_margin_tablet_px: space::SCREEN_MARGIN_TABLET,
            bottom_nav_bg: theme_color(mobile.bottom_nav_bg),
            bottom_nav_indicator: theme_color(mobile.bottom_nav_indicator),
            touch_ripple: theme_color(mobile.touch_ripple),
            swipe_action: theme_color(mobile.swipe_action),
            swipe_destructive: theme_color(mobile.swipe_destructive),
        }
    }

    /// Token-derived colors: the ONLY alpha derivations this layer performs,
    /// all from token inks. Everything else must be a plain token read.
    /// These are methods, not fields, so a [`ThemeSwitch`] (which replaces
    /// the whole resource) re-derives them with zero extra bookkeeping.
    /// Text selection wash: the accent at low opacity (BEVY-010 selection
    /// highlight).
    pub fn selection_fill(&self) -> Color {
        self.accent.with_alpha(0.25)
    }

    /// The wash under a **selected navigation row**: the readable ink at
    /// [`NAV_SELECTION_WASH_ALPHA`], left translucent so it blends with the
    /// column's own ground exactly as the peers do.
    ///
    /// Not [`Self::selection_fill`]: that is a text highlight and a chosen
    /// member, and the reference paints those with the accent. A navigation
    /// row states where the user is, and every reading of it is a quiet wash
    /// plus a narrow indicator — FlClash `onSurface.withOpacity(0.08)`, Slint
    /// `text-primary.with_alpha(8%)`, Iced `accent_soft` (see the module doc
    /// of `nav.rs`). None of them paints a solid accent capsule.
    pub fn nav_selection_fill(&self) -> Color {
        self.ink.with_alpha(NAV_SELECTION_WASH_ALPHA)
    }

    /// Area fill under a chart's upper series (accent wash).
    pub fn chart_fill_up(&self) -> Color {
        self.accent.with_alpha(0.18)
    }

    /// Area fill under a chart's lower series (success wash).
    pub fn chart_fill_down(&self) -> Color {
        self.success.with_alpha(0.14)
    }

    /// A semantic ink's soft wash: the ink at [`SEMANTIC_WASH_ALPHA`] over
    /// the surface it sits on.
    ///
    /// This is the one derivation a *semantic* ink needs: a badge or a chip
    /// that announces a state paints the state's own ink as a wash, so the
    /// state reads without a second colour.
    pub fn ink_wash(&self, ink: Color) -> Color {
        ink.with_alpha(SEMANTIC_WASH_ALPHA)
    }

    /// The opaque card fill: the elevated surface resolved over the base
    /// surface.
    ///
    /// The elevated fill is translucent, so a fade painted in the elevated
    /// colour would composite a second time over the card it covers and
    /// leave a light band. The card's own colour is therefore resolved
    /// once, here, as an opaque value every surface can paint with.
    pub fn card_fill(&self) -> Color {
        self.over_surface(self.surface_elevated)
    }

    /// A translucent fill resolved over the base surface into an opaque
    /// colour: the colour a reader actually sees where that fill sits on
    /// the surface.
    ///
    /// The composite happens in the linear space the renderer blends in:
    /// a translucent fill over the surface is *lighter* there than the
    /// same arithmetic in the tokens' non-linear space would say, so a
    /// fade painted in the wrong space shows a band against the surface
    /// it covers.
    pub fn over_surface(&self, fill: Color) -> Color {
        let base = self.surface.to_linear();
        let top = fill.to_linear();
        let alpha = top.alpha.clamp(0.0, 1.0);
        let mixed = LinearRgba {
            red: top.red * alpha + base.red * (1.0 - alpha),
            green: top.green * alpha + base.green * (1.0 - alpha),
            blue: top.blue * alpha + base.blue * (1.0 - alpha),
            alpha: 1.0,
        };
        let srgba = Color::from(mixed).to_srgba();
        Color::srgba(srgba.red, srgba.green, srgba.blue, 1.0)
    }

    /// The caret blink half-period, carried here so systems never reach into
    /// the token modules directly for timing.
    pub const CARET_BLINK_SECS: f32 = timing::CARET_BLINK_SECS;

    /// Create a palette variation suited for translucent desktop backdrops (Mica / Acrylic / Vibrancy).
    pub fn with_translucent_window_clear(&self, alpha: f32) -> Self {
        let mut copy = *self;
        copy.window_clear = copy.window_clear.with_alpha(alpha.clamp(0.0, 1.0));
        copy
    }
}

/// Convert a Bevy color into the shared resolved RGBA owner.
#[must_use]
pub fn as_rgba(color: Color) -> Rgba {
    let srgb = color.to_srgba();
    Rgba::new(srgb.red, srgb.green, srgb.blue, srgb.alpha)
}

/// A partial theme token patch for localized scoped overrides or emergency alert skins.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThemeTokenPatch {
    pub accent: Option<Color>,
    pub surface: Option<Color>,
    pub surface_elevated: Option<Color>,
    pub danger: Option<Color>,
    pub success: Option<Color>,
    pub border: Option<Color>,
}

impl UiPalette {
    /// Create a patched palette clone applying localized token overrides.
    pub fn with_patch(&self, patch: &ThemeTokenPatch) -> Self {
        let mut patched = *self;
        if let Some(c) = patch.accent {
            patched.accent = c;
        }
        if let Some(c) = patch.surface {
            patched.surface = c;
        }
        if let Some(c) = patch.surface_elevated {
            patched.surface_elevated = c;
        }
        if let Some(c) = patch.danger {
            patched.danger = c;
        }
        if let Some(c) = patch.success {
            patched.success = c;
        }
        if let Some(c) = patch.border {
            patched.border = c;
        }
        patched
    }
}

#[cfg(test)]
mod patch_tests {
    use super::*;

    #[test]
    fn test_palette_token_patch() {
        let theme = Theme::dark();
        let palette = UiPalette::new(&theme);

        let override_accent = Color::srgb(1.0, 0.5, 0.0);
        let patch = ThemeTokenPatch {
            accent: Some(override_accent),
            danger: None,
            ..Default::default()
        };

        let patched = palette.with_patch(&patch);
        assert_eq!(patched.accent, override_accent);
        // Unpatched fields remain identical
        assert_eq!(patched.surface, palette.surface);
        assert_eq!(patched.danger, palette.danger);
    }

    #[test]
    fn test_card_fill_is_the_opaque_card_colour() {
        for theme in [Theme::dark(), Theme::light()] {
            let palette = UiPalette::new(&theme);
            let card = palette.card_fill().to_srgba();

            // The elevated fill is translucent: a fade painted in it would
            // composite twice over the card and show a light band, so the
            // card's own colour is resolved opaque.
            assert_eq!(card.alpha, 1.0);
            assert_ne!(card, palette.surface_elevated.to_srgba());

            // The composite is the elevated fill over the base surface, in
            // the linear space the renderer blends in.
            let base = palette.surface.to_linear();
            let top = palette.surface_elevated.to_linear();
            let mixed = LinearRgba {
                red: top.red * top.alpha + base.red * (1.0 - top.alpha),
                green: top.green * top.alpha + base.green * (1.0 - top.alpha),
                blue: top.blue * top.alpha + base.blue * (1.0 - top.alpha),
                alpha: 1.0,
            };
            let expected = Color::from(mixed).to_srgba();
            assert!((card.red - expected.red).abs() < 1e-5);
            assert!((card.green - expected.green).abs() < 1e-5);
            assert!((card.blue - expected.blue).abs() < 1e-5);
        }
    }
}
