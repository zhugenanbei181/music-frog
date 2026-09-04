//! The single place theme tokens become bevy values.
//!
//! [`UiPalette`] is the resolved, bevy-typed snapshot of one [`Theme`].
//! Scenes read it as a `Res<UiPalette>`; nothing else in this crate (or in
//! the frontend crate) is allowed to construct a `Color` from raw numbers.

use bevy::color::Alpha;
use bevy::color::Color;
use bevy::ecs::resource::Resource;

use crate::theme::{Theme, TokenColor, metrics, radius, timing, type_scale};

/// Resolve one token to a bevy color, channel-exact (asserted by the
/// headless round-trip test).
pub fn theme_color(token: TokenColor) -> Color {
    Color::srgba(token.r, token.g, token.b, token.a)
}

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
    pub border: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub card_radius_px: f32,
    pub control_radius_px: f32,
    pub control_height_px: f32,
    pub control_square_px: f32,
    pub track_height_px: f32,
    pub hairline_px: f32,
    pub caret_width_px: f32,
    pub display_font_px: f32,
    pub heading_font_px: f32,
    pub body_font_px: f32,
    pub caption_font_px: f32,
    pub mono_font_px: f32,
}

impl UiPalette {
    pub fn new(theme: &Theme) -> Self {
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
            border: theme_color(theme.border),
            success: theme_color(theme.success),
            warning: theme_color(theme.warning),
            danger: theme_color(theme.danger),
            card_radius_px: radius::CARD,
            control_radius_px: radius::CONTROL,
            control_height_px: metrics::CONTROL_HEIGHT,
            control_square_px: metrics::CONTROL_SQUARE,
            track_height_px: metrics::TRACK_HEIGHT,
            hairline_px: metrics::HAIRLINE,
            caret_width_px: metrics::CARET_WIDTH,
            display_font_px: type_scale::DISPLAY,
            heading_font_px: type_scale::HEADING,
            body_font_px: type_scale::BODY,
            caption_font_px: type_scale::CAPTION,
            mono_font_px: type_scale::MONO,
        }
    }

    /// Token-derived colors: the ONLY alpha derivations this layer performs,
    /// all from token inks. Everything else must be a plain token read.
    /// These are methods, not fields, so a [`ThemeSwitch`] (which replaces
    /// the whole resource) re-derives them with zero extra bookkeeping.
    /// The light scrim behind menus and popovers: the window token held at
    /// half strength — the overlay darkens by the window's own tone, never
    /// by a raw gray.
    pub fn scrim(&self) -> Color {
        self.window_clear.with_alpha(0.5)
    }

    /// Text selection wash: the accent at low opacity (BEVY-010 selection
    /// highlight).
    pub fn selection_fill(&self) -> Color {
        self.accent.with_alpha(0.25)
    }

    /// Area fill under a chart's upper series (accent wash).
    pub fn chart_fill_up(&self) -> Color {
        self.accent.with_alpha(0.18)
    }

    /// Area fill under a chart's lower series (success wash).
    pub fn chart_fill_down(&self) -> Color {
        self.success.with_alpha(0.14)
    }

    /// The caret blink half-period, carried here so systems never reach into
    /// the token modules directly for timing.
    pub const CARET_BLINK_SECS: f32 = timing::CARET_BLINK_SECS;
}

/// Calculate relative luminance of a color according to WCAG 2.1 standard.
pub fn calculate_relative_luminance(color: Color) -> f32 {
    let srgb = color.to_srgba();
    let to_linear = |c: f32| -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };

    let r = to_linear(srgb.red);
    let g = to_linear(srgb.green);
    let b = to_linear(srgb.blue);

    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Calculate contrast ratio between background and foreground colors [1.0..21.0].
pub fn calculate_contrast_ratio(bg: Color, fg: Color) -> f32 {
    let l1 = calculate_relative_luminance(bg);
    let l2 = calculate_relative_luminance(fg);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

/// Check if color combination satisfies WCAG AA (4.5:1) standard for standard text.
pub fn satisfies_wcag_aa(bg: Color, fg: Color) -> bool {
    calculate_contrast_ratio(bg, fg) >= 4.5
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
}
