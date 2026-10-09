//! Optional 2.5D globe visualization with an explicit 2D fallback.
//!
//! This is a bounded, business-agnostic widget layer (BEVY-030 / BANDROID-016).
//! It renders a rotating orthographic globe of geo-tagged network nodes and
//! the great-circle traffic arcs between them, and it degrades without loss to
//! a flat 2D node-link view ([`GlobeMode::Flat2d`]) or a minimal 2D Eco view
//! ([`GlobeMode::Eco`]) when the optional sphere is disabled.
//!
//! **Honesty contract.** A coordinate can only exist as a [`GeoPoint`], which
//! carries a real [`GeoSource`] and [`GeoPrecision`] and is built through a
//! range-validating constructor. There is deliberately no `Default`, no
//! `From<&str>` and no path that derives a location from a node name, emoji or
//! latency. A node whose location is [`GeoLocation::Unknown`] — or merely
//! country-precision — is never plotted and never produces an arc.
//!
//! The heavy math is pure and headless-testable: [`GlobeProjection`] (yaw/pitch
//! rotation, back-hemisphere culling), [`build_globe_arcs`] and
//! [`rasterize_globe`]. The Bevy seam is the usual `bsn!` scene
//! ([`globe_scene`]) plus a compare-and-set repaint system
//! ([`sync_globe_charts`]) that does no work while the globe is static.

use crate::chart::texture::ChartTextureView;
use crate::chart::to_rgba8;
use crate::palette::UiPalette;
use crate::particle::{GreatCircleArc, ParticleEmitter, TrafficParticle};
use bevy::asset::{Assets, RenderAssetUsages};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::image::Image;
use bevy::math::{Vec2, Vec3};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::scene::{Scene, bsn};
use bevy::time::Time;
use bevy::ui::prelude::{Node, percent, px};
use std::collections::HashMap;
use std::f32::consts::TAU;

/// Arc altitude above the unit sphere, as a fraction of the radius.
pub const ARC_ALTITUDE_FRACTION: f32 = 0.12;

/// Hard upper bound on simultaneously live traffic particles.
pub const MAX_GLOBE_PARTICLES: usize = 128;

/// Segment count used when sampling a great-circle arc.
const ARC_SAMPLES: usize = 32;

/// Segment count used for the globe limb circle.
const LIMB_SAMPLES: usize = 96;

// ---------------------------------------------------------------------------
// Geographic provenance
// ---------------------------------------------------------------------------

/// Where a real coordinate came from. Every variant denotes an actual
/// geographic observation; there is deliberately no variant for a value
/// derived from a node name, emoji, latency or any other proxy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoSource {
    /// A GeoIP database (for example MaxMind) lookup for an observed address.
    GeoIpDatabase,
    /// A coordinate configured by the operator or the user.
    Configured,
    /// The endpoint itself reported its coordinate.
    EndpointReported,
}

/// How precise an observed coordinate is. `Country` carries no point:
/// plotting a country centroid would fabricate a location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoPrecision {
    /// Country-level only — deliberately not plottable.
    Country,
    /// City-level.
    City,
    /// Exact coordinates.
    Exact,
}

impl GeoPrecision {
    /// Only city- or exact-precision observations may be plotted.
    pub fn is_plottable(self) -> bool {
        matches!(self, GeoPrecision::City | GeoPrecision::Exact)
    }
}

/// Why a node has no plottable location. This is explicit data, never a
/// fallback coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeoUnknown {
    /// No observation has arrived yet.
    NotObserved,
    /// The source returned no geographic data.
    SourceEmpty,
    /// The source is disabled or unavailable.
    SourceUnavailable,
    /// Only country-level data is known; no point may be invented.
    CountryOnly,
}

/// A validated geographic coordinate in decimal degrees.
///
/// The fields are private and the only constructor validates finite range, so
/// a fabricated or out-of-range value can never become a `GeoPoint`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoPoint {
    lat_deg: f64,
    lon_deg: f64,
    source: GeoSource,
    precision: GeoPrecision,
}

