//! Headless tests for the theme-token → bevy adapter.
//!
//! The oracle is the token set itself: every resolved bevy value must equal
//! the token it claims to adapt (channel-exact colors, scale-exact metrics).
//! A mutation that hardcodes a bevy default or reinterprets a token fails
//! here without any window.

use infiltrator_bevy_widgets::palette::{UiPalette, theme_color};
use infiltrator_bevy_widgets::theme::{LightDark, Theme, TokenColor, metrics, radius, type_scale};

fn assert_same_color(bevy_color: bevy::color::Color, token: TokenColor) {
    let srgba = bevy_color.to_srgba();
    assert_eq!(
        (srgba.red, srgba.green, srgba.blue, srgba.alpha),
        (token.r, token.g, token.b, token.a),
        "bevy color diverged from its token"
    );
}

#[test]
fn token_colors_round_trip_channel_exact() {
    for theme in [Theme::dark(), Theme::light()] {
        for token in [
            theme.window_bg,
            theme.surface,
            theme.surface_elevated,
            theme.ink,
            theme.ink_dim,
            theme.accent,
            theme.on_accent,
            theme.accent_container,
            theme.sidebar,
            theme.icon_tile,
            theme.hover,
            theme.pressed,
            theme.border,
            theme.success,
            theme.warning,
            theme.danger,
        ] {
            assert_same_color(theme_color(token), token);
        }
    }
}

#[test]
fn palette_carries_every_token_and_scale() {
    for theme in [Theme::dark(), Theme::light()] {
        let palette = UiPalette::new(&theme);
        assert_same_color(palette.window_clear, theme.window_bg);
        assert_same_color(palette.surface, theme.surface);
        assert_same_color(palette.surface_elevated, theme.surface_elevated);
        assert_same_color(palette.ink, theme.ink);
        assert_same_color(palette.ink_dim, theme.ink_dim);
        assert_same_color(palette.accent, theme.accent);
        assert_same_color(palette.on_accent, theme.on_accent);
        assert_same_color(palette.accent_container, theme.accent_container);
        assert_same_color(palette.sidebar, theme.sidebar);
        assert_same_color(palette.icon_tile, theme.icon_tile);
        assert_same_color(palette.hover_bg, theme.hover);
        assert_same_color(palette.pressed_bg, theme.pressed);
        assert_same_color(palette.border, theme.border);
        assert_same_color(palette.success, theme.success);
        assert_same_color(palette.warning, theme.warning);
        assert_same_color(palette.danger, theme.danger);
        assert_eq!(palette.card_radius_px, radius::CARD);
        assert_eq!(palette.control_radius_px, radius::CONTROL);
        assert_eq!(palette.control_height_px, metrics::CONTROL_HEIGHT);
        assert_eq!(palette.heading_font_px, type_scale::HEADING);
        assert_eq!(palette.body_font_px, type_scale::BODY);
        assert_eq!(palette.caption_font_px, type_scale::CAPTION);
        assert_eq!(palette.mono_font_px, type_scale::MONO);
    }
}

#[test]
fn appearances_stay_distinct() {
    let dark = Theme::dark();
    let light = Theme::light();
    assert_eq!(dark.mode, LightDark::Dark);
    assert_eq!(light.mode, LightDark::Light);
    assert_ne!(dark.window_bg, light.window_bg);
    assert_ne!(dark.accent, light.accent);
}
