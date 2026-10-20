//! The globe's pointer seam: the pointer resolves to a node by its real
//! identity, and a drag rotates the view.
//!
//! **Identity contract.** A pointer never resolves to an index, a screen
//! position or a guessed label: it resolves to the id of a node that carries
//! a real geographic observation, or to nothing at all. A node without a
//! plottable location can never be hit, exactly like it can never be drawn.

use crate::globe::{GlobeMode, GlobeProjection, GlobeSpec, GlobeView, project_flat_nodes};
use bevy::math::Vec2;

/// The pointer hit radius, in raster pixels.
///
/// The radius lives in the raster's own coordinate space (the globe is a
/// texture), so it is a raster measurement and not a rem token.
pub const GLOBE_HIT_RADIUS_PX: f32 = 12.0;

/// What the pointer is over on the globe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlobePointerTarget {
    /// The node under the pointer, by its real identity.
    NodeId(String),
    /// Nothing plottable is under the pointer.
    Empty,
}

impl GlobePointerTarget {
    /// The identity of the node under the pointer, if any.
    pub fn node_id(&self) -> Option<&str> {
        match self {
            GlobePointerTarget::NodeId(id) => Some(id.as_str()),
            GlobePointerTarget::Empty => None,
        }
    }
}

/// Resolve the pointer to the node under it, by real node identity.
///
/// The resolution never invents a node: it returns the identity of a node
/// that carries a plottable location, or [`GlobePointerTarget::Empty`] when
/// the pointer is over empty space or over the back hemisphere of the sphere.
pub fn globe_pointer_target(spec: &GlobeSpec, pointer: Vec2) -> GlobePointerTarget {
    let hit = match spec.mode {
        GlobeMode::Globe => sphere_hit(spec, pointer),
        GlobeMode::Flat2d | GlobeMode::Eco => flat_hit(spec, pointer),
    };
    match hit {
        Some(id) => GlobePointerTarget::NodeId(id),
        None => GlobePointerTarget::Empty,
    }
}

/// Drag the globe by a pointer delta and report the resulting view.
pub fn drag_globe(spec: &mut GlobeSpec, delta: Vec2) -> GlobeView {
    spec.drag(delta);
    spec.view()
}

fn sphere_hit(spec: &GlobeSpec, pointer: Vec2) -> Option<String> {
    let projection = GlobeProjection::from_spec(spec);
    spec.nodes.iter().find_map(|node| {
        let geo = node.location.plottable()?;
        let point = projection.project_lat_lon(geo.lat_deg() as f32, geo.lon_deg() as f32)?;
        let distance = Vec2::new(point.x, point.y).distance(pointer);
        (distance <= GLOBE_HIT_RADIUS_PX).then(|| node.id.clone())
    })
}

fn flat_hit(spec: &GlobeSpec, pointer: Vec2) -> Option<String> {
    project_flat_nodes(spec)
        .into_iter()
        .find(|marker| Vec2::new(marker.x, marker.y).distance(pointer) <= GLOBE_HIT_RADIUS_PX)
        .map(|marker| marker.id)
}

impl GlobeSpec {
    /// Point the hover at a resolved pointer target.
    ///
    /// The hovered node is stored by its real identity, so the renderer
    /// highlights exactly the node the pointer resolved to.
    pub fn hover(&mut self, target: &GlobePointerTarget) {
        self.hovered_node = target.node_id().map(str::to_owned);
    }
}
