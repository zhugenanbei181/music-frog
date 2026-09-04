//! Network topology link & packet flow diagram: pure node-link layout,
//! smooth Bezier flow ribbons, GPU mesh generation, and Bevy scene adapter.
//!
//! Visualizes dynamic traffic propagation: Inbound Applications → Routing Rules →
//! Outbound Proxies / Direct Links, with proportional ribbon widths and status inks.

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

use super::bezier::{CubicBezierSegment, PlotPoint};
use super::mesh::{TelemetryMeshData, build_curve_ribbon_mesh};

/// Category classification for a topology node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeCategory {
    #[default]
    Inbound,
    Rule,
    Outbound,
    Direct,
    Reject,
}

/// A node in the topology network.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyNode {
    pub id: String,
    pub label: String,
    pub subtext: String,
    pub category: NodeCategory,
    pub x_fraction: f32,
    pub y_fraction: f32,
}

impl TopologyNode {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        category: NodeCategory,
        x_fraction: f32,
        y_fraction: f32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            subtext: String::new(),
            category,
            x_fraction,
            y_fraction,
        }
    }
}

/// A directional traffic flow link between two topology nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyLink {
    pub source_id: String,
    pub target_id: String,
    pub bandwidth_bps: f64,
    pub active_conns: u32,
    pub highlighted: bool,
}

impl TopologyLink {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        bandwidth_bps: f64,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            target_id: target_id.into(),
            bandwidth_bps,
            active_conns: 1,
            highlighted: false,
        }
    }
}

/// Specification of a Topology Link Graph.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologySpec {
    pub nodes: Vec<TopologyNode>,
    pub links: Vec<TopologyLink>,
    pub width: u32,
    pub height: u32,
}

impl Default for TopologySpec {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            width: 320,
            height: 140,
        }
    }
}

impl TopologySpec {
    pub fn new(
        nodes: Vec<TopologyNode>,
        links: Vec<TopologyLink>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            nodes,
            links,
            width,
            height,
        }
    }
}

/// Component holding the topology specification.
#[derive(Component, Clone, Debug, Default)]
pub struct TopologyPlate(pub TopologySpec);

/// Resolve category to token RGBA.
pub fn category_to_rgba(category: NodeCategory, palette: &UiPalette) -> [u8; 4] {
    let color = match category {
        NodeCategory::Inbound => palette.accent,
        NodeCategory::Rule => palette.icon_tile,
        NodeCategory::Outbound => palette.success,
        NodeCategory::Direct => palette.warning,
        NodeCategory::Reject => palette.danger,
    };
    crate::chart::to_rgba8(color)
}

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

/// Rasterize the topology diagram into RGBA8 pixels.
pub fn rasterize_topology(spec: &TopologySpec, palette: &UiPalette) -> Vec<u8> {
    let width = spec.width;
    let height = spec.height;
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    if width == 0 || height == 0 {
        return pixels;
    }

    let node_map: std::collections::HashMap<&str, &TopologyNode> =
        spec.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    // 1. Draw connecting Bezier ribbons
    for link in &spec.links {
        if let (Some(src), Some(dst)) = (
            node_map.get(link.source_id.as_str()),
            node_map.get(link.target_id.as_str()),
        ) {
            let p0 = PlotPoint::new(
                src.x_fraction * width as f32,
                src.y_fraction * height as f32,
            );
            let p1 = PlotPoint::new(
                dst.x_fraction * width as f32,
                dst.y_fraction * height as f32,
            );

            let dx = (p1.x - p0.x) * 0.5;
            let c1 = PlotPoint::new(p0.x + dx, p0.y);
            let c2 = PlotPoint::new(p1.x - dx, p1.y);

            let segment = CubicBezierSegment::new(p0, c1, c2, p1);
            let points = segment.sample_points(24);

            let link_color = if link.highlighted {
                crate::chart::to_rgba8(palette.accent)
            } else {
                crate::chart::to_rgba8(palette.border)
            };

            let thickness = if link.bandwidth_bps > 10_000_000.0 {
                3
            } else if link.bandwidth_bps > 1_000_000.0 {
                2
            } else {
                1
            };

            for pair in points.windows(2) {
                let a = pair[0];
                let b = pair[1];
                let xa = a.x.round() as i32;
                let xb = b.x.round() as i32;
                let span = (xb - xa).abs().max(1) as f32;

                for x in xa.min(xb)..=xa.max(xb) {
                    let t = (x - xa).abs() as f32 / span;
                    let y = (a.y + (b.y - a.y) * t).round() as i32;
                    for dy in -thickness..=thickness {
                        let alpha = if dy == 0 { 0.9 } else { 0.4 };
                        blend_pixel(&mut pixels, width, x, y + dy, link_color, alpha);
                    }
                }
            }
        }
    }

    // 2. Draw node pill cards
    let card_w = 20;
    let card_h = 10;
    for node in &spec.nodes {
        let cx = (node.x_fraction * width as f32).round() as i32;
        let cy = (node.y_fraction * height as f32).round() as i32;
        let color = category_to_rgba(node.category, palette);

        for dy in -card_h / 2..=card_h / 2 {
            for dx in -card_w / 2..=card_w / 2 {
                let is_border =
                    dx == -card_w / 2 || dx == card_w / 2 || dy == -card_h / 2 || dy == card_h / 2;
                let alpha = if is_border { 1.0 } else { 0.75 };
                blend_pixel(&mut pixels, width, cx + dx, cy + dy, color, alpha);
            }
        }
    }

    pixels
}

