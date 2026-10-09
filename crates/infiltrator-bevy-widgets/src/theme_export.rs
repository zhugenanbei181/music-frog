//! Theme token serializer exporting to Tailwind CSS, Material You, and standalone JSON specifications.
//!
//! It also carries the product's OKLCH tonal-ladder integration (BEVY-034):
//! the hand-picked semantic tokens (accent / success / warning / danger) seed
//! perceptually-uniform ladders generated in the OKLCH color space, with an
//! explicit sRGB ramp as the documented fallback. Existing token names and
//! values are untouched — the ladders are a derived view.

use crate::palette::UiPalette;
use crate::shader_fx::OklchColor;
use crate::theme::{Theme, TokenColor, contrast};
use bevy::color::Color;

/// Target export format for theme design tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeExportFormat {
    Json,
    TailwindCss,
    MaterialYouXml,
}

/// Convert linear/sRGB Bevy Color into standard hex `#RRGGBB` or `#RRGGBBAA` string.
pub fn color_to_hex(color: Color) -> String {
    let s = color.to_srgba();
    let r = (s.red * 255.0).round() as u8;
    let g = (s.green * 255.0).round() as u8;
    let b = (s.blue * 255.0).round() as u8;
    let a = (s.alpha * 255.0).round() as u8;
    if a == 255 {
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }
}

/// Export full UiPalette tokens into target format.
pub fn export_palette_tokens(palette: &UiPalette, format: ThemeExportFormat) -> String {
    match format {
        ThemeExportFormat::Json => {
            format!(
                r#"{{"accent":"{}","surface":"{}","border":"{}","ink":"{}"}}"#,
                color_to_hex(palette.accent),
                color_to_hex(palette.surface),
                color_to_hex(palette.border),
                color_to_hex(palette.ink)
            )
        }
        ThemeExportFormat::TailwindCss => {
            format!(
                ":root {{
  --color-accent: {};
  --color-surface: {};
  --color-border: {};
  --color-ink: {};
}}",
                color_to_hex(palette.accent),
                color_to_hex(palette.surface),
                color_to_hex(palette.border),
                color_to_hex(palette.ink)
            )
        }
        ThemeExportFormat::MaterialYouXml => {
            format!(
                r#"<resources>
  <color name="md_theme_primary">{}</color>
  <color name="md_theme_surface">{}</color>
</resources>"#,
                color_to_hex(palette.accent),
                color_to_hex(palette.surface)
            )
        }
    }
}

/// Number of stops in a product tonal ladder (Material You's 0-100 tonal grid
/// sampled at nine accessible levels).
pub const DEFAULT_LADDER_STEPS: usize = 9;

/// Which generator produced a [`TonalLadder`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LadderSource {
    /// Perceptually-uniform OKLCH sweep.
    Oklch,
    /// Explicit sRGB ramp, used when OKLCH cannot be applied.
    SrgbFallback,
}

/// How a [`TonalLadder`] should be generated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LadderStrategy {
    /// Perceptual OKLCH sweep (the product default).
    Perceptual,
    /// Explicit sRGB ramp — the fallback path.
    SrgbFallback,
}

/// A monotonic tonal ladder, ordered lightest stop first.
#[derive(Clone, Debug, PartialEq)]
pub struct TonalLadder {
    pub source: LadderSource,
    pub stops: Vec<TokenColor>,
}

impl TonalLadder {
    pub fn len(&self) -> usize {
        self.stops.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stops.is_empty()
    }

    /// Lightest stop (the ladder head).
    pub fn lightest(&self) -> Option<TokenColor> {
        self.stops.first().copied()
    }

    /// Darkest stop (the ladder tail).
    pub fn darkest(&self) -> Option<TokenColor> {
        self.stops.last().copied()
    }

    /// WCAG contrast ratio spanned by the lightest and darkest stops.
    pub fn contrast_span(&self) -> f32 {
        match (self.lightest(), self.darkest()) {
            (Some(light), Some(dark)) => contrast::contrast_ratio(light, dark),
            _ => 1.0,
        }
    }
}

