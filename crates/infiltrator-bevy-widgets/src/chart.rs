//! Chart / sparkline: pure polyline projection + hand-written CPU
//! rasterizer + `ImageNode` adapter.
//!
//! **Pure core (zero bevy)**: [`polyline`] projects a sample series
//! (oldest → newest) onto points in a pixel box — min/max normalization
//! shows the recent shape, the absolute value lives in adjacent text.
//! Non-finite samples project to `None` and the rasterizer **breaks the line
//! there** — a gap never bridges (an absent observation must not be drawn as
//! a trend; taskmanager's sparkline semantics, independently implemented).
//!
//! **Rasterization**: [`rasterize`] paints the projected layers into an RGBA8
//! pixel buffer by hand (~100 lines of pure filling/lerping/blending — no
//! new external dependency, charter dependency whitelist). Draw order:
//! grid, then per layer the fade fill under the line, then the 2px line on
//! top. Anti-aliasing is deliberately absent: nearest-pixel honesty at
//! token colors, cheap enough to redo on every data tick.
//!
//! **Series updates** rewrite the **same image handle** ([`ChartPlate`]
//! keeps it; [`sync_charts`] mutates the asset in place — `Assets::<Image>::
//! get_mut` fires the modified event and bevy re-uploads the texture). The
//! pixel size is fixed at mount, so a rewrite never changes the extent;
//! re-adding a fresh handle per tick would churn asset ids and GPU uploads
//! for a low-frequency feed (Overview pump cadence) — write-back is the
//! cheaper honest option.
//!
//! Colors resolve from the palette at raster time: upper series accent,
//! lower series success, grid lines the border token, area fills the
//! token-derived washes. A theme switch re-rasterizes in place (same
//! handle, same entity).

use bevy::asset::{Assets, RenderAssetUsages};
use bevy::color::Color;
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

/// One projected point in pixel space, y growing downward (y == 0 is the top
/// of the plot box). `None` is a gap: the rasterizer never draws across it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPoint {
    pub x: f32,
    pub y: f32,
}

/// Project `samples` (oldest → newest) onto a polyline inside a
/// `width × height` box. Pure function; contract:
/// - empty input → empty output (the caller owns the honest empty state);
/// - a single sample → one point pinned to the newest edge at mid height
///   (one point has no range; mid is the neutral projection);
/// - finite input normalizes min→bottom, max→top, evenly spaced in x, the
///   newest sample on the right edge;
/// - a constant (or single-finite-value) series → a flat mid line;
/// - a non-finite sample → `None` at that slot (a gap, never a fabricated
///   value); with no finite observation at all, every slot is a gap.
pub fn polyline(samples: &[f32], width: f32, height: f32) -> Vec<Option<PlotPoint>> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &sample in samples {
        if sample.is_finite() {
            min = min.min(sample);
            max = max.max(sample);
        }
    }
    let range = max - min;
    let mid = height / 2.0;
    let xs = spaced(samples.len(), width);
    samples
        .iter()
        .zip(xs)
        .map(|(&sample, x)| {
            if !sample.is_finite() {
                return None;
            }
            let y = if range <= 0.0 {
                mid
            } else {
                let normalized = ((sample - min) / range).clamp(0.0, 1.0);
                // y grows downward: the max sits on the top edge, the min on
                // the bottom edge.
                height * (1.0 - normalized)
            };
            Some(PlotPoint { x, y })
        })
        .collect()
}

/// Even x positions across `width` for `count` samples: endpoints at 0 and
/// `width`, one position per sample. `count == 1` pins the lone sample to
/// the newest edge.
fn spaced(count: usize, width: f32) -> Vec<f32> {
    match count {
        0 => Vec::new(),
        1 => vec![width],
        _ => (0..count)
            .map(|index| width * index as f32 / (count - 1) as f32)
            .collect(),
    }
}

/// Straight-alpha RGBA8, the channel order [`TextureFormat::Rgba8UnormSrgb`]
/// expects. Token colors arrive as bevy [`Color`]; this is the one place
/// they become bytes.
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

/// The polyline thickness (px) — a 2px slab reads as a line, not a hairline.
pub const LINE_THICKNESS_PX: i32 = 2;

/// Horizontal grid lines: 1px rows at the given height fractions, in `ink`.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub fractions: Vec<f32>,
    pub ink: Rgba,
}

/// One drawable layer: projected points plus its token-resolved inks. The
/// fade fill under the line is optional (a sparkline may skip it).
#[derive(Clone, Debug, PartialEq)]
pub struct ChartLayer {
    pub points: Vec<Option<PlotPoint>>,
    pub line: Rgba,
    pub fill: Option<Rgba>,
}

/// Rasterize the layers into an RGBA8 pixel buffer of `width × height`:
/// grid first, then each layer (fade fill under its line, line on top).
/// Pure function — the headless test surface for the whole pixel contract.
pub fn rasterize(width: u32, height: u32, grid: Option<Grid>, layers: &[ChartLayer]) -> Vec<u8> {
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
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
            // Fade from the fill ink at the line to fully transparent at the
            // bottom edge — "渐隐", a gradient, not a second flat color.
            for y in start.max(0)..height_i {
                let reach = (height_i - top.round() as i32).max(1) as f32;
                let fade = 1.0 - ((y - top.round() as i32) as f32 / reach).clamp(0.0, 1.0);
                let alpha = (fill[3] as f32 / 255.0) * fade;
                blend(pixels, width, x as i32, y, fill, alpha);
            }
        }
    }
    // Line on top of its own fill: walk every finite run, lerp y per column,
    // stamp the 2px slab.
    for pair in layer.points.windows(2) {
        let (Some(a), Some(b)) = (pair[0], pair[1]) else {
            continue; // a gap: never bridge
        };
        let xa = (a.x.round() as i32).clamp(0, width as i32 - 1);
        let xb = (b.x.round() as i32).clamp(0, width as i32 - 1);
        let span = (xb - xa).max(1) as f32;
        for x in xa.min(xb)..=xa.max(xb) {
            let t = (x - xa) as f32 / span;
            let y = a.y + (b.y - a.y) * t;
            let base = y.round() as i32;
            for dy in 0..LINE_THICKNESS_PX {
                blend(pixels, width, x, base + dy, layer.line, 1.0);
            }
        }
    }
}

