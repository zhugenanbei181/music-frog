//! Custom shader effects, analytical SDF box shadows, Kawase blur metrics, and OKLCH color science.

use bevy::color::Color;
use bevy::math::Vec2;

/// Fallback mode for shader effects on low-power or non-shader targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShaderFallbackMode {
    #[default]
    FullShader,
    SimulatedCpu,
    DisabledFlat,
}

/// Analytical Signed Distance Field (SDF) parameters for a rounded rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SdfRoundedBox {
    pub size: Vec2,
    pub radius: f32,
    pub border_width: f32,
}

impl SdfRoundedBox {
    pub fn new(size: Vec2, radius: f32, border_width: f32) -> Self {
        Self {
            size,
            radius,
            border_width,
        }
    }

    /// Exact signed distance from a 2D point relative to the box center.
    /// Returns negative distance inside the shape, positive outside.
    pub fn distance_at(&self, point: Vec2) -> f32 {
        let half_size = self.size * 0.5;
        let r = self.radius.min(half_size.x).min(half_size.y);
        let q = point.abs() - half_size + Vec2::splat(r);
        let outside = Vec2::new(q.x.max(0.0), q.y.max(0.0)).length();
        let inside = q.x.max(q.y).min(0.0);
        outside + inside - r
    }

    /// Evaluates coverage factor [0.0, 1.0] for anti-aliasing given pixel scale.
    pub fn coverage_at(&self, point: Vec2, pixel_scale: f32) -> f32 {
        let dist = self.distance_at(point);
        (1.0 - dist / pixel_scale.max(1e-4)).clamp(0.0, 1.0)
    }
}

/// Parameters for multi-pass Dual Kawase blur kernels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KawasePassMetrics {
    pub pass_index: usize,
    pub sample_offset_px: f32,
    pub downscale_factor: f32,
}

impl KawasePassMetrics {
    /// Compute sample offsets for an N-pass Dual Kawase blur.
    pub fn compute_passes(passes: usize) -> Vec<KawasePassMetrics> {
        (0..passes)
            .map(|idx| {
                let downscale_factor = (1 << idx) as f32;
                let sample_offset_px = (idx as f32 + 1.0) * 1.5;
                KawasePassMetrics {
                    pass_index: idx,
                    sample_offset_px,
                    downscale_factor,
                }
            })
            .collect()
    }
}

/// Color represented in the perceptually uniform OKLCH color space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OklchColor {
    pub lightness: f32, // [0.0, 1.0]
    pub chroma: f32,    // [0.0, ~0.4]
    pub hue_deg: f32,   // [0.0, 360.0]
    pub alpha: f32,
}

impl OklchColor {
    pub fn new(lightness: f32, chroma: f32, hue_deg: f32) -> Self {
        Self {
            lightness,
            chroma,
            hue_deg,
            alpha: 1.0,
        }
    }

    /// Generate an accessible perceptual tonal ladder of N steps from Light to Dark.
    pub fn generate_tonal_ladder(&self, steps: usize) -> Vec<Color> {
        let steps = steps.max(2);
        (0..steps)
            .map(|i| {
                let l = 0.95 - (i as f32 / (steps - 1) as f32) * 0.8;
                let c = self.chroma * (1.0 - (l - 0.5).abs() * 0.5);
                // Approximate conversion to linear sRGB
                let h_rad = self.hue_deg.to_radians();
                let a = c * h_rad.cos();
                let b = c * h_rad.sin();
                let r = (l + 0.396 * a + 0.215 * b).clamp(0.0, 1.0);
                let g = (l - 0.105 * a - 0.063 * b).clamp(0.0, 1.0);
                let bl = (l - 0.089 * a - 1.291 * b).clamp(0.0, 1.0);
                Color::srgba(r, g, bl, self.alpha)
            })
            .collect()
    }
}

use crate::palette::UiPalette;
use crate::theme::space;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    BackgroundColor, BorderRadius, FlexDirection, Node, UiRect, Val, percent, px,
};

/// Specification for moving shimmer gradient waves on skeleton loading placeholders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShimmerWaveSpec {
    pub speed_hz: f32,
    pub wave_width_fraction: f32,
    pub highlight_intensity: f32,
}

impl Default for ShimmerWaveSpec {
    fn default() -> Self {
        Self {
            speed_hz: 0.8,
            wave_width_fraction: 0.4,
            highlight_intensity: 0.15,
        }
    }
}

impl ShimmerWaveSpec {
    /// Calculate current wave center position in [0.0..1.0] at time `t` seconds.
    pub fn wave_position(&self, time_secs: f32) -> f32 {
        (time_secs * self.speed_hz).fract()
    }

    /// Calculate brightness boost [0.0..highlight_intensity] for normalized position `x` in [0.0..1.0].
    pub fn brightness_boost_at(&self, x: f32, time_secs: f32) -> f32 {
        let center = self.wave_position(time_secs);
        let dist = (x - center).abs();
        if dist < self.wave_width_fraction * 0.5 {
            let norm = 1.0 - (dist / (self.wave_width_fraction * 0.5));
            (norm * norm) * self.highlight_intensity
        } else {
            0.0
        }
    }
}