impl GeoPoint {
    /// The only constructor. Returns `None` for non-finite or out-of-range
    /// input so no fabricated coordinate survives.
    pub fn new(
        lat_deg: f64,
        lon_deg: f64,
        source: GeoSource,
        precision: GeoPrecision,
    ) -> Option<Self> {
        if !lat_deg.is_finite() || !lon_deg.is_finite() {
            return None;
        }
        if !(-90.0..=90.0).contains(&lat_deg) || !(-180.0..=180.0).contains(&lon_deg) {
            return None;
        }
        Some(Self {
            lat_deg,
            lon_deg,
            source,
            precision,
        })
    }

    pub fn lat_deg(self) -> f64 {
        self.lat_deg
    }

    pub fn lon_deg(self) -> f64 {
        self.lon_deg
    }

    pub fn source(self) -> GeoSource {
        self.source
    }

    pub fn precision(self) -> GeoPrecision {
        self.precision
    }

    /// Whether this observation may become a plotted point.
    pub fn is_plottable(self) -> bool {
        self.precision.is_plottable()
    }
}

/// A node location: a real source-tagged coordinate, or an explicit unknown.
/// There is no third state and no `Default`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GeoLocation {
    Known(GeoPoint),
    Unknown(GeoUnknown),
}

impl GeoLocation {
    /// The plottable coordinate, or `None` for an unknown or country-only
    /// observation. This is the one gate every marker and arc must pass.
    pub fn plottable(self) -> Option<GeoPoint> {
        match self {
            GeoLocation::Known(point) if point.is_plottable() => Some(point),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Globe model
// ---------------------------------------------------------------------------

/// Category classification for a globe node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlobeNodeCategory {
    #[default]
    Home,
    Relay,
    Exit,
    Direct,
    Reject,
}

/// A geo-tagged node. Its location is required at construction, so a node can
/// never exist without a provenance-bearing [`GeoLocation`].
#[derive(Clone, Debug, PartialEq)]
pub struct GlobeNode {
    pub id: String,
    pub label: String,
    pub location: GeoLocation,
    pub category: GlobeNodeCategory,
    /// Adapter-supplied, already-normalized marker weight in `0..=1`. It only
    /// affects marker size, never position.
    pub weight: f32,
}

impl GlobeNode {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        location: GeoLocation,
        category: GlobeNodeCategory,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            location,
            category,
            weight: 0.5,
        }
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight.clamp(0.0, 1.0);
        self
    }
}

/// A directional traffic flow between two globe nodes.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobeLink {
    pub source_id: String,
    pub target_id: String,
    pub bandwidth_bps: f64,
    pub highlighted: bool,
}

impl GlobeLink {
    pub fn new(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        bandwidth_bps: f64,
    ) -> Self {
        Self {
            source_id: source_id.into(),
            target_id: target_id.into(),
            bandwidth_bps,
            highlighted: false,
        }
    }
}

/// The rendering mode. `Globe` is the optional 2.5D sphere; `Flat2d` is the
/// lossless 2D fallback that shows every known node; `Eco` is the minimal
/// degradation with no links or animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlobeMode {
    #[default]
    Globe,
    Flat2d,
    Eco,
}

impl GlobeMode {
    /// Whether this mode uses the sphere and back-hemisphere culling.
    pub fn is_sphere(self) -> bool {
        matches!(self, GlobeMode::Globe)
    }

    /// Whether this mode draws link geometry.
    pub fn draws_links(self) -> bool {
        matches!(self, GlobeMode::Globe | GlobeMode::Flat2d)
    }

    /// Whether this mode runs the rotation/flow animation at all.
    pub fn animates(self) -> bool {
        matches!(self, GlobeMode::Globe)
    }
}

/// Specification of a globe view.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobeSpec {
    pub nodes: Vec<GlobeNode>,
    pub links: Vec<GlobeLink>,
    pub width: u32,
    pub height: u32,
    /// Center longitude facing the viewer, in degrees.
    pub yaw_deg: f32,
    /// Center latitude facing the viewer, in degrees.
    pub pitch_deg: f32,
    /// Normalized animation phase for the traffic particles.
    pub flow_phase: f32,
    /// Adapter-supplied phase speed; generic widget code does not infer
    /// network semantics from link values.
    pub flow_speed: f32,
    /// Adapter-supplied auto-rotation speed in degrees per second.
    pub auto_rotate_speed_deg_per_sec: f32,
    pub mode: GlobeMode,
    /// Node id currently hovered, highlighting its arcs.
    pub hovered_node: Option<String>,
}