/// Topmost line y per column (the fill's upper edge), `None` under gaps.
fn column_tops(layer: &ChartLayer, width: u32) -> Vec<Option<f32>> {
    let mut tops: Vec<Option<f32>> = vec![None; width as usize];
    for pair in layer.points.windows(2) {
        let (Some(a), Some(b)) = (pair[0], pair[1]) else {
            continue;
        };
        let xa = (a.x.round() as i32).clamp(0, width as i32 - 1);
        let xb = (b.x.round() as i32).clamp(0, width as i32 - 1);
        let span = (xb - xa).max(1) as f32;
        for x in xa.min(xb)..=xa.max(xb) {
            let t = (x - xa) as f32 / span;
            let y = a.y + (b.y - a.y) * t;
            let slot = &mut tops[x as usize];
            *slot = Some(match *slot {
                Some(previous) => previous.min(y),
                None => y,
            });
        }
    }
    tops
}

/// Straight-alpha blend of `ink` at `alpha` over the pixel at `(x, y)` —
/// the classic "over" operator on straight (un-premultiplied) RGBA8, the
/// interpretation `Rgba8UnormSrgb` textures get. Out-of-range coordinates
/// are skipped (total, never panicking).
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

/// The chart data one node draws: the two series plus the fixed pixel box.
/// Plain data only (no handles) so the `bsn!` template machinery accepts it
/// wholesale, exactly like a `TextField`'s state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartSpec {
    /// Upper series (accent line over the accent wash).
    pub up: Vec<f32>,
    /// Lower series (success line over the success wash).
    pub down: Vec<f32>,
    /// Raster width (px) — fixed at mount; a resize is a remount.
    pub width: u32,
    /// Raster height (px) — fixed at mount; a resize is a remount.
    pub height: u32,
}

impl ChartSpec {
    /// A spec for both series at a fixed pixel box.
    pub fn new(up: Vec<f32>, down: Vec<f32>, width: u32, height: u32) -> Self {
        Self {
            up,
            down,
            width,
            height,
        }
    }
}

/// The chart mounted on a node. Hosts restamp the spec (component swap) when
/// new data arrives; [`sync_charts`] rasterizes and re-uploads in place —
/// the current image handle lives on the node's `ImageNode`, never in the
/// scene-stamped component.
#[derive(Component, Clone, Debug, Default)]
pub struct ChartPlate(pub ChartSpec);

/// Grid line fractions: thirds (the reference card's band structure).
const GRID_FRACTIONS: [f32; 2] = [1.0 / 3.0, 2.0 / 3.0];

/// Rasterize both series into an [`Image`] asset (Extent3d 2D, rgba8). The
/// colors resolve from the palette here — the rasterizer itself stays
/// token-free.
pub fn chart_image(
    up: &[f32],
    down: &[f32],
    width: u32,
    height: u32,
    palette: &UiPalette,
) -> Image {
    let grid = Grid {
        fractions: GRID_FRACTIONS.to_vec(),
        ink: to_rgba8(palette.border),
    };
    let layers = [
        ChartLayer {
            points: polyline(up, width as f32, height as f32),
            line: to_rgba8(palette.accent),
            fill: Some(to_rgba8(palette.chart_fill_up())),
        },
        ChartLayer {
            points: polyline(down, width as f32, height as f32),
            line: to_rgba8(palette.success),
            fill: Some(to_rgba8(palette.chart_fill_down())),
        },
    ];
    let data = rasterize(width, height, Some(grid), &layers);
    Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// The chart scene: a fixed pixel box carrying the plate; the image node is
/// stamped by [`sync_charts`] (the icon-plate idiom — scenes never touch
/// image assets). The scene deliberately takes no palette: a chart's inks
/// must come from the *live* palette at raster time, not a spawn-time
/// snapshot — stamping one here would go stale on a theme switch.
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
            height: px(height_px),
            flex_shrink: 1.0,
        }
        ChartPlate({ ChartSpec::new(up, down, width, height) })
    }
}

/// Draw every changed chart (and re-rasterize all of them on a theme
/// switch): first mount inserts the [`ImageNode`], later updates rewrite the
/// same image asset — see the module docs for why the handle never rotates.
/// A host without an image store degrades to an invisible box (never a
/// panic).
pub fn sync_charts(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    mut charts: Query<(Entity, &mut ChartPlate, Option<&ImageNode>)>,
    mut commands: Commands,
) {
    // A theme swap rewrites the palette resource; every plate re-derives its
    // inks from it, so re-rasterize everything once. Otherwise only plates
    // whose spec changed pay the raster cost.
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
                // Rewrite the asset under the existing handle: same id,
                // same extent, `Modified` event drives the re-upload.
                if let Some(mut image) = images.get_mut(&node.image) {
                    *image = chart_image(&spec.up, &spec.down, spec.width, spec.height, &palette);
                }
            }
            None => {
                let image = chart_image(&spec.up, &spec.down, spec.width, spec.height, &palette);
                let handle = images.add(image);
                commands.entity(entity).insert(ImageNode {
                    image: handle,
                    ..ImageNode::default()
                });
            }
        }
    }
}
