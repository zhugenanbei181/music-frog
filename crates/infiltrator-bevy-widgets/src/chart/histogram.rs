//! Latency distribution histogram: tiered color coding, bar geometry,
//! GPU mesh generation, and Bevy scene adapter.
//!
//! Visualizes network latency bins (e.g. <50ms, 50-100ms, 100-200ms, >500ms, timeout)
//! with clear tier distinctions and average / P95 threshold markers.

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::image::Image;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, percent, px};
use bevy::ui::widget::ImageNode;

use crate::palette::UiPalette;

use super::mesh::{TelemetryMeshData, build_bar_mesh};

/// Latency severity classification tier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LatencyTier {
    /// Fast: < 100ms (Success green).
    #[default]
    Fast,
    /// Normal: 100ms - 300ms (Accent blue).
    Normal,
    /// Slow: 300ms - 800ms (Warning amber).
    Slow,
    /// Timeout / Packet Loss (Danger red).
    Timeout,
}

/// A single latency histogram bin.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramBin {
    pub label: String,
    pub min_ms: f32,
    pub max_ms: f32,
    pub count: u32,
    pub tier: LatencyTier,
}

impl HistogramBin {
    pub fn new(
        label: impl Into<String>,
        min_ms: f32,
        max_ms: f32,
        count: u32,
        tier: LatencyTier,
    ) -> Self {
        Self {
            label: label.into(),
            min_ms,
            max_ms,
            count,
            tier,
        }
    }
}

/// Specification for a Latency Histogram.
#[derive(Clone, Debug, PartialEq)]
pub struct HistogramSpec {
    pub bins: Vec<HistogramBin>,
    pub avg_latency_ms: Option<f32>,
    pub p95_latency_ms: Option<f32>,
    pub width: u32,
    pub height: u32,
}

impl Default for HistogramSpec {
    fn default() -> Self {
        Self {
            bins: Vec::new(),
            avg_latency_ms: None,
            p95_latency_ms: None,
            width: 240,
            height: 100,
        }
    }
}

impl HistogramSpec {
    pub fn new(bins: Vec<HistogramBin>, width: u32, height: u32) -> Self {
        Self {
            bins,
            avg_latency_ms: None,
            p95_latency_ms: None,
            width,
            height,
        }
    }
}

/// Component holding the latency histogram specification.
#[derive(Component, Clone, Debug, Default)]
pub struct HistogramPlate(pub HistogramSpec);

/// Resolve tier to RGBA color from palette.
pub fn tier_to_rgba(tier: LatencyTier, palette: &UiPalette) -> [u8; 4] {
    let color = match tier {
        LatencyTier::Fast => palette.success,
        LatencyTier::Normal => palette.accent,
        LatencyTier::Slow => palette.warning,
        LatencyTier::Timeout => palette.danger,
    };
    crate::chart::to_rgba8(color)
}