impl Default for GlobeSpec {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            links: Vec::new(),
            width: 320,
            height: 240,
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            flow_phase: 0.0,
            flow_speed: 1.0,
            auto_rotate_speed_deg_per_sec: 0.0,
            mode: GlobeMode::Globe,
            hovered_node: None,
        }
    }
}

impl GlobeSpec {
    pub fn new(nodes: Vec<GlobeNode>, links: Vec<GlobeLink>, width: u32, height: u32) -> Self {
        Self {
            nodes,
            links,
            width,
            height,
            ..Self::default()
        }
    }

    pub fn with_view(mut self, yaw_deg: f32, pitch_deg: f32) -> Self {
        self.yaw_deg = wrap_deg(yaw_deg);
        self.pitch_deg = clamp_pitch(pitch_deg);
        self
    }

    pub fn with_flow(mut self, phase: f32, speed: f32) -> Self {
        self.flow_phase = phase.fract();
        self.flow_speed = speed.max(0.0);
        self
    }

    pub fn with_mode(mut self, mode: GlobeMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_hovered_node(mut self, hovered_node: Option<String>) -> Self {
        self.hovered_node = hovered_node;
        self
    }

    /// Apply a pointer drag in logical pixels: horizontal drags yaw the globe,
    /// vertical drags pitch it within the ±89° clamp.
    pub fn drag(&mut self, delta_px: Vec2) {
        const DEG_PER_PX: f32 = 0.5;
        self.yaw_deg = wrap_deg(self.yaw_deg + delta_px.x * DEG_PER_PX);
        self.pitch_deg = clamp_pitch(self.pitch_deg + delta_px.y * DEG_PER_PX);
    }
}

fn wrap_deg(value: f32) -> f32 {
    (value + 180.0).rem_euclid(360.0) - 180.0
}

fn clamp_pitch(value: f32) -> f32 {
    value.clamp(-89.0, 89.0)
}

// ---------------------------------------------------------------------------
// Orthographic 2.5D projection
// ---------------------------------------------------------------------------

/// A projected point in logical screen pixels plus its front depth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedPoint {
    pub x: f32,
    pub y: f32,
    /// `0..=1`; larger is closer to the viewer.
    pub depth: f32,
}

/// A projected node marker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectedMarker {
    pub point: ProjectedPoint,
    pub weight: f32,
    pub category: GlobeNodeCategory,
}

/// An orthographic view: yaw/pitch rotation, a screen radius and center.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeProjection {
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub radius_px: f32,
    pub center: Vec2,
}

impl GlobeProjection {
    pub fn new(yaw_deg: f32, pitch_deg: f32, radius_px: f32, center: Vec2) -> Self {
        Self {
            yaw_deg,
            pitch_deg,
            radius_px,
            center,
        }
    }

    /// Fit the projection inside a spec's raster bounds.
    pub fn from_spec(spec: &GlobeSpec) -> Self {
        let radius_px = spec.width.min(spec.height) as f32 * 0.42;
        Self {
            yaw_deg: spec.yaw_deg,
            pitch_deg: spec.pitch_deg,
            radius_px,
            center: Vec2::new(spec.width as f32 * 0.5, spec.height as f32 * 0.5),
        }
    }

    /// Project a 3D direction. `radius_scale` lifts the point off the sphere
    /// for arc altitude. Returns `None` on the back hemisphere.
    pub fn project_direction(&self, direction: Vec3, radius_scale: f32) -> Option<ProjectedPoint> {
        let unit = direction.normalize_or_zero();
        let yaw = self.yaw_deg.to_radians();
        let pitch = self.pitch_deg.to_radians();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        // Yaw about the up axis (longitude -> depth).
        let x1 = unit.x * cos_yaw + unit.z * sin_yaw;
        let z1 = -unit.x * sin_yaw + unit.z * cos_yaw;
        let y1 = unit.y;
        // Pitch about the screen-horizontal axis (latitude -> depth).
        let x2 = x1 * cos_pitch + y1 * sin_pitch;
        let y2 = -x1 * sin_pitch + y1 * cos_pitch;
        if x2 <= 0.0 {
            return None; // back hemisphere
        }
        let scale = self.radius_px * radius_scale.max(0.0);
        Some(ProjectedPoint {
            x: self.center.x + z1 * scale,
            y: self.center.y - y2 * scale,
            depth: x2,
        })
    }

