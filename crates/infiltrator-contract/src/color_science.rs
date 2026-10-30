//! Material 3 dynamic color science engine and HCT / Monet tonal palette generator (FLPARITY-024).
//!
//! Implements:
//! * CAM16 (CIECAM02/CAM16 Color Appearance Model) forward colorimetric transform under standard D65/sRGB viewing conditions;
//! * HCT (Hue, Chroma, Tone) perceptual color system conforming to Google Material Color Utilities (MCU);
//! * `HctSolver` with Newton-Raphson iteration and binary-search gamut boundary projection;
//! * Material 3 `TonalPalette` generation (Tones 0..100);
//! * `MaterialCorePalette` derivation from any seed color (Primary, Secondary, Tertiary, Neutral, NeutralVariant, Error);
//! * `MaterialColorScheme` providing full 5-level surface container hierarchies and contrast-compliant semantic pairs;
//! * High-contrast dark mode scheme corresponding to FlClash v0.9.2;
//! * Mathematically rigorous WCAG 2.1 relative luminance and contrast ratio verification.

use crate::design_tokens::RgbaToken;
use serde::{Deserialize, Serialize};

/// Linear gamma expansion for sRGB channel in `[0.0, 1.0]`.
pub fn srgb_to_linear(c: f32) -> f32 {
    let clamped = c.clamp(0.0, 1.0);
    if clamped <= 0.04045 {
        clamped / 12.92
    } else {
        ((clamped + 0.055) / 1.055).powf(2.4)
    }
}

/// Gamma compression from linear to sRGB in `[0.0, 1.0]`.
pub fn linear_to_srgb(c: f32) -> f32 {
    let clamped = c.clamp(0.0, 1.0);
    if clamped <= 0.0031308 {
        clamped * 12.92
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    }
}

