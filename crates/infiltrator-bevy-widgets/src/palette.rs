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