/// Straight-alpha pixel blend helper.
fn blend_pixel(pixels: &mut [u8], width: u32, x: i32, y: i32, ink: [u8; 4], alpha: f32) {
    if x < 0 || y < 0 || x >= width as i32 {
        return;
    }
    let offset = (y as usize * width as usize + x as usize) * 4;
    let Some(pixel) = pixels.get_mut(offset..offset + 4) else {
        return;
    };
    let source_alpha = alpha.clamp(0.0, 1.0);
    let destination_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    for c in 0..3 {
        let src = ink[c] as f32 / 255.0;
        let dst = pixel[c] as f32 / 255.0;
        let out = if out_alpha <= 0.0 {
            0.0
        } else {
            (src * source_alpha + dst * destination_alpha * (1.0 - source_alpha)) / out_alpha
        };
        pixel[c] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Rasterize the histogram into RGBA8 buffer.
pub fn rasterize_histogram(spec: &HistogramSpec, palette: &UiPalette) -> Vec<u8> {
    let width = spec.width;
    let height = spec.height;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    if width == 0 || height == 0 || spec.bins.is_empty() {
        return pixels;
    }

    let border_ink = crate::chart::to_rgba8(palette.border);
    let grid_y = (height as f32 * 0.5).round() as i32;
    for x in 0..width as i32 {
        blend_pixel(&mut pixels, width, x, grid_y, border_ink, 0.4);
    }

    let max_count = spec.bins.iter().map(|b| b.count).max().unwrap_or(1).max(1) as f32;

    let num_bins = spec.bins.len();
    let gap = 4.0f32;
    let total_gap = gap * (num_bins + 1) as f32;
    let available_w = (width as f32 - total_gap).max(num_bins as f32);
    let bar_w = (available_w / num_bins as f32).max(2.0);

    for (i, bin) in spec.bins.iter().enumerate() {
        let bar_x = gap + i as f32 * (bar_w + gap);
        let bar_h = ((bin.count as f32 / max_count) * (height as f32 - 8.0)).max(2.0);
        let bar_y = height as f32 - 2.0 - bar_h;
        let ink = tier_to_rgba(bin.tier, palette);

        let x_start = bar_x.round() as i32;
        let x_end = (bar_x + bar_w).round() as i32;
        let y_start = bar_y.round() as i32;
        let y_end = (height as i32 - 2).max(y_start + 1);

        for y in y_start..y_end {
            for x in x_start..x_end {
                // Top edge rounding / highlight
                let alpha = if y == y_start { 1.0 } else { 0.85 };
                blend_pixel(&mut pixels, width, x, y, ink, alpha);
            }
        }
    }

    pixels
}

/// Convert HistogramSpec to GPU Mesh.
pub fn build_histogram_chart_mesh(spec: &HistogramSpec, palette: &UiPalette) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();
    let width = spec.width as f32;
    let height = spec.height as f32;

    if spec.bins.is_empty() {
        return mesh;
    }

    let max_count = spec.bins.iter().map(|b| b.count).max().unwrap_or(1).max(1) as f32;

    let num_bins = spec.bins.len();
    let gap = 4.0f32;
    let total_gap = gap * (num_bins + 1) as f32;
    let available_w = (width - total_gap).max(num_bins as f32);
    let bar_w = (available_w / num_bins as f32).max(2.0);

    for (i, bin) in spec.bins.iter().enumerate() {
        let bar_x = gap + i as f32 * (bar_w + gap);
        let bar_h = ((bin.count as f32 / max_count) * (height - 8.0)).max(2.0);
        let bar_y = height - 2.0 - bar_h;
        let ink = tier_to_rgba(bin.tier, palette);
        let color_f32 = [
            ink[0] as f32 / 255.0,
            ink[1] as f32 / 255.0,
            ink[2] as f32 / 255.0,
            ink[3] as f32 / 255.0,
        ];

        let bar_mesh = build_bar_mesh(
            bar_x,
            bar_y,
            bar_w,
            bar_h,
            2.0,
            color_f32,
            [color_f32[0], color_f32[1], color_f32[2], color_f32[3] * 0.7],
        );

        let base_idx = mesh.vertices.len() as u32;
        mesh.vertices.extend(bar_mesh.vertices);
        for idx in bar_mesh.indices {
            mesh.indices.push(base_idx + idx);
        }
    }

    mesh
}

/// Create an Image asset from HistogramSpec.
pub fn histogram_image(spec: &HistogramSpec, palette: &UiPalette) -> Image {
    let data = rasterize_histogram(spec, palette);
    Image::new(
        Extent3d {
            width: spec.width,
            height: spec.height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// The histogram scene function.
pub fn histogram_scene(spec: HistogramSpec) -> impl Scene + use<> {
    let width_px = spec.width as f32;
    let height_px = spec.height as f32;
    bsn! {
        Node {
            width: percent(100),
            max_width: px(width_px),
            height: px(height_px),
            flex_shrink: 1.0,
        }
        HistogramPlate({ spec })
    }
}

/// Sync system for Histogram charts.
pub fn sync_histogram_charts(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    mut charts: Query<(Entity, &mut HistogramPlate, Option<&ImageNode>)>,
    mut commands: Commands,
) {
    let retheme = palette.is_changed();
    let Some(mut images) = images else {
        return;
    };

    for (entity, plate, node) in &mut charts {
        if !retheme && !plate.is_changed() && node.is_some() {
            continue;
        }
        let spec = &plate.0;
        match node {
            Some(node) => {
                if let Some(mut image) = images.get_mut(&node.image) {
                    *image = histogram_image(spec, &palette);
                }
            }
            None => {
                let image = histogram_image(spec, &palette);
                let handle = images.add(image);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
    }
}
