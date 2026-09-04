//! Annular ring traffic distribution chart: pure geometry projection,
//! anti-aliased CPU rasterizer, GPU mesh generation, and Bevy scene adapter.
//!
//! Visualizes protocol distribution, outbound routing shares, or domain traffic
//! breakdowns with proportioned annular sectors, gap padding, and center summaries.

use std::f32::consts::PI;

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::image::Image;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, px};
use bevy::ui::widget::ImageNode;

use crate::palette::UiPalette;

use super::mesh::{TelemetryMeshData, build_donut_sector_mesh};

/// Single slice of the donut chart.
#[derive(Clone, Debug, PartialEq)]
pub struct DonutSlice {
    pub label: String,
    pub value: f64,
    pub color: [u8; 4],
}

impl DonutSlice {
    pub fn new(label: impl Into<String>, value: f64, color: [u8; 4]) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}

/// Computed angular geometry for a donut slice.
#[derive(Clone, Debug, PartialEq)]
pub struct DonutSliceGeometry {
    pub slice_index: usize,
    pub label: String,
    pub start_angle: f32,
    pub end_angle: f32,
    pub fraction: f32,
    pub percentage: f32,
    pub color: [u8; 4],
}

/// Compute the radial geometry for all slices.
pub fn calculate_donut_geometry(
    slices: &[DonutSlice],
    pad_angle_rad: f32,
) -> Vec<DonutSliceGeometry> {
    if slices.is_empty() {
        return Vec::new();
    }

    let total: f64 = slices.iter().map(|s| s.value.max(0.0)).sum();
    if total <= 0.0 {
        return Vec::new();
    }

    let mut geometries = Vec::with_capacity(slices.len());
    let mut current_angle = -PI / 2.0; // Start at 12 o'clock

    for (idx, slice) in slices.iter().enumerate() {
        let val = slice.value.max(0.0);
        let fraction = (val / total) as f32;
        let percentage = fraction * 100.0;
        let angle_span = 2.0 * PI * fraction;

        let start_angle = current_angle;
        let end_angle = if slices.len() > 1 {
            (current_angle + angle_span - pad_angle_rad).max(start_angle)
        } else {
            current_angle + angle_span
        };

        geometries.push(DonutSliceGeometry {
            slice_index: idx,
            label: slice.label.clone(),
            start_angle,
            end_angle,
            fraction,
            percentage,
            color: slice.color,
        });

        current_angle += angle_span;
    }

    geometries
}

/// Specification of a Donut Chart.
#[derive(Clone, Debug, PartialEq)]
pub struct DonutChartSpec {
    pub slices: Vec<DonutSlice>,
    pub inner_radius_ratio: f32,
    pub pad_angle_rad: f32,
    pub width: u32,
    pub height: u32,
}

impl Default for DonutChartSpec {
    fn default() -> Self {
        Self {
            slices: Vec::new(),
            inner_radius_ratio: 0.65,
            pad_angle_rad: 0.04,
            width: 160,
            height: 160,
        }
    }
}

impl DonutChartSpec {
    pub fn new(slices: Vec<DonutSlice>, width: u32, height: u32) -> Self {
        Self {
            slices,
            inner_radius_ratio: 0.65,
            pad_angle_rad: 0.04,
            width,
            height,
        }
    }
}

/// Component holding the donut chart specification.
#[derive(Component, Clone, Debug, Default)]
pub struct DonutChartPlate(pub DonutChartSpec);

