//! Custom shader effects, analytical SDF box shadows, Kawase blur metrics, and OKLCH color science.

use crate::shader_assets;
use crate::surface_shader::{SurfaceShaderMode, release_retired, sync};
use bevy::app::{App, Plugin, PostUpdate};
use bevy::asset::{Asset, AssetApp, embedded_asset};
use bevy::color::{Color, LinearRgba};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::math::Vec2;
use bevy::reflect::TypePath;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::ui::UiSystems;
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};
use std::f32::consts::TAU;

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

/// G2 continuous curvature superellipse (Squircle) Signed Distance Field.
///
/// Implements superellipse formula `(|x|/a)^p + (|y|/b)^p = 1` where
/// `p = 2.0 + 3.0 * smoothing`. When `smoothing = 0.0` (p = 2.0), this reduces
/// to a standard G1 circular fillet. When `smoothing = 1.0` (p = 5.0), it forms
/// a G2 continuous squircle with no sharp curvature jump at the corner seam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SdfSquircle {
    pub size: Vec2,
    pub radius: f32,
    pub smoothing: f32,
    pub border_width: f32,
}

impl SdfSquircle {
    pub fn new(size: Vec2, radius: f32, smoothing: f32, border_width: f32) -> Self {
        Self {
            size,
            radius,
            smoothing: smoothing.clamp(0.0, 1.0),
            border_width,
        }
    }

    /// Exact signed distance from a 2D point relative to the squircle center.
    /// Negative inside the shape, 0 on the boundary, positive outside.
    pub fn distance_at(&self, point: Vec2) -> f32 {
        let half_size = self.size * 0.5;
        let safe_r = self.radius.clamp(0.0, half_size.x.min(half_size.y));
        let q = point.abs() - (half_size - Vec2::splat(safe_r));
        let p_exp = 2.0 + 3.0 * self.smoothing;

        if q.x > 0.0 && q.y > 0.0 {
            (q.x.powf(p_exp) + q.y.powf(p_exp)).powf(1.0 / p_exp) - safe_r
        } else {
            q.x.max(q.y) - safe_r
        }
    }

    /// Evaluates coverage factor [0.0, 1.0] for anti-aliasing given pixel scale.
    pub fn coverage_at(&self, point: Vec2, pixel_scale: f32) -> f32 {
        let dist = self.distance_at(point);
        (1.0 - dist / pixel_scale.max(1e-4)).clamp(0.0, 1.0)
    }

    /// Evaluates border band coverage factor [0.0, 1.0].
    pub fn border_coverage_at(&self, point: Vec2, pixel_scale: f32) -> f32 {
        if self.border_width <= 0.0 {
            return 0.0;
        }
        let outer_cov = self.coverage_at(point, pixel_scale);
        let inner_dist = self.distance_at(point) + self.border_width;
        let inner_cov = (1.0 - inner_dist / pixel_scale.max(1e-4)).clamp(0.0, 1.0);
        (outer_cov - inner_cov).clamp(0.0, 1.0)
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
                Node {
                    width: percent(40),
                    height: px(16.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ shimmer_bar_fill })
                --
                Node {
                    width: percent(80),
                    height: px(12.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ shimmer_bar_fill })
                --
                Node {
                    flex_grow: 1.0,
                    width: percent(100),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ shimmer_bar_fill })
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
        let phase = (time_secs * self.pulse_frequency_hz * TAU).sin();
        let normalized = (phase + 1.0) * 0.5; // [0.0, 1.0]
        self.min_alpha + normalized * (self.max_alpha - self.min_alpha)
    }
}

/// Multi-tier analytical elevation levels for cards, modals, and tooltips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ModernSurfaceElevation {
    #[default]
    None,
    Low,
    Medium,
    High,
}

impl ModernSurfaceElevation {
    /// Return physical parameters: (offset_y, blur_radius, spread, ambient_alpha, key_alpha).
    pub const fn params(&self) -> (f32, f32, f32, f32, f32) {
        match self {
            Self::None => (0.0, 0.0, 0.0, 0.0, 0.0),
            Self::Low => (2.0, 4.0, 0.0, 0.04, 0.08),
            Self::Medium => (6.0, 12.0, 1.0, 0.06, 0.12),
            Self::High => (16.0, 32.0, 2.0, 0.08, 0.20),
        }
    }
}

/// Uniform data transmitted to the `modern_surface.wesl` shader.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub struct ModernSurfaceUniform {
    pub color: LinearRgba,
    pub border_color: LinearRgba,
    pub shadow_color: LinearRgba,
    pub dimensions: Vec2,
    pub radius: f32,
    pub border_width: f32,
    pub smoothing: f32,
    pub shadow_offset: Vec2,
    pub shadow_blur: f32,
    pub shadow_spread: f32,
    pub ambient_alpha: f32,
    pub key_alpha: f32,
}

/// GPU UI material rendering superellipse cards and analytical drop shadows.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone, PartialEq)]
pub struct ModernSurfaceMaterial {
    #[uniform(0)]
    pub uniform: ModernSurfaceUniform,
}

impl UiMaterial for ModernSurfaceMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://infiltrator_bevy_widgets/modern_surface.wesl".into()
    }
}

impl ModernSurfaceMaterial {
    /// Create a superellipse card with border and analytical shadow.
    pub fn card(
        dimensions: Vec2,
        radius: f32,
        smoothing: f32,
        fill: Color,
        border: Color,
        border_width: f32,
        elevation: ModernSurfaceElevation,
    ) -> Self {
        let (offset_y, blur, spread, ambient_alpha, key_alpha) = elevation.params();
        Self {
            uniform: ModernSurfaceUniform {
                color: LinearRgba::from(fill),
                border_color: LinearRgba::from(border),
                shadow_color: LinearRgba::BLACK,
                dimensions,
                radius,
                border_width,
                smoothing: smoothing.clamp(0.0, 1.0),
                shadow_offset: Vec2::new(0.0, offset_y),
                shadow_blur: blur,
                shadow_spread: spread,
                ambient_alpha,
                key_alpha,
            },
        }
    }