/// Calculate standard WCAG 2.1 relative luminance `Y` for an RGBA token.
pub fn relative_luminance(c: RgbaToken) -> f32 {
    let r = srgb_to_linear(c.r);
    let g = srgb_to_linear(c.g);
    let b = srgb_to_linear(c.b);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Calculate contrast ratio between two colors according to WCAG 2.1 specifications.
/// Returns a ratio in `[1.0, 21.0]`.
pub fn wcag_contrast_ratio(c1: RgbaToken, c2: RgbaToken) -> f32 {
    let y1 = relative_luminance(c1);
    let y2 = relative_luminance(c2);
    let (lighter, darker) = if y1 > y2 { (y1, y2) } else { (y2, y1) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Convert CIE L* lightness (0..100) to relative luminance Y (0..1).
pub fn y_from_lstar(lstar: f32) -> f32 {
    let l = lstar.clamp(0.0, 100.0);
    if l > 8.0 {
        let val = (l + 16.0) / 116.0;
        val * val * val
    } else {
        l / 903.3
    }
}

/// Convert relative luminance Y (0..1) to CIE L* lightness (0..100).
pub fn lstar_from_y(y: f32) -> f32 {
    let y = y.clamp(0.0, 1.0);
    if y > 0.008856 {
        116.0 * y.cbrt() - 16.0
    } else {
        903.3 * y
    }
}

// CAM16 Viewing Conditions constants for standard sRGB ambient (D65 whitepoint, 200 lux, background L* = 50).
const VC_N: f32 = 0.18418652;
const VC_AW: f32 = 15.446234;
const VC_NBB: f32 = 1.0169192;
const VC_NCB: f32 = 1.0169192;
const VC_C: f32 = 0.69;
const VC_NC: f32 = 1.0;
const VC_RGB_D: [f32; 3] = [1.0206556, 0.9866453, 0.9355885];
const VC_FL: f32 = 0.0778653;
const VC_Z: f32 = 1.9091696;

const LINRGB_FROM_SCALED_DISCOUNT: [[f32; 3]; 3] = [
    [1373.2198, -1100.4252, -7.278681],
    [-271.81598, 559.6581, -32.460476],
    [1.96229, -57.173813, 308.72333],
];

const Y_FROM_LINRGB: [f32; 3] = [0.2126, 0.7152, 0.0722];

#[inline]
fn inverse_chromatic_adaptation(adapted: f32) -> f32 {
    let adapted_abs = adapted.abs();
    let base = (27.13 * adapted_abs / (400.0 - adapted_abs)).max(0.0);
    let sign = if adapted > 0.0 {
        1.0
    } else if adapted < 0.0 {
        -1.0
    } else {
        0.0
    };
    sign * base.powf(1.0 / 0.42)
}

#[inline]
fn matrix_multiply(row: [f32; 3], matrix: &[[f32; 3]; 3]) -> [f32; 3] {
    [
        row[0] * matrix[0][0] + row[1] * matrix[0][1] + row[2] * matrix[0][2],
        row[0] * matrix[1][0] + row[1] * matrix[1][1] + row[2] * matrix[1][2],
        row[0] * matrix[2][0] + row[1] * matrix[2][1] + row[2] * matrix[2][2],
    ]
}

/// Newton-Raphson search for linear RGB given CAM16 hue (radians), chroma, and target luminance Y (0..100).
fn find_result_by_j(hue_rad: f32, chroma: f32, y: f32) -> Option<[f32; 3]> {
    if y <= 0.0 {
        return Some([0.0, 0.0, 0.0]);
    }
    if y >= 100.0 {
        return Some([1.0, 1.0, 1.0]);
    }
    let mut j = y.sqrt() * 11.0;
    let t_inner_coeff = 1.0 / (1.64 - (0.29f32).powf(VC_N)).powf(0.73);
    let e_hue = 0.25 * ((hue_rad + 2.0).cos() + 3.8);
    let p1 = (50000.0 / 13.0) * e_hue * VC_NC * VC_NCB;
    let h_sin = hue_rad.sin();
    let h_cos = hue_rad.cos();

    for round_idx in 0..5 {
        let j_norm = (j / 100.0).max(1e-6);
        let alpha = if chroma == 0.0 || j == 0.0 {
            0.0
        } else {
            chroma / j_norm.sqrt()
        };
        let t = (alpha * t_inner_coeff).powf(1.0 / 0.9);
        let ac = VC_AW * j_norm.powf(1.0 / VC_C / VC_Z);
        let p2 = ac / VC_NBB;
        let denom = 23.0 * p1 + 11.0 * t * h_cos + 108.0 * t * h_sin;
        let gamma = if denom.abs() < 1e-6 {
            0.0
        } else {
            23.0 * (p2 + 0.305) * t / denom
        };
        let a = gamma * h_cos;
        let b = gamma * h_sin;
        let r_a = (460.0 * p2 + 451.0 * a + 288.0 * b) / 1403.0;
        let g_a = (460.0 * p2 - 891.0 * a - 261.0 * b) / 1403.0;
        let b_a = (460.0 * p2 - 220.0 * a - 6300.0 * b) / 1403.0;

        let r_c_scaled = inverse_chromatic_adaptation(r_a);
        let g_c_scaled = inverse_chromatic_adaptation(g_a);
        let b_c_scaled = inverse_chromatic_adaptation(b_a);

        let linrgb = matrix_multiply(
            [r_c_scaled, g_c_scaled, b_c_scaled],
            &LINRGB_FROM_SCALED_DISCOUNT,
        );
        if linrgb[0] < -0.001 || linrgb[1] < -0.001 || linrgb[2] < -0.001 {
            return None;
        }
        let fnj = Y_FROM_LINRGB[0] * linrgb[0]
            + Y_FROM_LINRGB[1] * linrgb[1]
            + Y_FROM_LINRGB[2] * linrgb[2];
        if fnj <= 0.0 {
            return None;
        }
        if round_idx == 4 || (fnj - y).abs() < 0.002 {
            if linrgb[0] > 100.001 || linrgb[1] > 100.001 || linrgb[2] > 100.001 {
                return None;
            }
            return Some([
                linear_to_srgb((linrgb[0] / 100.0).clamp(0.0, 1.0)),
                linear_to_srgb((linrgb[1] / 100.0).clamp(0.0, 1.0)),
                linear_to_srgb((linrgb[2] / 100.0).clamp(0.0, 1.0)),
            ]);
        }
        j -= (fnj - y) * j / (2.0 * fnj);
    }
    None
}

/// Solve an exact or gamut-bounded sRGB representation for CAM16 (hue, chroma) and CIE L* (tone).
fn solve_to_srgb(hue_deg: f32, chroma: f32, lstar: f32) -> [f32; 3] {
    if lstar <= 0.0 {
        return [0.0, 0.0, 0.0];
    }
    if lstar >= 100.0 {
        return [1.0, 1.0, 1.0];
    }
    if chroma < 0.001 {
        let v = linear_to_srgb(y_from_lstar(lstar));
        return [v, v, v];
    }
    let hue_sanitized = ((hue_deg % 360.0) + 360.0) % 360.0;
    let hue_rad = hue_sanitized.to_radians();
    let y = y_from_lstar(lstar) * 100.0;

    if let Some(res) = find_result_by_j(hue_rad, chroma, y) {
        return res;
    }

    // Binary search chroma limit within sRGB gamut
    let mut lo = 0.0f32;
    let mut hi = chroma;
    let mut best = None;
    for _ in 0..16 {
        let mid = (lo + hi) * 0.5;
        if let Some(attempt) = find_result_by_j(hue_rad, mid, y) {
            best = Some(attempt);
            lo = mid;
        } else {
            hi = mid;
        }
    }
    if let Some(res) = best {
        res
    } else {
        let v = linear_to_srgb(y / 100.0);
        [v, v, v]
    }
}

/// High-definition perceptual color representation in HCT (Hue, Chroma, Tone).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HctColor {
    /// Hue angle in degrees `[0.0, 360.0)`.
    pub hue: f32,
    /// Chroma intensity (colorful-ness) `[0.0, ~120.0]`.
    pub chroma: f32,
    /// Perceptual tone (CIE L* Lightness) `[0.0, 100.0]`.
    pub tone: f32,
}

impl HctColor {
    pub fn new(hue: f32, chroma: f32, tone: f32) -> Self {
        let h = ((hue % 360.0) + 360.0) % 360.0;
        let c = chroma.max(0.0);
        let t = tone.clamp(0.0, 100.0);
        Self {
            hue: h,
            chroma: c,
            tone: t,
        }
    }

    /// Construct an HCT color from RGB components in `[0.0, 1.0]` using standard CAM16.
    pub fn from_rgb(r: f32, g: f32, b: f32) -> Self {
        let r_l = srgb_to_linear(r) * 100.0;
        let g_l = srgb_to_linear(g) * 100.0;
        let b_l = srgb_to_linear(b) * 100.0;

        let x = 0.41233895 * r_l + 0.35762064 * g_l + 0.18051042 * b_l;
        let y = 0.2126 * r_l + 0.7152 * g_l + 0.0722 * b_l;
        let z = 0.019_321_41 * r_l + 0.119_163_82 * g_l + 0.950_344_8 * b_l;

        let tone = lstar_from_y(y / 100.0);

        let r_c = 0.401288 * x + 0.650173 * y - 0.051461 * z;
        let g_c = -0.250268 * x + 1.204414 * y + 0.045854 * z;
        let b_c = -0.002079 * x + 0.048952 * y + 0.953127 * z;

        let r_d = VC_RGB_D[0] * r_c;
        let g_d = VC_RGB_D[1] * g_c;
        let b_d = VC_RGB_D[2] * b_c;

        let sign = |v: f32| {
            if v > 0.0 {
                1.0
            } else if v < 0.0 {
                -1.0
            } else {
                0.0
            }
        };

        let r_af = ((VC_FL * r_d.abs()) / 100.0).powf(0.42);
        let g_af = ((VC_FL * g_d.abs()) / 100.0).powf(0.42);
        let b_af = ((VC_FL * b_d.abs()) / 100.0).powf(0.42);

        let r_a = (sign(r_d) * 400.0 * r_af) / (r_af + 27.13);
        let g_a = (sign(g_d) * 400.0 * g_af) / (g_af + 27.13);
        let b_a = (sign(b_d) * 400.0 * b_af) / (b_af + 27.13);

        let a = (11.0 * r_a - 12.0 * g_a + b_a) / 11.0;
        let b_val = (r_a + g_a - 2.0 * b_a) / 9.0;
        let u = (20.0 * r_a + 20.0 * g_a + 21.0 * b_a) / 20.0;
        let p2 = (40.0 * r_a + 20.0 * g_a + b_a) / 20.0;

        let atan_deg = b_val.atan2(a).to_degrees();
        let hue = if atan_deg < 0.0 {
            atan_deg + 360.0
        } else if atan_deg >= 360.0 {
            atan_deg - 360.0
        } else {
            atan_deg
        };

        let ac = p2 * VC_NBB;
        let j = 100.0 * (ac / VC_AW).powf(VC_C * VC_Z);

        let hue_prime = if hue < 20.14 { hue + 360.0 } else { hue };
        let e_hue = 0.25 * ((hue_prime.to_radians() + 2.0).cos() + 3.8);
        let p1 = (50000.0 / 13.0) * e_hue * VC_NC * VC_NCB;
        let t = (p1 * (a * a + b_val * b_val).sqrt()) / (u + 0.305);
        let alpha = t.powf(0.9) * (1.64 - (0.29f32).powf(VC_N)).powf(0.73);
        let chroma = alpha * (j / 100.0).sqrt();

        Self::new(hue, chroma, tone)
    }

    /// Convert HCT color back to an sRGB `RgbaToken` via `HctSolver`.
    pub fn to_rgba(&self) -> RgbaToken {
        let [r, g, b] = solve_to_srgb(self.hue, self.chroma, self.tone);
        RgbaToken::rgb(r, g, b)
    }
}

/// A Material 3 Tonal Palette generating colors at different tones from 0 to 100.
#[derive(Clone, Debug, PartialEq)]
pub struct TonalPalette {
    pub hue: f32,
    pub chroma: f32,
}

impl TonalPalette {
    pub fn of(hue: f32, chroma: f32) -> Self {
        Self { hue, chroma }
    }

    /// Get exact color for a specific tone (0..100).
    pub fn tone(&self, tone_level: u8) -> RgbaToken {
        HctColor::new(self.hue, self.chroma, tone_level as f32).to_rgba()
    }
}

/// Material 3 Core Palette derived from a single seed color.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialCorePalette {
    pub primary: TonalPalette,
    pub secondary: TonalPalette,
    pub tertiary: TonalPalette,
    pub neutral: TonalPalette,
    pub neutral_variant: TonalPalette,
    pub error: TonalPalette,
}

impl MaterialCorePalette {
    /// Derive a complete harmonious 6-palette core from a single seed color.
    pub fn of_seed(seed: HctColor) -> Self {
        Self {
            primary: TonalPalette::of(seed.hue, seed.chroma.max(48.0)),
            secondary: TonalPalette::of(seed.hue, 16.0),
            tertiary: TonalPalette::of((seed.hue + 60.0) % 360.0, 24.0),
            neutral: TonalPalette::of(seed.hue, 4.0),
            neutral_variant: TonalPalette::of(seed.hue, 8.0),
            error: TonalPalette::of(25.0, 84.0),
        }
    }
}

/// Material 3 Dynamic Color Scheme with full 5-level surface containers.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialColorScheme {
    pub primary: RgbaToken,
    pub on_primary: RgbaToken,
    pub primary_container: RgbaToken,
    pub on_primary_container: RgbaToken,
    pub secondary: RgbaToken,
    pub on_secondary: RgbaToken,
    pub secondary_container: RgbaToken,
    pub on_secondary_container: RgbaToken,
    pub tertiary: RgbaToken,
    pub on_tertiary: RgbaToken,
    pub tertiary_container: RgbaToken,
    pub on_tertiary_container: RgbaToken,
    pub error: RgbaToken,
    pub on_error: RgbaToken,
    pub error_container: RgbaToken,
    pub on_error_container: RgbaToken,
    pub background: RgbaToken,
    pub on_background: RgbaToken,
    pub surface: RgbaToken,
    pub on_surface: RgbaToken,
    pub surface_variant: RgbaToken,
    pub on_surface_variant: RgbaToken,
    pub outline: RgbaToken,
    pub outline_variant: RgbaToken,
    // 5-level M3 surface container hierarchy
    pub surface_container_lowest: RgbaToken,
    pub surface_container_low: RgbaToken,
    pub surface_container: RgbaToken,
    pub surface_container_high: RgbaToken,
    pub surface_container_highest: RgbaToken,
}

