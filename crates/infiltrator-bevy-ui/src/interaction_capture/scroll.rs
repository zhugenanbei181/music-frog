//! Scroll actual SDK surfaces to expose a complete interaction region before measuring clipping.
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Commands;
use bevy::input::{mouse::MouseScrollUnit, touch::TouchPhase};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerScroll};
use bevy::picking::pointer::{Location, PointerId};
pub fn request_scroll(
    commands: &mut Commands,
    scroll_entity: Entity,
    bounds: [f32; 4],
    scroll: [f32; 4],
) -> bool {
    if bounds[3] > scroll[3] {
        return false;
    }
    let delta = if bounds[1] < scroll[1] {
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
