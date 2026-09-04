//! Chart / sparkline / multi-dimensional telemetry: smooth Bezier waveforms,
//! dual-series shared scaling, GPU 2D mesh generation, and Bevy scene adapters.
//!
//! **Pure core (zero bevy)**:
//! - [`bezier`]: Catmull-Rom to cubic Bezier interpolation, monotone clamping,
//!   and shared-scale dynamic normalization;
//! - [`mesh`]: GPU 2D vertex mesh compilation (`TriangleList`, colors, UVs) and
//!   embedded WGSL shader definition;
//! - [`donut`]: Annular ring traffic distribution chart with anti-aliased sectors;
//! - [`histogram`]: Network latency distribution bins with tiered colors;
//! - [`topology`]: Node-link packet propagation graphs with smooth flow ribbons;
//! - [`interaction`]: Interactive crosshair guidelines, hover tooltips, time-range
//!   zoom/pan, and O(N) LTTB downsampling;
//! - [`ring_buffer`]: Zero-allocation fixed ring buffer, rolling statistics, and
//!   background power throttling.
//!
//! **Rasterization**: [`rasterize`] paints projected layers into RGBA8 buffers:
//! background grid, area fade fills beneath curves, smooth line strokes on top,
//! and interactive crosshair overlays. Anti-aliasing and subpixel coverage provide
//! sharp, honest visuals at token colors.
//!
//! **Series updates** rewrite the **same image handle** in place ([`ChartPlate`]
//! keeps it; [`sync_charts`] mutates the asset in place via `Assets::<Image>::get_mut`).

pub mod bezier;
pub mod donut;
pub mod histogram;
pub mod interaction;
pub mod log_scale;
pub mod mesh;
pub mod nice_scale;
pub mod quantiles;
pub mod radar;
pub mod ring_buffer;
pub mod topology;

use bezier::{PlotPoint, ScaleMode, build_dual_series_curves, linear_polyline};
use interaction::{
    CrosshairState, TimeRangeZoom, apply_zoom_pan, draw_crosshair_overlay,
    find_nearest_sample_index,
};

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::image::Image;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, Overflow, percent, px};
use bevy::ui::widget::ImageNode;

use crate::palette::UiPalette;

/// Backwards-compatible linear projection function.
/// Projects `samples` onto a polyline inside a `width × height` box.
pub fn polyline(samples: &[f32], width: f32, height: f32) -> Vec<Option<PlotPoint>> {
    linear_polyline(samples, width, height, None)
}

/// Straight-alpha RGBA8, the channel order [`TextureFormat::Rgba8UnormSrgb`] expects.
pub fn to_rgba8(color: Color) -> Rgba {
    let srgba = color.to_srgba();
    [
        (srgba.red * 255.0).round() as u8,
        (srgba.green * 255.0).round() as u8,
        (srgba.blue * 255.0).round() as u8,
        (srgba.alpha * 255.0).round() as u8,
    ]
}

/// One RGBA8 pixel color.
pub type Rgba = [u8; 4];

/// The polyline thickness (px).
pub const LINE_THICKNESS_PX: i32 = 2;

/// Horizontal grid lines: 1px rows at the given height fractions, in `ink`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub fractions: Vec<f32>,
    pub ink: Rgba,
}

/// One drawable layer: projected points plus its token-resolved inks.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLayer {
    pub points: Vec<Option<PlotPoint>>,
    pub line: Rgba,
    pub fill: Option<Rgba>,
}

/// Rasterize the layers into an RGBA8 pixel buffer of `width × height`:
/// grid first, then each layer (fade fill under its line, line on top).
pub fn rasterize(width: u32, height: u32, grid: Option<Grid>, layers: &[ChartLayer]) -> Vec<u8> {
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    if width == 0 || height == 0 {
        return pixels;
    }

    if let Some(grid) = grid {
        for fraction in grid.fractions {
            let y = (fraction.clamp(0.0, 1.0) * height as f32).round() as i32;
            for x in 0..width as i32 {
                blend(&mut pixels, width, x, y, grid.ink, 1.0);
            }
        }
    }
    for layer in layers {
        draw_layer(&mut pixels, width, height, layer);
    }
    pixels
}