impl MaterialColorScheme {
    /// Generate Dark Theme Material Color Scheme with M3 tonal contrast.
    pub fn dark_from_core(core: &MaterialCorePalette) -> Self {
        Self {
            primary: core.primary.tone(80),
            on_primary: core.primary.tone(20),
            primary_container: core.primary.tone(30),
            on_primary_container: core.primary.tone(90),
            secondary: core.secondary.tone(80),
            on_secondary: core.secondary.tone(20),
            secondary_container: core.secondary.tone(30),
            on_secondary_container: core.secondary.tone(90),
            tertiary: core.tertiary.tone(80),
            on_tertiary: core.tertiary.tone(20),
            tertiary_container: core.tertiary.tone(30),
            on_tertiary_container: core.tertiary.tone(90),
            error: core.error.tone(80),
            on_error: core.error.tone(20),
            error_container: core.error.tone(30),
            on_error_container: core.error.tone(90),
            background: core.neutral.tone(6),
            on_background: core.neutral.tone(90),
            surface: core.neutral.tone(6),
            on_surface: core.neutral.tone(90),
            surface_variant: core.neutral_variant.tone(30),
            on_surface_variant: core.neutral_variant.tone(80),
            outline: core.neutral_variant.tone(60),
            outline_variant: core.neutral_variant.tone(30),
            surface_container_lowest: core.neutral.tone(4),
            surface_container_low: core.neutral.tone(10),
            surface_container: core.neutral.tone(12),
            surface_container_high: core.neutral.tone(17),
            surface_container_highest: core.neutral.tone(22),
        }
    }

