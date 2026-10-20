//! The globe's rasterizer: one RGBA8 buffer per rendered frame.
//!
//! The raster is the *measured* surface of the widget: its size is a raster
//! dimension (not a rem token), so the sizes here are the plate's own
//! pixel budget and never a UI length. The projection, the arc geometry and
//! the honesty gates live in the parent module; this module only paints what
//! they decided.

use crate::chart::to_rgba8;
use crate::globe::{
    ARC_ALTITUDE_FRACTION, GlobeArc, GlobeMode, GlobeProjection, GlobeSpec, MAX_GLOBE_PARTICLES,
    ProjectedPoint, build_globe_arcs, build_globe_particles, globe_category_to_rgba,
    project_flat_nodes,
};
use crate::palette::UiPalette;
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;
use std::f32::consts::TAU;

fn blend(pixels: &mut [u8], width: u32, x: i32, y: i32, ink: [u8; 4], alpha: f32) {
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
    for channel in 0..3 {
        let source = ink[channel] as f32 / 255.0;
        let destination = pixel[channel] as f32 / 255.0;
        let out = if out_alpha <= 0.0 {
            0.0
        } else {
            (source * source_alpha + destination * destination_alpha * (1.0 - source_alpha))
                / out_alpha
        };
        pixel[channel] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn draw_disc(
    pixels: &mut [u8],
    width: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    ink: [u8; 4],
    alpha: f32,
) {
    let radius = radius.max(0.5);
    let min_x = (cx - radius).floor() as i32;
    let max_x = (cx + radius).ceil() as i32;
    let min_y = (cy - radius).floor() as i32;
    let max_y = (cy + radius).ceil() as i32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= radius * radius {
                blend(pixels, width, x, y, ink, alpha);
            }
        }
    }
}

fn draw_line(
    pixels: &mut [u8],
    width: u32,
    from: Vec2,
    to: Vec2,
    ink: [u8; 4],
    alpha: f32,
    thickness: i32,
) {
    let steps = (to.x - from.x)
        .abs()
        .max((to.y - from.y).abs())
        .ceil()
        .max(1.0) as i32;
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        let x = from.x + (to.x - from.x) * t;
        let y = from.y + (to.y - from.y) * t;
        for oy in -thickness..=thickness {
            for ox in -thickness..=thickness {
                if ox * ox + oy * oy <= thickness * thickness {
                    blend(
                        pixels,
                        width,
                        x.round() as i32 + ox,
                        y.round() as i32 + oy,
                        ink,
                        alpha,
                    );
                }
            }
        }
    }
}

fn draw_limb(pixels: &mut [u8], width: u32, center: Vec2, radius: f32, ink: [u8; 4]) {
    if radius <= 0.0 {
        return;
    }
    let mut previous: Option<Vec2> = None;
    for step in 0..=LIMB_SAMPLES {
        let angle = TAU * step as f32 / LIMB_SAMPLES as f32;
        let point = Vec2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        );
        if let Some(previous) = previous {
            draw_line(pixels, width, previous, point, ink, 0.45, 0);
        }
        previous = Some(point);
    }
}

fn draw_sphere_arc(
    pixels: &mut [u8],
    width: u32,
    projection: &GlobeProjection,
    arc: &GlobeArc,
    ink: [u8; 4],
    highlighted: bool,
) {
    let thickness = if highlighted { 2 } else { 1 };
    let alpha = if highlighted { 0.95 } else { 0.55 };
    let mut previous: Option<ProjectedPoint> = None;
    for step in 0..=ARC_SAMPLES {
        let t = step as f32 / ARC_SAMPLES as f32;
        let point = projection.project_point3(arc.arc.sample_point(t, ARC_ALTITUDE_FRACTION), 1.0);
        if let (Some(a), Some(b)) = (previous, point) {
            draw_line(
                pixels,
                width,
                Vec2::new(a.x, a.y),
                Vec2::new(b.x, b.y),
                ink,
                alpha,
                thickness,
            );
        }
        previous = point;
    }
}

