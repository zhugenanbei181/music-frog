//! Color and theme styling utilities for Slint presentation.

use super::AppWindow;

/// Parses a six digit hexadecimal colour into a Slint colour.
///
/// The surface, the settings callback and the surface tests all use this one
/// parser, so an invalid accent colour can never be accepted by one path and
/// rejected by another.
pub fn parse_color(value: &str) -> Option<slint::Color> {
    let value = value.trim().trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    let red = u8::from_str_radix(&value[0..2], 16).ok()?;
    let green = u8::from_str_radix(&value[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some(slint::Color::from_rgb_u8(red, green, blue))
}

pub(crate) fn apply_theme_color(weak: &slint::Weak<AppWindow>, hex: &str) {
    let Some(color) = parse_color(hex) else {
        return;
    };
    let hex = hex.to_owned();
    let _ = weak.clone().upgrade_in_event_loop(move |window| {
        window.set_theme_color(color);
        window.set_theme_color_input_code(hex.clone().into());
        window.set_theme_color_code_draft(hex.clone().into());
        window.set_theme_color_draft(hex.clone().into());
        window.set_theme_accent_light_foreground(contrast_is_light(&hex));
    });
}

/// Reports whether an accent colour needs a light foreground on top of it.
pub fn contrast_is_light(hex: &str) -> bool {
    let Some(color) = parse_color(hex) else {
        return true;
    };
    let rgba = color.to_argb_u8();
    u32::from(rgba.red) * 299 + u32::from(rgba.green) * 587 + u32::from(rgba.blue) * 114 >= 128_000
}

pub(crate) fn hsv_hex(hue: f64, saturation: f64, value: f64) -> String {
    let hue = hue.rem_euclid(360.0) / 60.0;
    let saturation = saturation.clamp(0.0, 1.0);
    let value = value.clamp(0.0, 1.0);
    let chroma = value * saturation;
    let secondary = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let offset = value - chroma;
    let (red, green, blue) = match hue as u8 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    format!(
        "#{:02X}{:02X}{:02X}",
        ((red + offset) * 255.0).round() as u8,
        ((green + offset) * 255.0).round() as u8,
        ((blue + offset) * 255.0).round() as u8,
    )
}