/// Rasterize the donut chart into straight-alpha RGBA8 pixels with anti-aliasing.
pub fn rasterize_donut(spec: &DonutChartSpec, palette: &UiPalette) -> Vec<u8> {
    let width = spec.width;
    let height = spec.height;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    if width == 0 || height == 0 {
        return pixels;
    }

    let cx = width as f32 / 2.0;
    let cy = height as f32 / 2.0;
    let max_radius = (cx.min(cy) - 2.0).max(1.0);
    let inner_radius = max_radius * spec.inner_radius_ratio.clamp(0.1, 0.9);
    let outer_radius = max_radius;

    let geometries = calculate_donut_geometry(&spec.slices, spec.pad_angle_rad);

    if geometries.is_empty() {
        // Empty state: render a faint neutral ring
        let border_rgba = crate::chart::to_rgba8(palette.border);
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                let r = (dx * dx + dy * dy).sqrt();

                let edge_in = (r - inner_radius).clamp(-1.0, 1.0) * 0.5 + 0.5;
                let edge_out = (outer_radius - r).clamp(-1.0, 1.0) * 0.5 + 0.5;
                let alpha_cov = edge_in.min(edge_out).clamp(0.0, 1.0);

                if alpha_cov > 0.0 {
                    let offset = ((y * width + x) * 4) as usize;
                    pixels[offset] = border_rgba[0];
                    pixels[offset + 1] = border_rgba[1];
                    pixels[offset + 2] = border_rgba[2];
                    pixels[offset + 3] = ((border_rgba[3] as f32 * alpha_cov * 0.4).round()) as u8;
                }
            }
        }
        return pixels;
    }

    // Rasterize slices
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let r = (dx * dx + dy * dy).sqrt();

            // Radial coverage
            let edge_in = (r - inner_radius).clamp(-0.5, 0.5) + 0.5;
            let edge_out = (outer_radius - r).clamp(-0.5, 0.5) + 0.5;
            let r_coverage = edge_in.min(edge_out).clamp(0.0, 1.0);

            if r_coverage <= 0.0 {
                continue;
            }

            // Angle in [-PI, PI], convert to [-PI/2, 3PI/2] matching geometry
            let mut angle = dy.atan2(dx);
            if angle < -PI / 2.0 {
                angle += 2.0 * PI;
            }

            for geo in &geometries {
                let in_angle = if geo.end_angle >= geo.start_angle {
                    angle >= geo.start_angle && angle <= geo.end_angle
                } else {
                    angle >= geo.start_angle || angle <= geo.end_angle
                };

                if in_angle {
                    let offset = ((y * width + x) * 4) as usize;
                    let alpha = geo.color[3] as f32 / 255.0 * r_coverage;
                    pixels[offset] = geo.color[0];
                    pixels[offset + 1] = geo.color[1];
                    pixels[offset + 2] = geo.color[2];
                    pixels[offset + 3] = (alpha * 255.0).round() as u8;
                    break;
                }
            }
        }
    }

    pixels
}

/// Convert DonutChartSpec to GPU Mesh.
pub fn build_donut_chart_mesh(spec: &DonutChartSpec) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();
    let cx = spec.width as f32 / 2.0;
    let cy = spec.height as f32 / 2.0;
    let max_radius = (cx.min(cy) - 2.0).max(1.0);
    let inner_radius = max_radius * spec.inner_radius_ratio.clamp(0.1, 0.9);
    let outer_radius = max_radius;

    let geometries = calculate_donut_geometry(&spec.slices, spec.pad_angle_rad);
    for geo in geometries {
        let color_f32 = [
            geo.color[0] as f32 / 255.0,
            geo.color[1] as f32 / 255.0,
            geo.color[2] as f32 / 255.0,
            geo.color[3] as f32 / 255.0,
        ];
        let sector_mesh = build_donut_sector_mesh(
            [cx, cy],
            inner_radius,
            outer_radius,
            geo.start_angle,
            geo.end_angle,
            color_f32,
            16,
        );

        let base_idx = mesh.vertices.len() as u32;
        mesh.vertices.extend(sector_mesh.vertices);
        for idx in sector_mesh.indices {
            mesh.indices.push(base_idx + idx);
        }
    }

    mesh
}

/// Generate an Image asset from DonutChartSpec.
pub fn donut_chart_image(spec: &DonutChartSpec, palette: &UiPalette) -> Image {
    let data = rasterize_donut(spec, palette);
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

/// The donut chart scene function.
pub fn donut_chart_scene(spec: DonutChartSpec) -> impl Scene + use<> {
    let width_px = spec.width as f32;
    let height_px = spec.height as f32;
    bsn! {
        Node {
            width: px(width_px),
            height: px(height_px),
            flex_shrink: 0.0,
        }
        DonutChartPlate({ spec })
    }
}

/// Sync system for Donut Charts: stamps ImageNode on mount, updates image asset on spec change or retheme.
pub fn sync_donut_charts(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    mut charts: Query<(Entity, &mut DonutChartPlate, Option<&ImageNode>)>,
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
                    *image = donut_chart_image(spec, &palette);
                }
            }
            None => {
                let image = donut_chart_image(spec, &palette);
                let handle = images.add(image);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
    }
}