    /// Project a latitude/longitude pair in degrees.
    pub fn project_lat_lon(&self, lat_deg: f32, lon_deg: f32) -> Option<ProjectedPoint> {
        let direction = GreatCircleArc::lat_lon_to_cartesian(lat_deg, lon_deg, 1.0);
        self.project_direction(direction, 1.0)
    }

    /// Project a 3D point whose length is `sphere_radius` plus any altitude.
    pub fn project_point3(&self, point: Vec3, sphere_radius: f32) -> Option<ProjectedPoint> {
        let radius_scale = if sphere_radius > 0.0 {
            point.length() / sphere_radius
        } else {
            1.0
        };
        self.project_direction(point, radius_scale)
    }
}

/// Project every node with a plottable location, culling the back hemisphere.
pub fn project_globe_nodes(spec: &GlobeSpec, projection: &GlobeProjection) -> Vec<ProjectedMarker> {
    let mut markers = Vec::new();
    for node in &spec.nodes {
        let Some(geo) = node.location.plottable() else {
            continue;
        };
        let Some(point) = projection.project_lat_lon(geo.lat_deg() as f32, geo.lon_deg() as f32)
        else {
            continue;
        };
        markers.push(ProjectedMarker {
            point,
            weight: node.weight.clamp(0.0, 1.0),
            category: node.category,
        });
    }
    markers
}

/// A flat (equirectangular) node position, used by the 2D fallback modes.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatMarker {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub weight: f32,
    pub category: GlobeNodeCategory,
}