/// Convert TopologySpec to GPU Mesh.
pub fn build_topology_chart_mesh(spec: &TopologySpec, palette: &UiPalette) -> TelemetryMeshData {
    let mut mesh = TelemetryMeshData::new();
    let width = spec.width as f32;
    let height = spec.height as f32;

    let node_map: std::collections::HashMap<&str, &TopologyNode> =
        spec.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    for link in &spec.links {
        if let (Some(src), Some(dst)) = (
            node_map.get(link.source_id.as_str()),
            node_map.get(link.target_id.as_str()),
        ) {
            let p0 = PlotPoint::new(src.x_fraction * width, src.y_fraction * height);
            let p1 = PlotPoint::new(dst.x_fraction * width, dst.y_fraction * height);

            let dx = (p1.x - p0.x) * 0.5;
            let c1 = PlotPoint::new(p0.x + dx, p0.y);
            let c2 = PlotPoint::new(p1.x - dx, p1.y);

            let segment = CubicBezierSegment::new(p0, c1, c2, p1);
            let points: Vec<Option<PlotPoint>> =
                segment.sample_points(20).into_iter().map(Some).collect();

            let color = if link.highlighted {
                crate::chart::to_rgba8(palette.accent)
            } else {
                crate::chart::to_rgba8(palette.border)
            };

            let color_f32 = [
                color[0] as f32 / 255.0,
                color[1] as f32 / 255.0,
                color[2] as f32 / 255.0,
                0.8,
            ];

            let ribbon = build_curve_ribbon_mesh(&points, 2.5, color_f32);
            let base_idx = mesh.vertices.len() as u32;
            mesh.vertices.extend(ribbon.vertices);
            for idx in ribbon.indices {
                mesh.indices.push(base_idx + idx);
            }
        }
    }

    mesh
}

/// Create an Image asset from TopologySpec.
pub fn topology_image(spec: &TopologySpec, palette: &UiPalette) -> Image {
    let data = rasterize_topology(spec, palette);
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

/// The topology scene function.
pub fn topology_scene(spec: TopologySpec) -> impl Scene + use<> {
    let width_px = spec.width as f32;
    let height_px = spec.height as f32;
    bsn! {
        Node {
            width: percent(100),
            max_width: px(width_px),
            height: px(height_px),
            flex_shrink: 1.0,
        }
        TopologyPlate({ spec })
    }
}

/// Sync system for Topology charts.
pub fn sync_topology_charts(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    mut charts: Query<(Entity, &mut TopologyPlate, Option<&ImageNode>)>,
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
                    *image = topology_image(spec, &palette);
                }
            }
            None => {
                let image = topology_image(spec, &palette);
                let handle = images.add(image);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
    }
}