    /// Generate Higher-contrast Dark Theme Material Color Scheme matching FlClash v0.9.2.
    pub fn dark_high_contrast_from_core(core: &MaterialCorePalette) -> Self {
        Self {
            primary: core.primary.tone(85),
            on_primary: core.primary.tone(15),
            primary_container: core.primary.tone(35),
            on_primary_container: core.primary.tone(95),
            secondary: core.secondary.tone(85),
            on_secondary: core.secondary.tone(15),
            secondary_container: core.secondary.tone(35),
            on_secondary_container: core.secondary.tone(95),
            tertiary: core.tertiary.tone(85),
            on_tertiary: core.tertiary.tone(15),
            tertiary_container: core.tertiary.tone(35),
            on_tertiary_container: core.tertiary.tone(95),
            error: core.error.tone(85),
            on_error: core.error.tone(15),
            error_container: core.error.tone(35),
            on_error_container: core.error.tone(95),
            background: core.neutral.tone(4),
            on_background: core.neutral.tone(98),
            surface: core.neutral.tone(4),
            on_surface: core.neutral.tone(98),
            surface_variant: core.neutral_variant.tone(25),
            on_surface_variant: core.neutral_variant.tone(88),
            outline: core.neutral_variant.tone(70),
            outline_variant: core.neutral_variant.tone(45),
            surface_container_lowest: core.neutral.tone(2),
            surface_container_low: core.neutral.tone(8),
            surface_container: core.neutral.tone(14),
            surface_container_high: core.neutral.tone(20),
            surface_container_highest: core.neutral.tone(26),
        }
    }