/// Project every node with a plottable location onto the flat map. Unlike the
/// sphere this is lossless: antipodal nodes are both visible.
pub fn project_flat_nodes(spec: &GlobeSpec) -> Vec<FlatMarker> {
    let width = spec.width as f32;
    let height = spec.height as f32;
    spec.nodes
        .iter()
        .filter_map(|node| {
            let geo = node.location.plottable()?;
            let x = ((geo.lon_deg() as f32 + 180.0) / 360.0) * width;
            let y = ((90.0 - geo.lat_deg() as f32) / 180.0) * height;
            Some(FlatMarker {
                id: node.id.clone(),
                x,
                y,
                weight: node.weight.clamp(0.0, 1.0),
                category: node.category,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Great-circle arcs and particles
// ---------------------------------------------------------------------------

/// A rendered traffic arc. It can only be built from two plottable locations.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobeArc {
    pub source_id: String,
    pub target_id: String,
    pub arc: GreatCircleArc,
    pub highlighted: bool,
    pub bandwidth_bps: f64,
}

/// Build arcs only between two plottable, known locations. A link touching an
/// unknown or country-only node can never produce an arc.
pub fn build_globe_arcs(spec: &GlobeSpec) -> Vec<GlobeArc> {
    let node_map: HashMap<&str, &GlobeNode> = spec
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let mut arcs = Vec::new();
    for link in &spec.links {
        let (Some(source), Some(target)) = (
            node_map.get(link.source_id.as_str()),
            node_map.get(link.target_id.as_str()),
        ) else {
            continue;
        };
        let (Some(from), Some(to)) = (source.location.plottable(), target.location.plottable())
        else {
            continue;
        };
        arcs.push(GlobeArc {
            source_id: link.source_id.clone(),
            target_id: link.target_id.clone(),
            arc: GreatCircleArc::new(
                Vec2::new(from.lat_deg() as f32, from.lon_deg() as f32),
                Vec2::new(to.lat_deg() as f32, to.lon_deg() as f32),
                1.0,
            ),
            highlighted: link.highlighted,
            bandwidth_bps: link.bandwidth_bps,
        });
    }
    arcs
}

fn arc_is_active(arc: &GlobeArc) -> bool {
    arc.highlighted || (arc.bandwidth_bps.is_finite() && arc.bandwidth_bps > 0.0)
}

/// Sample the active flow particles, reusing the shared [`ParticleEmitter`] so
/// the same bounded-capacity contract holds. No particle is emitted for an
/// inactive link or a non-animating mode.
pub fn build_globe_particles(spec: &GlobeSpec, max_particles: usize) -> ParticleEmitter {
    let mut emitter = ParticleEmitter::new(max_particles);
    if !spec.mode.animates() || spec.flow_speed <= 0.0 {
        return emitter;
    }
    for (index, arc) in build_globe_arcs(spec).iter().enumerate() {
        if !arc_is_active(arc) {
            continue;
        }
        let phase = (spec.flow_phase + index as f32 * 0.19).fract();
        for offset in [0.0_f32, 0.5] {
            let t = (phase + offset).fract();
            let position = arc.arc.sample_point(t, ARC_ALTITUDE_FRACTION);
            let particle = TrafficParticle::new(position, Vec3::ZERO, 1.0, Color::WHITE);
            if !emitter.emit(particle) {
                break;
            }
        }
    }
    emitter
}

/// Resolve a node category to token RGBA.
pub fn globe_category_to_rgba(category: GlobeNodeCategory, palette: &UiPalette) -> [u8; 4] {
    let color = match category {
        GlobeNodeCategory::Home => palette.accent,
        GlobeNodeCategory::Relay => palette.icon_tile,
        GlobeNodeCategory::Exit => palette.success,
        GlobeNodeCategory::Direct => palette.warning,
        GlobeNodeCategory::Reject => palette.danger,
    };
    to_rgba8(color)
}

// ---------------------------------------------------------------------------
// Rasterization
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Bevy seam
// ---------------------------------------------------------------------------

/// Component holding the globe specification.
#[derive(Component, Clone, Debug, Default)]
pub struct GlobePlate(pub GlobeSpec);

/// The last rasterized globe content. Its presence lets a static frame skip
/// both rasterization and upload; [`advance_globe_rotation`] only mutates the
/// spec while an animation is actually active.
#[derive(Component, Clone)]
pub(crate) struct GlobePaint {
    spec: GlobeSpec,
    palette: UiPalette,
}

impl GlobePaint {
    fn matches(&self, spec: &GlobeSpec, palette: &UiPalette) -> bool {
        self.palette == *palette && self.spec == *spec
    }
}

#[derive(QueryData)]
pub struct GlobeRenderView {
    entity: Entity,
    plate: &'static GlobePlate,
    paint: Option<&'static GlobePaint>,
    texture: ChartTextureView,
}

/// The globe scene function.
pub fn globe_scene(spec: GlobeSpec) -> impl Scene + use<> {
    let width_px = spec.width as f32;
    let height_px = spec.height as f32;
    bsn! {
            Node {
                width: percent(100),
                max_width: px(width_px),
                height: px(height_px),
                flex_shrink: 1.0,
            }
            GlobePlate({ spec })
    }
}

/// Sync system for globe charts. Repaints only when the content, view or
/// palette actually changed.
pub fn sync_globe_charts(
    palette: Res<UiPalette>,
    images: Option<ResMut<Assets<Image>>>,
    charts: Query<GlobeRenderView>,
    mut commands: Commands,
) {
    let Some(mut images) = images else {
        return;
    };
    for view in &charts {
        let (entity, plate, paint) = (view.entity, view.plate, view.paint);
        let spec = &plate.0;
        let repaint = !paint.is_some_and(|paint| paint.matches(spec, &palette));
        if view.texture.sync::<GlobePlate>(
            &mut images,
            repaint,
            || globe_image(spec, &palette),
            &mut commands,
        ) {
            commands.entity(entity).insert(GlobePaint {
                spec: spec.clone(),
                palette: *palette,
            });
        }
    }
}

/// Advance only mounted globe plates that are actually animating. A static
/// globe (no auto-rotation, no active flow) is left untouched, so the repaint
/// system above performs no per-frame work.
pub fn advance_globe_rotation(time: Res<Time>, mut charts: Query<&mut GlobePlate>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for mut plate in &mut charts {
        let spec = &mut plate.0;
        if !spec.mode.animates() {
            continue;
        }
        if spec.auto_rotate_speed_deg_per_sec != 0.0 {
            spec.yaw_deg = wrap_deg(spec.yaw_deg + spec.auto_rotate_speed_deg_per_sec * dt);
        }
        let has_active_flow = spec.flow_speed > 0.0
            && spec.links.iter().any(|link| {
                link.bandwidth_bps.is_finite() && (link.highlighted || link.bandwidth_bps > 0.0)
            });
        if has_active_flow {
            spec.flow_phase = (spec.flow_phase + dt * spec.flow_speed).fract();
        }
    }
}