    /// Create a simple squircle with fill and optional border, no shadow.
    pub fn squircle(dimensions: Vec2, radius: f32, smoothing: f32, fill: Color) -> Self {
        Self::card(
            dimensions,
            radius,
            smoothing,
            fill,
            Color::NONE,
            0.0,
            ModernSurfaceElevation::None,
        )
    }
}

/// Plugin registering modern surface squircle and shadow assets and shaders.
#[derive(Default)]
pub struct ModernSurfacePlugin;

impl Plugin for ModernSurfacePlugin {
    fn build(&self, app: &mut App) {
        // Shader libraries belong to the renderer. Headless tests need only the
        // material store and its handle-drop tracking, not another Shader store.
        if app.is_plugin_added::<RenderPlugin>() {
            shader_assets::install(app);
            embedded_asset!(app, "modern_surface.wesl");
            app.add_plugins(UiMaterialPlugin::<ModernSurfaceMaterial>::default());
        } else {
            app.init_asset::<ModernSurfaceMaterial>();
        }
        app.init_resource::<SurfaceShaderMode>();
        app.add_systems(PostUpdate, (release_retired, sync).after(UiSystems::Layout));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::asset::{AssetPlugin, Assets};
    #[cfg(test)]
    use std::f32::consts::FRAC_1_SQRT_2;

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

    #[test]
    fn test_sdf_squircle_distance_and_curvature() {
        let size = Vec2::new(120.0, 80.0);
        let radius = 16.0;

        let circular = SdfSquircle::new(size, radius, 0.0, 2.0);
        let superellipse = SdfSquircle::new(size, radius, 1.0, 2.0);

        // Center must be strictly inside (negative distance)
        assert!(circular.distance_at(Vec2::ZERO) < 0.0);
        assert!(superellipse.distance_at(Vec2::ZERO) < 0.0);

        // Far outside must be positive distance
        assert!(circular.distance_at(Vec2::new(200.0, 200.0)) > 0.0);
        assert!(superellipse.distance_at(Vec2::new(200.0, 200.0)) > 0.0);

        // At corner diagonal: (half_size - radius + radius * cos(45deg))
        // Superellipse with p=5.0 fills corner more than circle with p=2.0
        let corner_pt = (size * 0.5) - Vec2::splat(radius) + Vec2::splat(radius * FRAC_1_SQRT_2);
        let d_circ = circular.distance_at(corner_pt);
        let d_super = superellipse.distance_at(corner_pt);
        assert!(
            d_super < d_circ,
            "superellipse must fill corner more fully than circular arc (d_super={d_super}, d_circ={d_circ})"
        );

        // Anti-aliasing coverage
        assert_eq!(superellipse.coverage_at(Vec2::ZERO, 1.0), 1.0);
        assert_eq!(superellipse.coverage_at(Vec2::new(300.0, 300.0), 1.0), 0.0);

        // Border coverage
        let edge_pt = Vec2::new(size.x * 0.5 - 1.0, 0.0);
        let b_cov = superellipse.border_coverage_at(edge_pt, 1.0);
        assert!(b_cov > 0.0 && b_cov <= 1.0);
    }

    #[test]
    fn test_modern_surface_elevation_params() {
        let none = ModernSurfaceElevation::None.params();
        let low = ModernSurfaceElevation::Low.params();
        let medium = ModernSurfaceElevation::Medium.params();
        let high = ModernSurfaceElevation::High.params();

        assert_eq!(none.1, 0.0); // blur_radius
        assert!(low.1 < medium.1);
        assert!(medium.1 < high.1);
        assert!(low.3 < medium.3); // ambient_alpha
        assert!(medium.3 < high.3);
        assert!(low.4 < medium.4); // key_alpha
        assert!(medium.4 < high.4);
    }

    #[test]
    fn test_modern_surface_material_construction() {
        let card = ModernSurfaceMaterial::card(
            Vec2::new(280.0, 160.0),
            16.0,
            0.8,
            Color::srgba(0.1, 0.1, 0.1, 1.0),
            Color::srgba(0.3, 0.3, 0.3, 1.0),
            1.5,
            ModernSurfaceElevation::Medium,
        );

        assert_eq!(card.uniform.dimensions, Vec2::new(280.0, 160.0));
        assert_eq!(card.uniform.radius, 16.0);
        assert_eq!(card.uniform.smoothing, 0.8);
        assert_eq!(card.uniform.border_width, 1.5);
        assert_eq!(card.uniform.shadow_offset, Vec2::new(0.0, 6.0));
        assert_eq!(card.uniform.shadow_blur, 12.0);
        assert_eq!(card.uniform.shadow_spread, 1.0);

        let squircle =
            ModernSurfaceMaterial::squircle(Vec2::new(100.0, 100.0), 20.0, 0.5, Color::WHITE);
        assert_eq!(squircle.uniform.dimensions, Vec2::new(100.0, 100.0));
        assert_eq!(squircle.uniform.shadow_blur, 0.0);
    }

    #[test]
    fn test_modern_surface_plugin_registration() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(AssetPlugin::default());
        app.add_plugins(ModernSurfacePlugin);

        // Assets<ModernSurfaceMaterial> must exist and be accessible
        let assets = app.world().get_resource::<Assets<ModernSurfaceMaterial>>();
        assert!(
            assets.is_some(),
            "ModernSurfaceMaterial assets must be initialized by plugin"
        );
    }
}