    /// Generate Light Theme Material Color Scheme with M3 tonal contrast.
    pub fn light_from_core(core: &MaterialCorePalette) -> Self {
        Self {
            primary: core.primary.tone(40),
            on_primary: core.primary.tone(100),
            primary_container: core.primary.tone(90),
            on_primary_container: core.primary.tone(10),
            secondary: core.secondary.tone(40),
            on_secondary: core.secondary.tone(100),
            secondary_container: core.secondary.tone(90),
            on_secondary_container: core.secondary.tone(10),
            tertiary: core.tertiary.tone(40),
            on_tertiary: core.tertiary.tone(100),
            tertiary_container: core.tertiary.tone(90),
            on_tertiary_container: core.tertiary.tone(10),
            error: core.error.tone(40),
            on_error: core.error.tone(100),
            error_container: core.error.tone(90),
            on_error_container: core.error.tone(10),
            background: core.neutral.tone(98),
            on_background: core.neutral.tone(10),
            surface: core.neutral.tone(98),
            on_surface: core.neutral.tone(10),
            surface_variant: core.neutral_variant.tone(90),
            on_surface_variant: core.neutral_variant.tone(30),
            outline: core.neutral_variant.tone(50),
            outline_variant: core.neutral_variant.tone(80),
            surface_container_lowest: core.neutral.tone(100),
            surface_container_low: core.neutral.tone(96),
            surface_container: core.neutral.tone(94),
            surface_container_high: core.neutral.tone(92),
            surface_container_highest: core.neutral.tone(90),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_gamma_round_trip() {
        for i in 0..=100 {
            let val = i as f32 / 100.0;
            let lin = srgb_to_linear(val);
            let back = linear_to_srgb(lin);
            assert!((val - back).abs() < 1e-4);
        }
    }

    #[test]
    fn relative_luminance_and_wcag_contrast() {
        let white = RgbaToken::rgb(1.0, 1.0, 1.0);
        let black = RgbaToken::rgb(0.0, 0.0, 0.0);

        assert!((relative_luminance(white) - 1.0).abs() < 1e-4);
        assert!((relative_luminance(black) - 0.0).abs() < 1e-4);

        let ratio = wcag_contrast_ratio(white, black);
        assert!((ratio - 21.0).abs() < 0.1);
    }

    #[test]
    fn cam16_forward_reference_vectors() {
        // Red #FF0000
        let red_hct = HctColor::from_rgb(1.0, 0.0, 0.0);
        assert!((red_hct.hue - 27.33).abs() < 1.0);
        assert!((red_hct.chroma - 111.32).abs() < 2.0);
        assert!((red_hct.tone - 53.23).abs() < 1.0);

        // Green #00FF00
        let green_hct = HctColor::from_rgb(0.0, 1.0, 0.0);
        assert!((green_hct.hue - 142.32).abs() < 1.0);
        assert!((green_hct.chroma - 107.98).abs() < 2.0);
        assert!((green_hct.tone - 87.74).abs() < 1.0);

        // Blue #0000FF
        let blue_hct = HctColor::from_rgb(0.0, 0.0, 1.0);
        assert!((blue_hct.hue - 282.82).abs() < 1.0);
        assert!((blue_hct.chroma - 86.06).abs() < 2.0);
        assert!((blue_hct.tone - 32.30).abs() < 1.0);

        // M3 Baseline #6750A4 (RGB: 103, 80, 164)
        let m3_baseline = HctColor::from_rgb(103.0 / 255.0, 80.0 / 255.0, 164.0 / 255.0);
        assert!((m3_baseline.hue - 298.82).abs() < 1.0);
        assert!((m3_baseline.chroma - 47.10).abs() < 1.5);
        assert!((m3_baseline.tone - 40.08).abs() < 1.0);

        // White #FFFFFF
        let white_hct = HctColor::from_rgb(1.0, 1.0, 1.0);
        assert!((white_hct.tone - 100.0).abs() < 0.1);

        // Black #000000
        let black_hct = HctColor::from_rgb(0.0, 0.0, 0.0);
        assert!((black_hct.tone - 0.0).abs() < 0.1);
        assert!((black_hct.chroma - 0.0).abs() < 0.1);
    }

    #[test]
    fn hct_solver_gamut_mapping_and_round_trip() {
        // M3 baseline round-trip
        let m3 = HctColor::new(298.82, 47.10, 40.08);
        let rgba = m3.to_rgba();
        assert!(rgba.r >= 0.0 && rgba.r <= 1.0);
        assert!(rgba.g >= 0.0 && rgba.g <= 1.0);
        assert!(rgba.b >= 0.0 && rgba.b <= 1.0);
        let back = HctColor::from_rgb(rgba.r, rgba.g, rgba.b);
        assert!((back.tone - 40.08).abs() < 1.5);
        assert!((back.hue - 298.82).abs() < 2.0);

        // Out-of-gamut request has deterministic bounded result
        let oog = HctColor::new(120.0, 150.0, 50.0);
        let o_rgba = oog.to_rgba();
        assert!(o_rgba.r >= 0.0 && o_rgba.r <= 1.0);
        assert!(o_rgba.g >= 0.0 && o_rgba.g <= 1.0);
        assert!(o_rgba.b >= 0.0 && o_rgba.b <= 1.0);
        let o_back = HctColor::from_rgb(o_rgba.r, o_rgba.g, o_rgba.b);
        assert!((o_back.tone - 50.0).abs() < 1.5);
        assert!((o_back.hue - 120.0).abs() < 3.0);
    }

    #[test]
    fn hct_palette_and_scheme_generation() {
        let seed = HctColor::from_rgb(0.18, 0.90, 0.62);
        let core = MaterialCorePalette::of_seed(seed);
        let dark_scheme = MaterialColorScheme::dark_from_core(&core);
        let dark_hi_scheme = MaterialColorScheme::dark_high_contrast_from_core(&core);
        let light_scheme = MaterialColorScheme::light_from_core(&core);

        // Dark mode primary on surface has readable contrast
        let dark_contrast = wcag_contrast_ratio(dark_scheme.on_surface, dark_scheme.surface);
        assert!(dark_contrast >= 4.5);

        // High-contrast dark mode meets AAA / high contrast
        let dark_hi_contrast =
            wcag_contrast_ratio(dark_hi_scheme.on_surface, dark_hi_scheme.surface);
        assert!(dark_hi_contrast >= 7.0);

        // Light mode primary on surface has readable contrast
        let light_contrast = wcag_contrast_ratio(light_scheme.on_surface, light_scheme.surface);
        assert!(light_contrast >= 4.5);

        // 5-level surface container steps are monotonic in tone
        assert!(
            relative_luminance(dark_scheme.surface_container_lowest)
                <= relative_luminance(dark_scheme.surface_container_low)
        );
        assert!(
            relative_luminance(dark_scheme.surface_container_low)
                <= relative_luminance(dark_scheme.surface_container)
        );
        assert!(
            relative_luminance(dark_scheme.surface_container)
                <= relative_luminance(dark_scheme.surface_container_high)
        );
        assert!(
            relative_luminance(dark_scheme.surface_container_high)
                <= relative_luminance(dark_scheme.surface_container_highest)
        );
    }
}