/// Construct a declarative skeleton card scene matching real card layouts.
pub fn skeleton_card_scene(height_px: f32, palette: &UiPalette) -> Box<dyn Scene> {
    let base_fill = palette.surface_elevated;
    let shimmer_bar_fill = palette.border;

    Box::new(bsn! {
        Node {
            width: percent(100),
            height: px(height_px),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::S16)),
            row_gap: Val::Px(space::S12),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ base_fill })
        Children [
            (
                Node {
                    width: percent(40),
                    height: px(16.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ shimmer_bar_fill })
            ),
            (
                Node {
                    width: percent(80),
                    height: px(12.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ shimmer_bar_fill })
            ),
            (
                Node {
                    flex_grow: 1.0,
                    width: percent(100),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ shimmer_bar_fill })
            ),
        ]
    })
}

/// Multi-tier analytical drop shadow parameters for elevated cards, dialogs, and floating menus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalyticalDropShadow {
    pub offset: Vec2,
    pub blur_radius: f32,
    pub spread: f32,
    pub shadow_color: Color,
}

impl AnalyticalDropShadow {
    /// Low elevation: subtle 2px card depth.
    pub fn elevation_low() -> Self {
        Self {
            offset: Vec2::new(0.0, 2.0),
            blur_radius: 4.0,
            spread: 0.0,
            shadow_color: Color::srgba(0.0, 0.0, 0.0, 0.12),
        }
    }

    /// Medium elevation: floating dropdown/menu 6px depth.
    pub fn elevation_medium() -> Self {
        Self {
            offset: Vec2::new(0.0, 6.0),
            blur_radius: 12.0,
            spread: 1.0,
            shadow_color: Color::srgba(0.0, 0.0, 0.0, 0.20),
        }
    }

    /// High elevation: centered modal dialog / command palette 16px depth.
    pub fn elevation_high() -> Self {
        Self {
            offset: Vec2::new(0.0, 16.0),
            blur_radius: 32.0,
            spread: 2.0,
            shadow_color: Color::srgba(0.0, 0.0, 0.0, 0.35),
        }
    }

    /// Approximate Gaussian/Hermite alpha falloff at signed distance `d`.
    pub fn falloff_alpha(&self, distance: f32) -> f32 {
        if distance <= 0.0 {
            1.0
        } else if distance >= self.blur_radius {
            0.0
        } else {
            let t = distance / self.blur_radius;
            (1.0 - t * t * (3.0 - 2.0 * t)).clamp(0.0, 1.0)
        }
    }
}

/// Dynamic pulsating neon glow effect for status badges, active proxies, and alert indicators.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlowSpec {
    pub base_color: Color,
    pub radius_px: f32,
    pub pulse_frequency_hz: f32,
    pub min_alpha: f32,
    pub max_alpha: f32,
}

impl GlowSpec {
    pub fn new(color: Color, radius_px: f32, pulse_hz: f32) -> Self {
        Self {
            base_color: color,
            radius_px,
            pulse_frequency_hz: pulse_hz,
            min_alpha: 0.2,
            max_alpha: 0.85,
        }
    }

    /// Calculate animated alpha at time `t` seconds with sine wave modulation.
    pub fn current_alpha(&self, time_secs: f32) -> f32 {
        let phase = (time_secs * self.pulse_frequency_hz * std::f32::consts::TAU).sin();
        let normalized = (phase + 1.0) * 0.5; // [0.0, 1.0]
        self.min_alpha + normalized * (self.max_alpha - self.min_alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_rounded_box_distance_and_coverage() {
        let box_sdf = SdfRoundedBox::new(Vec2::new(100.0, 60.0), 8.0, 0.0);
        // Center is inside: negative distance
        assert!(box_sdf.distance_at(Vec2::ZERO) < 0.0);
        // Outside far point
        assert!(box_sdf.distance_at(Vec2::new(100.0, 100.0)) > 0.0);
        // Coverage is 1 inside, 0 outside
        assert_eq!(box_sdf.coverage_at(Vec2::ZERO, 1.0), 1.0);
    }

    #[test]
    fn test_drop_shadow_elevations_and_falloff() {
        let low = AnalyticalDropShadow::elevation_low();
        let high = AnalyticalDropShadow::elevation_high();
        assert!(high.blur_radius > low.blur_radius);

        assert_eq!(low.falloff_alpha(0.0), 1.0);
        assert_eq!(low.falloff_alpha(low.blur_radius + 1.0), 0.0);
        let mid = low.falloff_alpha(low.blur_radius * 0.5);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn test_glow_spec_pulse_alpha() {
        let glow = GlowSpec::new(Color::srgba(0.0, 1.0, 0.5, 1.0), 12.0, 1.0);
        let a0 = glow.current_alpha(0.0); // sin(0)=0 -> (0+1)/2 = 0.5
        let mid_expected = (glow.min_alpha + glow.max_alpha) * 0.5;
        assert!((a0 - mid_expected).abs() < 1e-4);

        let a_max = glow.current_alpha(0.25); // sin(pi/2)=1 -> 1.0 -> max_alpha
        assert!((a_max - glow.max_alpha).abs() < 1e-4);
    }
}
