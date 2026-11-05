//! The overview waveform: a WESL-glow strip for one authoritative series.
//!
//! The caller owns the samples; this widget only normalizes them for display.
//! The material is a compare-and-set projection of those samples and the live
//! palette, so a projection restamp or theme switch rewrites one uniform in
//! place and never rebuilds the tree.

use crate::palette::{UiPalette, rem};
use crate::theme::space;
use bevy::app::{App, Plugin, Update};
use bevy::asset::{Asset, AssetApp, Assets, Handle, embedded_asset};
use bevy::color::{Color, LinearRgba};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::math::Vec4;
use bevy::reflect::TypePath;
use bevy::render::RenderPlugin;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::scene::{Scene, bsn};
use bevy::shader::ShaderRef;
use bevy::ui::prelude::{Node, percent};
use bevy::ui_render::prelude::{UiMaterial, UiMaterialPlugin};
use bevy::ui_render::ui_material::MaterialNode;

/// How many normalized samples the shader ABI carries. The app pads or truncates
/// real histories to this fixed uniform width.
pub const WAVEFORM_SAMPLE_CAPACITY: usize = 64;

/// The semantic series carried by one waveform strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WaveformSeries {
    #[default]
    Upload,
    Download,
}

/// The authoritative waveform samples and their series role.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct WaveformPlate {
    pub series: WaveformSeries,
    /// Raw sample values in display order; the widget normalizes only for paint.
    pub samples: Vec<f32>,
}

/// The material handle owned by one waveform plate.
#[derive(Component)]
pub struct WaveformMaterialOwner(pub Handle<WaveformGlowMaterial>);

/// What the current material was projected from. Unchanged signatures cost nothing.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct WaveformMaterialPaint {
    pub samples_hash: u64,
    pub ink: Color,
}

/// The writable projection view for one waveform plate.
#[derive(QueryData)]
pub struct WaveformView {
    pub entity: Entity,
    pub plate: &'static WaveformPlate,
    pub binding: Option<&'static MaterialNode<WaveformGlowMaterial>>,
    pub owner: Option<&'static WaveformMaterialOwner>,
    pub paint: Option<&'static WaveformMaterialPaint>,
}

/// Uniform ABI for `waveform_glow.wesl`.
#[derive(ShaderType, Clone, Copy, Debug, PartialEq)]
pub struct WaveformUniform {
    pub samples: [Vec4; WAVEFORM_SAMPLE_CAPACITY / 4],
    pub ink: LinearRgba,
    pub sample_count: f32,
    pub opacity: f32,
    pub line_width: f32,
    pub glow_width: f32,
    pub glow_alpha: f32,
    pub fill_alpha: f32,
}

/// The UI material that paints one anti-aliased waveform and its soft glow.
#[derive(AsBindGroup, Asset, TypePath, Debug, Clone, PartialEq)]
pub struct WaveformGlowMaterial {
    #[uniform(0)]
    pub uniform: WaveformUniform,
}

impl UiMaterial for WaveformGlowMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://infiltrator_bevy_widgets/waveform_glow.wesl".into()
    }
}

/// The semantic ink for a waveform series. Pure function.
pub fn waveform_ink(series: WaveformSeries, palette: &UiPalette) -> Color {
    match series {
        WaveformSeries::Upload => palette.accent,
        WaveformSeries::Download => palette.success,
    }
}

/// Normalize raw samples into the shader's fixed display range.
pub fn normalized_waveform(samples: &[f32]) -> ([Vec4; WAVEFORM_SAMPLE_CAPACITY / 4], f32) {
    let peak = samples.iter().copied().fold(0.0_f32, f32::max);
    let mut normalized = [Vec4::splat(0.0); WAVEFORM_SAMPLE_CAPACITY / 4];
    if !peak.is_finite() || peak <= 0.0 {
        return (normalized, 0.0);
    }
    for (index, sample) in samples.iter().enumerate().take(WAVEFORM_SAMPLE_CAPACITY) {
        if sample.is_finite() && *sample > 0.0 {
            let value = (sample / peak).clamp(0.0, 1.0);
            let lane = index % 4;
            normalized[index / 4][lane] = value;
        }
    }
    (
        normalized,
        samples.len().min(WAVEFORM_SAMPLE_CAPACITY) as f32,
    )
}

/// A short signature so unchanged frames do not touch the material asset.
pub fn waveform_samples_hash(samples: &[f32]) -> u64 {
    samples.iter().fold(0u64, |hash, sample| {
        hash.wrapping_mul(31)
            .wrapping_add(u64::from(sample.to_bits()))
    })
}

/// Mount one waveform strip. The material is bound after layout by
/// [`sync_waveform_materials`], mirroring the shared surface material.
pub fn waveform_scene(series: WaveformSeries, samples: Vec<f32>) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                height: rem(space::S24),
                min_height: rem(space::S16),
            }
            WaveformPlate { series: { series }, samples: { samples } }
    }
}

/// Project every waveform plate into its owned material. Compare-and-set:
/// an unchanged series and palette never touches the asset store.
pub fn sync_waveform_materials(
    palette: Res<UiPalette>,
    materials: Option<ResMut<Assets<WaveformGlowMaterial>>>,
    mut plates: Query<WaveformView>,
    mut commands: Commands,
) {
    let Some(mut materials) = materials else {
        return;
    };
    for item in &mut plates {
        let WaveformViewItem {
            entity,
            plate,
            binding,
            owner,
            paint,
        } = item;
        let ink = waveform_ink(plate.series, &palette);
        let samples_hash = waveform_samples_hash(&plate.samples);
        if paint.is_some_and(|paint| {
            paint.samples_hash == samples_hash
                && paint.ink == ink
                && owner.is_some_and(|owner| materials.contains(&owner.0))
                && binding.is_some_and(|node| owner.is_some_and(|owner| node.0 == owner.0))
        }) {
            continue;
        }
        let (normalized, sample_count) = normalized_waveform(&plate.samples);
        let desired = WaveformGlowMaterial {
            uniform: WaveformUniform {
                samples: normalized,
                ink: LinearRgba::from(ink),
                sample_count,
                opacity: if sample_count > 0.0 { 1.0 } else { 0.0 },
                line_width: 0.06,
                glow_width: 0.30,
                glow_alpha: 0.50,
                fill_alpha: 0.20,
            },
        };
        let handle = match owner.filter(|owner| materials.contains(&owner.0)) {
            Some(owner) => {
                if let Some(mut existing) = materials.get_mut(&owner.0) {
                    *existing = desired;
                }
                owner.0.clone()
            }
            None => materials.add(desired),
        };
        if binding.is_none_or(|node| node.0 != handle) {
            commands.entity(entity).insert(MaterialNode(handle.clone()));
        }
        commands.entity(entity).insert((
            WaveformMaterialOwner(handle),
            WaveformMaterialPaint { samples_hash, ink },
        ));
    }
}

/// Register the shared waveform material and its paint system.
#[derive(Default)]
pub struct WaveformGlowPlugin;

impl Plugin for WaveformGlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<WaveformGlowMaterial>();
        if app.is_plugin_added::<RenderPlugin>() {
            embedded_asset!(app, "waveform_glow.wesl");
            app.add_plugins(UiMaterialPlugin::<WaveformGlowMaterial>::default());
        }
        app.add_systems(Update, sync_waveform_materials);
    }
}