/// The derived tonal ladders for one resolved [`Theme`].
#[derive(Clone, Debug, PartialEq)]
pub struct ThemeTonalLadders {
    pub accent: TonalLadder,
    pub success: TonalLadder,
    pub warning: TonalLadder,
    pub danger: TonalLadder,
}

/// Serialize a token to `#RRGGBB` / `#RRGGBBAA` through the shared hex writer.
pub fn token_to_hex(token: TokenColor) -> String {
    color_to_hex(Color::srgba(token.r, token.g, token.b, token.a))
}

fn linearize_channel(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn encode_channel(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.003_130_8 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

/// Convert an sRGB token into the perceptually-uniform OKLCH space
/// (Björn Ottosson's OKLab, expressed as lightness / chroma / hue).
pub fn token_to_oklch(token: TokenColor) -> OklchColor {
    let red = linearize_channel(token.r);
    let green = linearize_channel(token.g);
    let blue = linearize_channel(token.b);

    let long = 0.412_221_47 * red + 0.536_332_54 * green + 0.051_445_995 * blue;
    let medium = 0.211_903_5 * red + 0.680_699_5 * green + 0.107_396_96 * blue;
    let short = 0.088_302_46 * red + 0.281_718_84 * green + 0.629_978_7 * blue;

    let long_root = long.cbrt();
    let medium_root = medium.cbrt();
    let short_root = short.cbrt();

    let lightness =
        0.210_454_26 * long_root + 0.793_617_8 * medium_root - 0.004_072_047 * short_root;
    let axis_a = 1.977_998_5 * long_root - 2.428_592_2 * medium_root + 0.450_593_7 * short_root;
    let axis_b = 0.025_904_037 * long_root + 0.782_771_77 * medium_root - 0.808_675_77 * short_root;

    let chroma = (axis_a * axis_a + axis_b * axis_b).sqrt();
    let mut hue_deg = axis_b.atan2(axis_a).to_degrees();
    if hue_deg < 0.0 {
        hue_deg += 360.0;
    }

    OklchColor {
        lightness,
        chroma,
        hue_deg,
        alpha: token.a,
    }
}

/// Convert an OKLCH color back to an sRGB token, clamping to the sRGB gamut.
pub fn oklch_to_token(oklch: OklchColor) -> TokenColor {
    let hue = oklch.hue_deg.to_radians();
    let axis_a = oklch.chroma * hue.cos();
    let axis_b = oklch.chroma * hue.sin();

    let long_root = oklch.lightness + 0.396_337_78 * axis_a + 0.215_803_76 * axis_b;
    let medium_root = oklch.lightness - 0.105_561_346 * axis_a - 0.063_854_17 * axis_b;
    let short_root = oklch.lightness - 0.089_484_18 * axis_a - 1.291_485_5 * axis_b;

    let long = long_root * long_root * long_root;
    let medium = medium_root * medium_root * medium_root;
    let short = short_root * short_root * short_root;

    let red = 4.076_741_7 * long - 3.307_711_6 * medium + 0.230_969_94 * short;
    let green = -1.268_438 * long + 2.609_757_4 * medium - 0.341_319_4 * short;
    let blue = -0.004_196_086_3 * long - 0.703_418_6 * medium + 1.707_614_7 * short;

    TokenColor {
        r: encode_channel(red),
        g: encode_channel(green),
        b: encode_channel(blue),
        a: oklch.alpha,
    }
}

fn blend_token(from: TokenColor, toward: TokenColor, amount: f32) -> TokenColor {
    let amount = amount.clamp(0.0, 1.0);
    TokenColor {
        r: from.r + (toward.r - from.r) * amount,
        g: from.g + (toward.g - from.g) * amount,
        b: from.b + (toward.b - from.b) * amount,
        a: from.a,
    }
}

fn lerp_token(from: TokenColor, to: TokenColor, amount: f32) -> TokenColor {
    let amount = amount.clamp(0.0, 1.0);
    TokenColor {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}

/// Generate a perceptually-uniform ladder by sweeping OKLCH lightness from
/// light to dark while holding hue and tapering chroma at the gamut extremes.
pub fn perceptual_ladder(seed: TokenColor, steps: usize) -> TonalLadder {
    let steps = steps.max(2);
    let base = token_to_oklch(seed);
    let stops = (0..steps)
        .map(|index| {
            let t = index as f32 / (steps - 1) as f32;
            let lightness = 0.97 - t * 0.82;
            let chroma = base.chroma * (1.0 - (lightness - 0.5).abs() * 0.6);
            oklch_to_token(OklchColor {
                lightness,
                chroma: chroma.max(0.0),
                hue_deg: base.hue_deg,
                alpha: base.alpha,
            })
        })
        .collect();
    TonalLadder {
        source: LadderSource::Oklch,
        stops,
    }
}

/// Generate an explicit sRGB fallback ladder by ramping between a light tint
/// and a dark shade of the seed. This is the documented path when OKLCH
/// conversion is unavailable, and it stays monotonic in relative luminance.
pub fn srgb_fallback_ladder(seed: TokenColor, steps: usize) -> TonalLadder {
    let steps = steps.max(2);
    let light = blend_token(seed, TokenColor::rgb(1.0, 1.0, 1.0), 0.85);
    let dark = blend_token(seed, TokenColor::rgb(0.0, 0.0, 0.0), 0.72);
    let stops = (0..steps)
        .map(|index| {
            let t = index as f32 / (steps - 1) as f32;
            lerp_token(light, dark, t)
        })
        .collect();
    TonalLadder {
        source: LadderSource::SrgbFallback,
        stops,
    }
}

/// Generate a ladder under an explicit strategy.
pub fn generate_ladder(seed: TokenColor, steps: usize, strategy: LadderStrategy) -> TonalLadder {
    match strategy {
        LadderStrategy::Perceptual => perceptual_ladder(seed, steps),
        LadderStrategy::SrgbFallback => srgb_fallback_ladder(seed, steps),
    }
}

/// Derive the product tonal ladders from the resolved theme's semantic tokens.
/// The seed tokens keep their hand-picked values; only the surrounding ladder
/// is generated, and the token names and semantics are unchanged.
pub fn theme_tonal_ladders(theme: &Theme) -> ThemeTonalLadders {
    let steps = DEFAULT_LADDER_STEPS;
    ThemeTonalLadders {
        accent: perceptual_ladder(theme.accent, steps),
        success: perceptual_ladder(theme.success, steps),
        warning: perceptual_ladder(theme.warning, steps),
        danger: perceptual_ladder(theme.danger, steps),
    }
}

fn ladder_source_label(source: LadderSource) -> &'static str {
    match source {
        LadderSource::Oklch => "oklch",
        LadderSource::SrgbFallback => "srgb-fallback",
    }
}

/// Export one tonal ladder in a target format.
pub fn export_tonal_ladder(ladder: &TonalLadder, format: ThemeExportFormat) -> String {
    match format {
        ThemeExportFormat::Json => {
            let stops: Vec<String> = ladder
                .stops
                .iter()
                .map(|stop| format!("\"{}\"", token_to_hex(*stop)))
                .collect();
            format!(
                r#"{{"source":"{}","stops":[{}]}}"#,
                ladder_source_label(ladder.source),
                stops.join(",")
            )
        }
        ThemeExportFormat::TailwindCss => {
            let mut output = String::from(":root {");
            for (index, stop) in ladder.stops.iter().enumerate() {
                output.push_str(&format!("\n  --tonal-{}: {};", index, token_to_hex(*stop)));
            }
            output.push_str("\n}");
            output
        }
        ThemeExportFormat::MaterialYouXml => {
            let mut output = String::from("<resources>\n");
            for (index, stop) in ladder.stops.iter().enumerate() {
                output.push_str(&format!(
                    "  <color name=\"md_theme_tonal_{}\">{}</color>\n",
                    index,
                    token_to_hex(*stop)
                ));
            }
            output.push_str("</resources>");
            output
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::ThemeSkin;

    fn is_monotonic(ladder: &TonalLadder) -> bool {
        ladder.stops.windows(2).all(|window| {
            contrast::relative_luminance(window[0]) > contrast::relative_luminance(window[1])
        })
    }

    #[test]
    fn test_perceptual_ladder_is_monotonic_in_lightness() {
        let seeds = [
            TokenColor::rgb(0.12, 0.56, 0.96),
            TokenColor::rgb(0.24, 0.78, 0.44),
            TokenColor::rgb(0.96, 0.62, 0.15),
            TokenColor::rgb(0.96, 0.35, 0.32),
            TokenColor::rgb(0.188, 0.435, 0.306),
        ];
        for seed in seeds {
            let ladder = perceptual_ladder(seed, DEFAULT_LADDER_STEPS);
            assert_eq!(ladder.len(), DEFAULT_LADDER_STEPS);
            assert_eq!(ladder.source, LadderSource::Oklch);
            assert!(
                is_monotonic(&ladder),
                "ladder for {seed:?} is not monotonic"
            );
            assert!(ladder.contrast_span() >= 7.0);
        }
    }

    #[test]
    fn test_srgb_fallback_ladder_is_explicit_and_monotonic() {
        let seed = TokenColor::rgb(0.12, 0.56, 0.96);
        let ladder = generate_ladder(seed, DEFAULT_LADDER_STEPS, LadderStrategy::SrgbFallback);
        assert_eq!(ladder.source, LadderSource::SrgbFallback);
        assert_eq!(ladder.len(), DEFAULT_LADDER_STEPS);
        assert!(is_monotonic(&ladder));
        assert!(ladder.contrast_span() >= 4.5);
        // The fallback is a distinct generator, never a silent alias.
        assert_ne!(
            ladder,
            generate_ladder(seed, DEFAULT_LADDER_STEPS, LadderStrategy::Perceptual)
        );
    }

    #[test]
    fn test_theme_tonal_ladders_derive_from_real_tokens() {
        for skin in ThemeSkin::ALL {
            let theme = Theme::for_mode(skin);
            let ladders = theme_tonal_ladders(&theme);
            for ladder in [
                &ladders.accent,
                &ladders.success,
                &ladders.warning,
                &ladders.danger,
            ] {
                assert_eq!(ladder.source, LadderSource::Oklch);
                assert_eq!(ladder.len(), DEFAULT_LADDER_STEPS);
                assert!(is_monotonic(ladder));
                assert!(ladder.contrast_span() >= 7.0);
            }
        }
        // The hand-picked seed tokens are unchanged by the ladder integration.
        assert_eq!(Theme::dark().accent, TokenColor::rgb(0.12, 0.56, 0.96));
        assert_eq!(Theme::light().accent, TokenColor::rgb(0.04, 0.44, 0.88));
    }

    #[test]
    fn test_oklch_round_trip_is_stable() {
        let seeds = [
            TokenColor::rgb(0.12, 0.56, 0.96),
            TokenColor::rgb(0.96, 0.35, 0.32),
            TokenColor::rgb(0.5, 0.5, 0.5),
        ];
        for seed in seeds {
            let round_tripped = oklch_to_token(token_to_oklch(seed));
            assert!((round_tripped.r - seed.r).abs() < 1e-3);
            assert!((round_tripped.g - seed.g).abs() < 1e-3);
            assert!((round_tripped.b - seed.b).abs() < 1e-3);
        }
    }

    #[test]
    fn test_export_tonal_ladder_formats() {
        let ladder = perceptual_ladder(TokenColor::rgb(0.12, 0.56, 0.96), 5);
        let json = export_tonal_ladder(&ladder, ThemeExportFormat::Json);
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"source\":\"oklch\""));
        assert!(json.contains("\"stops\":["));

        let css = export_tonal_ladder(&ladder, ThemeExportFormat::TailwindCss);
        assert!(css.contains("--tonal-0:"));
        assert!(css.contains("--tonal-4:"));

        let xml = export_tonal_ladder(&ladder, ThemeExportFormat::MaterialYouXml);
        assert!(xml.contains("md_theme_tonal_0"));
        assert!(xml.contains("md_theme_tonal_4"));
    }
}
