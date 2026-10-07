//! test-intent: behavior
//! Explicit layout facts exercise the actual SDK scroll input and production follow system.
use super::{navigate_to, setup_matrix_a_app};
use crate::native_input::click_entity;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::input::mouse::{MouseScrollPixelsPerLine, MouseScrollUnit};
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerScroll};
use bevy::picking::pointer::{Location, PointerId};
use bevy::ui::{ComputedNode, ScrollPosition};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::command::DemoCommandSink;
use infiltrator_bevy_ui::pages::logs::{LogsPageRoot, PauseLogsButton};
use infiltrator_bevy_ui::pages::logs_search::LogsViewState;
use infiltrator_bevy_ui::route::Route;
use std::sync::Arc;

#[test]
fn native_history_scroll_lock_growth_and_resume_move_only_the_real_scroll_surface() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin)
        .init_resource::<MouseScrollPixelsPerLine>();
    navigate_to(&mut app, Route::Logs);
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<LogsPageRoot>>()
        .single(app.world())
        .unwrap();
    {
        let mut measured = app.world_mut().get_mut::<ComputedNode>(root).unwrap();
        measured.size = Vec2::new(500.0, 200.0);
        measured.content_size = Vec2::new(500.0, 800.0);
        measured.inverse_scale_factor = 1.0;
    }
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 600.0);
    app.world_mut().trigger(PointerScroll {
        entity: root,
        pointer: Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: 500,
                    height: 200,
                },
                position: Vec2::new(10.0, 10.0),
            },
        ),
        unit: MouseScrollUnit::Pixel,
        x: 0.0,
        y: 100.0,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        phase: TouchPhase::Moved,
    });
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 500.0);
    assert!(
        !app.world()
            .resource::<LogsViewState>()
            .0
            .follow
            .should_follow()
    );
    app.world_mut()
        .get_mut::<ComputedNode>(root)
        .unwrap()
        .content_size
        .y = 900.0;
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 500.0);
    let toggle = app
        .world_mut()
        .query_filtered::<Entity, With<PauseLogsButton>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, toggle);
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 700.0);
    assert!(
        app.world()
            .resource::<LogsViewState>()
            .0
            .follow
            .should_follow()
    );
    click_entity(&mut app, toggle);
    app.world_mut()
        .get_mut::<ComputedNode>(root)
        .unwrap()
        .content_size
        .y = 1000.0;
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 700.0);
    assert!(
        !app.world()
            .resource::<LogsViewState>()
            .0
            .follow
            .should_follow()
    );
    assert_eq!(app.world().resource::<LogsViewState>().0.source_count(), 5);
    assert!(sink.submitted().is_empty());
}