fn draw_flat(spec: &GlobeSpec, palette: &UiPalette, pixels: &mut [u8], draw_links: bool) {
    let markers = project_flat_nodes(spec);
    if draw_links {
        let positions: HashMap<&str, Vec2> = markers
            .iter()
            .map(|marker| (marker.id.as_str(), Vec2::new(marker.x, marker.y)))
            .collect();
        for link in &spec.links {
            let (Some(from), Some(to)) = (
                positions.get(link.source_id.as_str()),
                positions.get(link.target_id.as_str()),
            ) else {
                continue;
            };
            let ink = to_rgba8(if link.highlighted {
                palette.accent
            } else {
                palette.border
            });
            draw_line(pixels, spec.width, *from, *to, ink, 0.7, 1);
        }
    }
    for marker in &markers {
        let hovered = spec.hovered_node.as_deref() == Some(marker.id.as_str());
        let ink = if hovered {
            to_rgba8(palette.accent)
        } else {
            globe_category_to_rgba(marker.category, palette)
        };
        let radius = 2.0 + marker.weight * 3.0;
        draw_disc(pixels, spec.width, marker.x, marker.y, radius, ink, 1.0);
    }
}

fn draw_globe(spec: &GlobeSpec, palette: &UiPalette, pixels: &mut [u8]) {
    let projection = GlobeProjection::from_spec(spec);
    draw_limb(
        pixels,
        spec.width,
        projection.center,
        projection.radius_px,
        to_rgba8(palette.border),
    );

    for arc in &build_globe_arcs(spec) {
        let hovered = spec
            .hovered_node
            .as_deref()
            .is_some_and(|hovered| hovered == arc.source_id || hovered == arc.target_id);
        let highlighted = arc.highlighted || hovered;
        let ink = to_rgba8(if highlighted {
            palette.accent
        } else {
            palette.border
        });
        draw_sphere_arc(pixels, spec.width, &projection, arc, ink, highlighted);
    }

    let particle_ink = to_rgba8(palette.accent);
    for particle in &build_globe_particles(spec, MAX_GLOBE_PARTICLES).particles {
        if let Some(point) = projection.project_point3(particle.position, 1.0) {
            draw_disc(
                pixels,
                spec.width,
                point.x,
                point.y,
                2.0,
                particle_ink,
                0.95,
            );
        }
    }

    for node in &spec.nodes {
        let Some(geo) = node.location.plottable() else {
            continue;
        };
        let Some(point) = projection.project_lat_lon(geo.lat_deg() as f32, geo.lon_deg() as f32)
        else {
            continue;
        };
        let hovered = spec.hovered_node.as_deref() == Some(node.id.as_str());
        let ink = if hovered {
            to_rgba8(palette.accent)
        } else {
            globe_category_to_rgba(node.category, palette)
        };
        let radius = 2.0 + node.weight.clamp(0.0, 1.0) * 3.0 + point.depth * 1.5;
        draw_disc(pixels, spec.width, point.x, point.y, radius, ink, 1.0);
    }
}

/// Rasterize the globe (or its 2D fallback) into an RGBA8 buffer.
pub fn rasterize_globe(spec: &GlobeSpec, palette: &UiPalette) -> Vec<u8> {
    let width = spec.width;
    let height = spec.height;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    if width == 0 || height == 0 {
        return pixels;
    }
    match spec.mode {
        GlobeMode::Globe => draw_globe(spec, palette, &mut pixels),
        GlobeMode::Flat2d => draw_flat(spec, palette, &mut pixels, true),
        GlobeMode::Eco => draw_flat(spec, palette, &mut pixels, false),
    }
    pixels
}

/// Create an [`Image`] asset from a globe spec.
pub fn globe_image(spec: &GlobeSpec, palette: &UiPalette) -> Image {
    let data = rasterize_globe(spec, palette);
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

/// Segment count used for the globe limb circle.
const LIMB_SAMPLES: usize = 96;

/// Segment count used when sampling a great-circle arc.
const ARC_SAMPLES: usize = 32;
