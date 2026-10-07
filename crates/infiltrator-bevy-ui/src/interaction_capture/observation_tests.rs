//! test-intent: behavior
use super::*;
use bevy::ecs::component::Component;
use bevy::ecs::system::SystemState;
use bevy::ecs::world::World;
use bevy::math::Vec2;
use bevy::math::{Affine2, Rect};

#[derive(Component)]
struct Inspection;

#[test]
fn native_inherited_clipping_rejects_a_partially_visible_or_hidden_capture_target() {
    let mut world = World::new();
    world.init_resource::<GeometryRejections>();
    let entity = world
        .spawn((
            Inspection,
            Node::default(),
            ComputedNode {
                size: Vec2::new(80.0, 40.0),
                ..Default::default()
            },
            UiGlobalTransform::from_xy(100.0, 80.0),
        ))
        .id();
    assert_eq!(
        SystemState::<CaptureGeometry>::new(&mut world)
            .get_mut(&mut world)
            .expect("capture geometry resources")
            .bounds(entity, "inspection"),
        Some([60.0, 60.0, 80.0, 40.0])
    );
    let mut clip = CalculatedClip::default();
    clip.push_rect(Rect::new(50.0, 50.0, 150.0, 110.0), Affine2::IDENTITY);
    world.entity_mut(entity).insert(clip.clone());
    assert_eq!(
        SystemState::<CaptureGeometry>::new(&mut world)
            .get_mut(&mut world)
            .expect("capture geometry resources")
            .bounds(entity, "inspection"),
        Some([60.0, 60.0, 80.0, 40.0])
    );
    clip.push_rect(Rect::new(50.0, 50.0, 150.0, 90.0), Affine2::IDENTITY);
    world.entity_mut(entity).insert(clip);
    assert_eq!(
        SystemState::<CaptureGeometry>::new(&mut world)
            .get_mut(&mut world)
            .expect("capture geometry resources")
            .bounds(entity, "inspection"),
        None
    );
    world
        .entity_mut(entity)
        .insert(CalculatedClip::FullyClipped);
    assert_eq!(
        SystemState::<CaptureGeometry>::new(&mut world)
            .get_mut(&mut world)
            .expect("capture geometry resources")
            .bounds(entity, "inspection"),
        None
    );
    world.entity_mut(entity).remove::<CalculatedClip>();
    assert_eq!(
        SystemState::<CaptureGeometry>::new(&mut world)
            .get_mut(&mut world)
            .expect("capture geometry resources")
            .bounds(entity, "inspection"),
        Some([60.0, 60.0, 80.0, 40.0])
    );
}
