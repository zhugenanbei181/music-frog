//! Read-only native UI geometry; captures never receive unrestricted ECS access.
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut, SystemParam};
use bevy::math::Vec2;
use bevy::ui::{CalculatedClip, ComputedNode, Display, Node, UiGlobalTransform};
use std::collections::HashSet;

const CAPTURE_INTERACTION_CORE_INSET_PX: f32 = 12.0;

#[derive(Resource, Default)]
pub struct GeometryRejections(HashSet<String>);
#[derive(QueryData)]
pub struct LayoutNode {
    computed: &'static ComputedNode,
    transform: &'static UiGlobalTransform,
    clip: Option<&'static CalculatedClip>,
}
#[derive(SystemParam)]
pub struct CaptureGeometry<'w, 's> {
    layouts: Query<'w, 's, LayoutNode>,
    visibility: Query<'w, 's, &'static Node>,
    rejected: ResMut<'w, GeometryRejections>,
}
impl CaptureGeometry<'_, '_> {
    pub fn visible(&self, entity: Entity) -> bool {
        self.visibility
            .get(entity)
            .is_ok_and(|node| node.display != Display::None)
    }
    pub fn rect(&self, entity: Entity) -> Option<[f32; 4]> {
        let layout = self.layouts.get(entity).ok()?;
        let size = layout.computed.size();
        let origin = layout.transform.affine().translation - size / 2.0;
        (size.is_finite() && size.x > 0.0 && size.y > 0.0)
            .then_some([origin.x, origin.y, size.x, size.y])
    }
    /// Whether the inset interaction core is inside its clipping ancestors.
    ///
    /// The capture asks this question before it measures: a control that is
    /// not laid out yet only needs one more frame, while a control that is
    /// clipped away is answered by the official scroll-into-view. Neither
    /// answer is a rejection, so this question never logs.
    pub fn unclipped(&self, entity: Entity) -> bool {
        self.visible(entity) && self.clipped(entity) == Some(false)
    }
    /// Whether a corner of the inset interaction core falls outside its clip.
    ///
    /// `None` when the entity has no measured size yet: the surface has not
    /// laid it out, which is a different fact from "the surface clipped it".
    /// The outer inset may contain shadow decoration; the remaining core must
    /// stay wholly visible for the capture to target it.
    fn clipped(&self, entity: Entity) -> Option<bool> {
        let layout = self.layouts.get(entity).ok()?;
        let size = layout.computed.size();
        if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
            return None;
        }
        let half = size / 2.0;
        let shrink_half = (half - Vec2::splat(CAPTURE_INTERACTION_CORE_INSET_PX)).max(Vec2::ZERO);
        let core_corners = [
            Vec2::new(-shrink_half.x, -shrink_half.y),
            Vec2::new(shrink_half.x, -shrink_half.y),
            Vec2::new(-shrink_half.x, shrink_half.y),
            Vec2::new(shrink_half.x, shrink_half.y),
        ];
        Some(layout.clip.is_some_and(|clip| {
            core_corners.into_iter().any(|point| {
                !clip.contains_point(layout.transform.affine().transform_point2(point))
            })
        }))
    }
    pub fn bounds(&mut self, entity: Entity, label: &str) -> Option<[f32; 4]> {
        if !self.visible(entity) {
            return None;
        }
        let Ok(layout) = self.layouts.get(entity) else {
            return None;
        };
        let size = layout.computed.size();
        if !size.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
            let key = format!("{label}/empty");
            if self.rejected.0.insert(key.clone()) {
                eprintln!("capture geometry rejected {key}: size={size:?}");
            }
            return None;
        }
        if self.clipped(entity) == Some(true) {
            let key = format!("{label}/clipped");
            if self.rejected.0.insert(key.clone()) {
                eprintln!(
                    "capture geometry rejected {key}: size={size:?} transform={:?} clip={:?}",
                    layout.transform, layout.clip
                );
            }
            return None;
        }
        self.rect(entity)
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
