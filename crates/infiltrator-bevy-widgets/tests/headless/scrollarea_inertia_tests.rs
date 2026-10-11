//! Headless scroll inertia and reduce-motion behavior coverage.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerDrag, PointerDragEnd, PointerDragStart};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::scene::ScenePlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ScrollPosition;
use bevy::ui::prelude::{ComputedNode, Node, Overflow};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::scrollarea::{
    SCROLL_INERTIA_MAX_SPEED, SCROLL_INERTIA_MIN_SPEED, ScrollMotion, ScrollMotionPreference,
    ScrollMotionState, ScrollVelocitySampling,
};
use infiltrator_bevy_widgets::theme::Theme;
use std::time::Duration;
fn touch_pointer(id: u64) -> Pointer {
    Pointer::new(
        PointerId::Touch(id),
        Location {
            target: NormalizedRenderTarget::None {
                width: 300,
                height: 400,
            },
            position: Vec2::new(20.0, 200.0),
        },
    )
}

fn trigger_drag_start(app: &mut App, entity: Entity, pointer: &Pointer) {
    let camera = app.world_mut().spawn_empty().id();
    app.world_mut().trigger(PointerDragStart {
        entity,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        hit: HitData::new(camera, 0.0, None, None),
    });
}

fn trigger_drag_move(app: &mut App, entity: Entity, pointer: &Pointer, delta: Vec2) {
    app.world_mut().trigger(PointerDrag {
        entity,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        distance: delta,
        delta,
    });
}

fn trigger_drag_end(app: &mut App, entity: Entity, pointer: &Pointer, delta: Vec2) {
    app.world_mut().trigger(PointerDragEnd {
        entity,
        pointer: pointer.clone(),
        button: PointerButton::Primary,
        distance: delta,
    });
}

fn trigger_scroll_drag(app: &mut App, entity: Entity, id: u64, delta: Vec2) {
    let pointer = touch_pointer(id);
    trigger_drag_start(app, entity, &pointer);
    app.update();
    trigger_drag_move(app, entity, &pointer, delta);
    app.update();
    trigger_drag_end(app, entity, &pointer, delta);
}

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn released_scroll_drag_coasts_and_stops_at_both_content_bounds() {
    for (initial, delta, expected_direct, expected_bound) in [
        (400.0, Vec2::new(0.0, -40.0), 440.0, 600.0),
        (200.0, Vec2::new(0.0, 40.0), 160.0, 0.0),
    ] {
        let mut app = headless_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            20,
        )));
        let root = app
            .world_mut()
            .spawn((
                Node {
                    overflow: Overflow::scroll_y(),
                    ..Default::default()
                },
                ScrollArea,
                ScrollPosition(Vec2::new(0.0, initial)),
            ))
            .id();
        app.world_mut().entity_mut(root).insert(ComputedNode {
            size: Vec2::new(300.0, 400.0),
            content_size: Vec2::new(300.0, 1000.0),
            inverse_scale_factor: 1.0,
            ..Default::default()
        });
        app.update();

        trigger_scroll_drag(&mut app, root, 40, delta);
        assert_eq!(
            app.world().get::<ScrollPosition>(root).unwrap().y,
            expected_direct
        );
        assert!(matches!(
            app.world().resource::<ScrollMotionState>().motion(root),
            Some(ScrollMotion::Coasting { velocity })
                if velocity.length() <= SCROLL_INERTIA_MAX_SPEED
                    && velocity.length() >= SCROLL_INERTIA_MIN_SPEED
        ));

        app.update();
        let after_first_coast = app.world().get::<ScrollPosition>(root).unwrap().y;
        if expected_bound > expected_direct {
            assert!(after_first_coast > expected_direct);
        } else {
            assert!(after_first_coast < expected_direct);
        }
        for _ in 0..100 {
            app.update();
        }
        let final_position = app.world().get::<ScrollPosition>(root).unwrap().y;
        assert_eq!(final_position, expected_bound);
        assert_eq!(
            app.world().resource::<ScrollMotionState>().motion(root),
            None
        );
    }
}

#[test]
fn reduced_motion_stops_scroll_without_a_coast() {
    let mut app = headless_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        20,
    )));
    *app.world_mut().resource_mut::<ScrollMotionPreference>() = ScrollMotionPreference::Reduced;
    let root = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
        ))
        .id();
    app.world_mut().entity_mut(root).insert(ComputedNode {
        size: Vec2::new(300.0, 400.0),
        content_size: Vec2::new(300.0, 1000.0),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
    app.update();

    trigger_scroll_drag(&mut app, root, 41, Vec2::new(0.0, -40.0));
    let direct_position = app.world().get::<ScrollPosition>(root).unwrap().y;
    assert_eq!(direct_position, 40.0);
    assert_eq!(
        app.world().resource::<ScrollMotionState>().motion(root),
        None
    );

    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().get::<ScrollPosition>(root).unwrap().y,
        direct_position
    );
}

#[test]
fn stale_frame_gap_cannot_turn_a_resumed_drag_into_a_fling() {
    let mut app = headless_app();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        500,
    )));
    let root = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
        ))
        .id();
    app.world_mut().entity_mut(root).insert(ComputedNode {
        size: Vec2::new(300.0, 400.0),
        content_size: Vec2::new(300.0, 1000.0),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
    app.update();

    let pointer = touch_pointer(42);
    trigger_drag_start(&mut app, root, &pointer);
    app.update();
    trigger_drag_move(&mut app, root, &pointer, Vec2::new(0.0, -60.0));

    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 60.0);
    assert!(matches!(
        app.world().resource::<ScrollMotionState>().motion(root),
        Some(ScrollMotion::Direct {
            velocity,
            sampling: ScrollVelocitySampling::Stale { .. },
            ..
        }) if *velocity == Vec2::ZERO
    ));

    app.update();
    trigger_drag_end(&mut app, root, &pointer, Vec2::new(0.0, -60.0));
    assert_eq!(
        app.world().resource::<ScrollMotionState>().motion(root),
        None
    );
}

#[test]
fn direct_motion_state_is_pruned_after_area_despawn_or_missing_end() {
    let mut despawned_app = headless_app();
    despawned_app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        500,
    )));
    let despawned_area = spawn_scroll_area(&mut despawned_app);
    let pointer = touch_pointer(43);
    trigger_drag_start(&mut despawned_app, despawned_area, &pointer);
    assert!(
        despawned_app
            .world()
            .resource::<ScrollMotionState>()
            .motion(despawned_area)
            .is_some()
    );
    despawned_app.world_mut().despawn(despawned_area);
    despawned_app.update();
    assert_eq!(
        despawned_app
            .world()
            .resource::<ScrollMotionState>()
            .motion(despawned_area),
        None
    );

    let mut missing_end_app = headless_app();
    missing_end_app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        700,
    )));
    let area = spawn_scroll_area(&mut missing_end_app);
    let pointer = touch_pointer(44);
    trigger_drag_start(&mut missing_end_app, area, &pointer);
    for _ in 0..3 {
        missing_end_app.update();
    }
    assert_eq!(
        missing_end_app
            .world()
            .resource::<ScrollMotionState>()
            .motion(area),
        None
    );
}

fn spawn_scroll_area(app: &mut App) -> Entity {
    let entity = app
        .world_mut()
        .spawn((
            Node {
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
        ))
        .id();
    app.world_mut().entity_mut(entity).insert(ComputedNode {
        size: Vec2::new(300.0, 400.0),
        content_size: Vec2::new(300.0, 1000.0),
        inverse_scale_factor: 1.0,
        ..Default::default()
    });
    app.update();
    entity
}