fn draw_layer(pixels: &mut [u8], width: u32, height: u32, layer: &ChartLayer) {
    let tops = column_tops(layer, width);
    if let Some(fill) = layer.fill {
        let height_i = height as i32;
        for (x, top) in tops.iter().enumerate() {
            let Some(top) = top else { continue };
            let start = top.round() as i32 + LINE_THICKNESS_PX;
            for y in start.max(0)..height_i {
                let reach = (height_i - top.round() as i32).max(1) as f32;
                let fade = 1.0 - ((y - top.round() as i32) as f32 / reach).clamp(0.0, 1.0);
                let alpha = (fill[3] as f32 / 255.0) * fade;
                blend(pixels, width, x as i32, y, fill, alpha);
            }
        }
    }

    for pair in layer.points.windows(2) {
        let (Some(a), Some(b)) = (pair[0], pair[1]) else {
            continue; // a gap: never bridge
        };
        let xa = (a.x.round() as i32).clamp(0, width as i32 - 1);
        let xb = (b.x.round() as i32).clamp(0, width as i32 - 1);
        let span = (xb - xa).abs().max(1) as f32;
        for x in xa.min(xb)..=xa.max(xb) {
            let t = (x - xa).abs() as f32 / span;
            let y = a.y + (b.y - a.y) * t;
            let base = y.round() as i32;
            for dy in 0..LINE_THICKNESS_PX {
                blend(pixels, width, x, base + dy, layer.line, 1.0);
            }
        }
    }
}

fn column_tops(layer: &ChartLayer, width: u32) -> Vec<Option<f32>> {
    let mut tops: Vec<Option<f32>> = vec![None; width as usize];
    for pair in layer.points.windows(2) {
        let (Some(a), Some(b)) = (pair[0], pair[1]) else {
            continue;
        };
        let xa = (a.x.round() as i32).clamp(0, width as i32 - 1);
        let xb = (b.x.round() as i32).clamp(0, width as i32 - 1);
        let span = (xb - xa).abs().max(1) as f32;
        for x in xa.min(xb)..=xa.max(xb) {
            let t = (x - xa).abs() as f32 / span;
            let y = a.y + (b.y - a.y) * t;
            let slot = &mut tops[x as usize];
            *slot = Some(match *slot {
                Some(prev) => prev.min(y),
                None => y,
            });
        }
    }
    tops
}

fn blend(pixels: &mut [u8], width: u32, x: i32, y: i32, ink: Rgba, alpha: f32) {
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

/// The chart data one node draws: the two series, dimensions, scaling, and interaction states.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartSpec {
    /// Upper series (accent line over the accent wash).
    pub up: Vec<f32>,
    /// Lower series (success line over the success wash).
    pub down: Vec<f32>,
    /// Raster width (px).
    pub width: u32,
    /// Raster height (px).
    pub height: u32,
    /// Dynamic scaling strategy.
    pub scale_mode: ScaleMode,
    /// Whether smooth Bezier interpolation is enabled.
    pub smooth: bool,
    /// Optional crosshair inspection state.
    pub crosshair: Option<CrosshairState>,
    /// Optional time-range zoom/pan window.
    pub zoom: Option<TimeRangeZoom>,
}

impl Default for ChartSpec {
    fn default() -> Self {
        Self {
            up: Vec::new(),
            down: Vec::new(),
            width: 0,
            height: 0,
            scale_mode: ScaleMode::Shared,
            smooth: true,
            crosshair: None,
            zoom: None,
        }
    }
}

impl ChartSpec {
    /// Create a new chart specification with shared scaling and smooth curves enabled by default.
    pub fn new(up: Vec<f32>, down: Vec<f32>, width: u32, height: u32) -> Self {
        Self {
            up,
            down,
            width,
            height,
            scale_mode: ScaleMode::Shared,
            smooth: true,
            crosshair: None,
            zoom: None,
        }
    }

    pub fn with_scale_mode(mut self, scale_mode: ScaleMode) -> Self {
        self.scale_mode = scale_mode;
        self
    }

    pub fn with_smooth(mut self, smooth: bool) -> Self {
        self.smooth = smooth;
        self
    }

    pub fn with_crosshair(mut self, crosshair: Option<CrosshairState>) -> Self {
        self.crosshair = crosshair;
        self
    }

