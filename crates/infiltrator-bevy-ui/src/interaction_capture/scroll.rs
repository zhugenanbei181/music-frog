//! Scroll actual SDK surfaces to expose a complete interaction region before measuring clipping.
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query};
use bevy::input::{mouse::MouseScrollUnit, touch::TouchPhase};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerScroll};
use bevy::picking::pointer::{Location, PointerId};
use bevy::ui_widgets::ScrollArea;

/// The scroll area that owns a node: its nearest scrolling ancestor.
///
/// The official scroll behaviour answers a wheel event on the entity that
/// carries the `ScrollArea` marker, so a capture must address that entity —
/// never the node it wants to reveal.
pub fn owning_scroll_area(
    entity: Entity,
    parents: &Query<&'static ChildOf>,
    areas: &Query<Entity, With<ScrollArea>>,
) -> Option<Entity> {
    let mut current = entity;
    while let Ok(child_of) = parents.get(current) {
        current = child_of.parent();
        if areas.contains(current) {
            return Some(current);
        }
    }
    None
}

/// The page's own scroll area: the outermost scrolling ancestor of a node.
///
/// A panel inside a page usually owns a nested scroll area of its own. The
/// page's viewport is the outermost one, so it is the area that has to move
/// when a control still sits below the fold of the page itself.
pub fn outermost_scroll_area(
    entity: Entity,
    parents: &Query<&'static ChildOf>,
    areas: &Query<Entity, With<ScrollArea>>,
) -> Option<Entity> {
    let mut current = entity;
    let mut outermost = None;
    while let Ok(child_of) = parents.get(current) {
        current = child_of.parent();
        if areas.contains(current) {
            outermost = Some(current);
        }
    }
    outermost
}

/// Scroll the owning viewport until the target's box is inside it.
///
/// Returns `true` when the target is already fully visible; `false` means a
/// scroll was requested (or the target can never fit), so the caller waits
/// for the next frame before it measures the control.
pub fn request_scroll(
    commands: &mut Commands,
    scroll_entity: Entity,
    bounds: [f32; 4],
    scroll: [f32; 4],
) -> bool {
    let delta = if bounds[3] > scroll[3] {
        if bounds[1] != scroll[1] {
            bounds[1] - scroll[1]
        } else {
            return true;
        }
    } else if bounds[1] < scroll[1] {
        bounds[1] - scroll[1]
    } else if bounds[1] + bounds[3] > scroll[1] + scroll[3] {
        bounds[1] + bounds[3] - scroll[1] - scroll[3] + 2.0
    } else {
        return true;
    };
    commands.trigger(PointerScroll {
        entity: scroll_entity,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: scroll[2] as u32,
                    height: scroll[3] as u32,
                },
                position: Vec2::new(scroll[0] + scroll[2] / 2.0, scroll[1] + scroll[3] / 2.0),
            },
        ),
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: -delta,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        phase: TouchPhase::Moved,
    });
    false
}

/// The smallest box that holds every one of the given boxes.
///
/// A receipt that needs two controls on screen at once scrolls until *both*
/// of them are rendered, so the region it measures is the union of the two.
pub fn union(boxes: &[[f32; 4]]) -> Option<[f32; 4]> {
    let mut iter = boxes.iter();
    let first = *iter.next()?;
    let mut result = first;
    for next in iter {
        let left = result[0].min(next[0]);
        let top = result[1].min(next[1]);
        let right = (result[0] + result[2]).max(next[0] + next[2]);
        let bottom = (result[1] + result[3]).max(next[1] + next[3]);
        result = [left, top, right - left, bottom - top];
    }
    Some(result)
}