    pub fn with_zoom(mut self, zoom: Option<TimeRangeZoom>) -> Self {
        self.zoom = zoom;
        self
    }
}

/// The chart mounted on a node.
#[derive(Component, Clone, Debug, Default)]
pub struct ChartPlate(pub ChartSpec);

/// Grid line fractions: thirds (the reference card's band structure).
const GRID_FRACTIONS: [f32; 2] = [1.0 / 3.0, 2.0 / 3.0];

/// Rasterize the chart into an [`Image`] asset.
pub fn chart_image_with_spec(spec: &ChartSpec, palette: &UiPalette) -> Image {
    let width = spec.width;
    let height = spec.height;

    let up_samples = if let Some(ref zoom) = spec.zoom {
        apply_zoom_pan(&spec.up, zoom)
    } else {
        spec.up.clone()
    };

    let down_samples = if let Some(ref zoom) = spec.zoom {
        apply_zoom_pan(&spec.down, zoom)
    } else {
        spec.down.clone()
    };

    let (up_points, down_points, _scale) = build_dual_series_curves(
        &up_samples,
        &down_samples,
        width as f32,
        height as f32,
        spec.scale_mode,
        spec.smooth,
    );

    let grid = Grid {
        fractions: GRID_FRACTIONS.to_vec(),
        ink: to_rgba8(palette.border),
    };

    let layers = [
        ChartLayer {
            points: up_points,
            line: to_rgba8(palette.accent),
            fill: Some(to_rgba8(palette.chart_fill_up())),
        },
        ChartLayer {
            points: down_points,
            line: to_rgba8(palette.success),
            fill: Some(to_rgba8(palette.chart_fill_down())),
        },
    ];

    let mut data = rasterize(width, height, Some(grid), &layers);

    if let Some(ref crosshair) = spec.crosshair {
        let snapped_idx = crosshair.snapped_index.or_else(|| {
            find_nearest_sample_index(
                up_samples.len().max(down_samples.len()),
                width as f32,
                crosshair.cursor_x,
            )
        });

        let up_pt = snapped_idx.and_then(|i| {
            if i < up_samples.len() {
                linear_polyline(&up_samples, width as f32, height as f32, None)
                    .get(i)
                    .copied()
                    .flatten()
            } else {
                None
            }
        });

        let down_pt = snapped_idx.and_then(|i| {
            if i < down_samples.len() {
                linear_polyline(&down_samples, width as f32, height as f32, None)
                    .get(i)
                    .copied()
                    .flatten()
            } else {
                None
            }
        });

        draw_crosshair_overlay(&mut data, width, height, crosshair, up_pt, down_pt, palette);
    }

    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Backwards-compatible `chart_image` function.
pub fn chart_image(
    up: &[f32],
    down: &[f32],
    width: u32,
    height: u32,
    palette: &UiPalette,
) -> Image {
    let spec = ChartSpec::new(up.to_vec(), down.to_vec(), width, height);
    chart_image_with_spec(&spec, palette)
}

/// The chart scene constructor.
pub fn chart_scene(
    up: Vec<f32>,
    down: Vec<f32>,
    width_px: f32,
    height_px: f32,
) -> impl Scene + use<> {
    let width = width_px.round().max(1.0) as u32;
    let height = height_px.round().max(1.0) as u32;
    bsn! {
        Node {
            width: percent(100),
            max_width: px(width_px),
            min_width: px(0.0),
            height: px(height_px),
            min_height: px(height_px),
            flex_shrink: 1.0,
            overflow: Overflow::clip(),
        }
        ChartPlate({ ChartSpec::new(up, down, width, height) })
    }
}

/// Draw every changed chart (and re-rasterize all of them on a theme switch).
pub fn sync_charts(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    mut charts: Query<(Entity, &ChartPlate, Option<&ImageNode>)>,
    mut commands: Commands,
) {
    let Some(mut images) = images else {
        return;
    };
    for (entity, plate, node) in &mut charts {
        let spec = &plate.0;
        match node {
            Some(node) => {
                if let Some(mut image) = images.get_mut(&node.image) {
                    *image = chart_image_with_spec(spec, &palette);
                }
            }
            None => {
                let image = chart_image_with_spec(spec, &palette);
                let handle = images.add(image);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
    }
}
