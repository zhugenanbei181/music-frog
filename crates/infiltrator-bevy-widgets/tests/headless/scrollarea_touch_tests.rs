//! Headless touch event to Bevy Picking and ScrollArea behavior coverage.

use bevy::MinimalPlugins;
use bevy::app::{App, Update};
use bevy::asset::AssetPlugin;
use bevy::camera::ComputedCameraValues;
use bevy::camera::visibility::InheritedVisibility;
use bevy::camera::{Camera, RenderTargetInfo};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::message::MessageReader;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{ResMut, SystemParam};
use bevy::image::{ImagePlugin, TextureAtlasPlugin};
use bevy::input::InputPlugin;
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::{UVec2, Vec2};
use bevy::picking::backend::PointerHits;
use bevy::picking::events::{
    PointerCancel, PointerClick, PointerDrag, PointerDragEnd, PointerDragStart,
};
use bevy::picking::input::PointerInputPlugin;
use bevy::picking::pointer::{PointerAction, PointerButton, PointerId, PointerInput, PointerMap};
use bevy::picking::{DefaultPickingPlugins, Pickable};
use bevy::text::TextPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::prelude::{Node, Overflow, percent, px};
use bevy::ui::{FlexDirection, UiPlugin, UiTargetCamera};
use bevy::ui::{Pressed, ScrollPosition};
use bevy::ui_widgets::{Activate, Button, ButtonPlugin, ScrollArea};
use bevy::window::{PrimaryWindow, Window, WindowEvent, WindowResolution};
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::scrollarea::{ScrollMotion, ScrollMotionState};
use infiltrator_bevy_widgets::theme::Theme;
use std::time::Duration;
#[derive(Resource, Default)]
struct PointerInputCapture(Vec<PointerInput>);

pub(super) struct TouchScrollTargets {
    pub(super) window: Entity,
    pub(super) fixed_top: Entity,
    pub(super) scroll_area: Entity,
    pub(super) scroll_row: Entity,
    pub(super) fixed_bottom: Entity,
}

#[derive(Resource, Default)]
pub(super) struct PickingTrace {
    pub(super) pointer_inputs: Vec<PointerInput>,
    pub(super) hits: Vec<PointerHits>,
    pub(super) drag_starts: Vec<PointerDragStart>,
    pub(super) drags: Vec<PointerDrag>,
    pub(super) drag_ends: Vec<PointerDragEnd>,
    pub(super) cancels: Vec<PointerCancel>,
    pub(super) clicks: Vec<PointerClick>,
}

#[derive(Resource, Default)]
pub(super) struct ActivationCount(pub(super) usize);

fn count_activations(_: On<Activate>, mut count: ResMut<ActivationCount>) {
    count.0 += 1;
}

fn capture_pointer_inputs(
    mut pointer_inputs: MessageReader<PointerInput>,
    mut capture: ResMut<PointerInputCapture>,
) {
    capture.0.extend(pointer_inputs.read().cloned());
}

#[derive(SystemParam)]
struct PickingTraceMessages<'w, 's> {
    pointer_inputs: MessageReader<'w, 's, PointerInput>,
    hits: MessageReader<'w, 's, PointerHits>,
    drag_starts: MessageReader<'w, 's, PointerDragStart>,
    drags: MessageReader<'w, 's, PointerDrag>,
    drag_ends: MessageReader<'w, 's, PointerDragEnd>,
    cancels: MessageReader<'w, 's, PointerCancel>,
    clicks: MessageReader<'w, 's, PointerClick>,
}

pub(super) fn send_window_touch(
    app: &mut App,
    window: Entity,
    id: u64,
    phase: TouchPhase,
    position: Vec2,
) {
    app.world_mut()
        .write_message(WindowEvent::TouchInput(TouchInput {
            phase,
            position,
            window,
            force: None,
            id,
        }));
}

fn capture_picking_trace(mut messages: PickingTraceMessages, mut trace: ResMut<PickingTrace>) {
    trace
        .pointer_inputs
        .extend(messages.pointer_inputs.read().cloned());
    trace.hits.extend(messages.hits.read().cloned());
    trace
        .drag_starts
        .extend(messages.drag_starts.read().cloned());
    trace.drags.extend(messages.drags.read().cloned());
    trace.drag_ends.extend(messages.drag_ends.read().cloned());
    trace.cancels.extend(messages.cancels.read().cloned());
    trace.clicks.extend(messages.clicks.read().cloned());
}

pub(super) fn touch_scroll_app() -> (App, TouchScrollTargets) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(InputPlugin)
        .add_plugins(TextPlugin)
        .add_plugins(ImagePlugin::default_nearest())
        .add_plugins(TextureAtlasPlugin)
        .add_plugins(UiPlugin)
        .add_plugins(WidgetsPlugin::new(&Theme::dark()))
        .add_plugins(DefaultPickingPlugins)
        .add_plugins(ButtonPlugin)
        .add_message::<WindowEvent>();
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        20,
    )));
    app.init_resource::<ActivationCount>();
    app.init_resource::<PickingTrace>();
    app.add_observer(count_activations);
    app.add_systems(Update, capture_picking_trace);

    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(1200, 1600).with_scale_factor_override(2.0),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    let camera = app
        .world_mut()
        .spawn(Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(1200, 1600),
                    scale_factor: 2.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        })
        .id();
    let shell = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            UiTargetCamera(camera),
            InheritedVisibility::VISIBLE,
        ))
        .id();
    let fixed_top = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: rem(100.0),
                min_height: rem(100.0),
                flex_shrink: 0.0,
                ..Default::default()
            },
            Button,
            Pickable::default(),
            InheritedVisibility::VISIBLE,
            ChildOf(shell),
        ))
        .id();
    let content_slot = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                flex_basis: px(0.0),
                min_height: px(0.0),
                overflow: Overflow::clip(),
                ..Default::default()
            },
            InheritedVisibility::VISIBLE,
            ChildOf(shell),
        ))
        .id();
    let scroll_area = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_shrink: 1.0,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..Default::default()
            },
            ScrollArea,
            ScrollPosition::default(),
            InheritedVisibility::VISIBLE,
            ChildOf(content_slot),
        ))
        .id();
    let scroll_row = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: rem(1000.0),
                min_height: rem(1000.0),
                flex_shrink: 0.0,
                ..Default::default()
            },
            Button,
            Pickable::default(),
            InheritedVisibility::VISIBLE,
            ChildOf(scroll_area),
        ))
        .id();
    let fixed_bottom = app
        .world_mut()
        .spawn((
            Node {
                width: percent(100),
                height: rem(100.0),
                min_height: rem(100.0),
                flex_shrink: 0.0,
                ..Default::default()
            },
            Button,
            Pickable::default(),
            InheritedVisibility::VISIBLE,
            ChildOf(shell),
        ))
        .id();

    app.update();
    (
        app,
        TouchScrollTargets {
            window,
            fixed_top,
            scroll_area,
            scroll_row,
            fixed_bottom,
        },
    )
}

#[test]
fn window_touch_phases_become_typed_picking_pointer_actions() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<WindowEvent>()
        .add_message::<TouchInput>()
        .add_message::<PointerInput>()
        .init_resource::<PointerMap>()
        .add_plugins(PointerInputPlugin)
        .init_resource::<PointerInputCapture>()
        .add_systems(Update, capture_pointer_inputs);
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.update();

    app.world_mut()
        .write_message(WindowEvent::TouchInput(TouchInput {
            phase: TouchPhase::Started,
            position: Vec2::new(42.0, 64.0),
            window,
            force: None,
            id: 7,
        }));
    app.update();

    {
        let capture = app.world().resource::<PointerInputCapture>();
        assert_eq!(capture.0.len(), 1);
        assert_eq!(capture.0[0].pointer_id, PointerId::Touch(7));
        assert!(matches!(
            capture.0[0].action,
            PointerAction::Press(PointerButton::Primary)
        ));
        assert_eq!(capture.0[0].location.position, Vec2::new(42.0, 64.0));
    }

    app.world_mut()
        .resource_mut::<PointerInputCapture>()
        .0
        .clear();
    app.world_mut()
        .write_message(WindowEvent::TouchInput(TouchInput {
            phase: TouchPhase::Moved,
            position: Vec2::new(42.0, 164.0),
            window,
            force: None,
            id: 7,
        }));
    app.update();

    let capture = app.world().resource::<PointerInputCapture>();
    assert_eq!(capture.0.len(), 1);
    assert!(matches!(
        capture.0[0].action,
        PointerAction::Move { delta } if delta == Vec2::new(0.0, 100.0)
    ));

    app.world_mut()
        .resource_mut::<PointerInputCapture>()
        .0
        .clear();
    send_window_touch(
        &mut app,
        window,
        7,
        TouchPhase::Ended,
        Vec2::new(42.0, 164.0),
    );
    app.update();
    let capture = app.world().resource::<PointerInputCapture>();
    assert_eq!(capture.0.len(), 1);
    assert!(matches!(
        capture.0[0].action,
        PointerAction::Release(PointerButton::Primary)
    ));

    app.world_mut()
        .resource_mut::<PointerInputCapture>()
        .0
        .clear();
    send_window_touch(
        &mut app,
        window,
        8,
        TouchPhase::Started,
        Vec2::new(12.0, 14.0),
    );
    app.update();
    app.world_mut()
        .resource_mut::<PointerInputCapture>()
        .0
        .clear();
    send_window_touch(
        &mut app,
        window,
        8,
        TouchPhase::Canceled,
        Vec2::new(12.0, 14.0),
    );
    app.update();
    let capture = app.world().resource::<PointerInputCapture>();
    assert_eq!(capture.0.len(), 1);
    assert_eq!(capture.0[0].pointer_id, PointerId::Touch(8));
    assert!(matches!(capture.0[0].action, PointerAction::Cancel));
}

#[test]
fn window_touch_drag_scrolls_logical_distance_without_activating_the_pressed_row() {
    let (mut app, targets) = touch_scroll_app();
    let window = targets.window;
    let root = targets.scroll_area;
    let child = targets.scroll_row;

    // The UI backend calculates the row hit from its camera, computed layout,
    // clip chain, and the logical pointer location; no PointerHits are injected.
    send_window_touch(
        &mut app,
        window,
        9,
        TouchPhase::Started,
        Vec2::new(120.0, 500.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        9,
        TouchPhase::Moved,
        Vec2::new(120.0, 400.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        9,
        TouchPhase::Moved,
        Vec2::new(120.0, 395.0),
    );
    app.update();
    assert_eq!(app.world().get::<ScrollPosition>(root).unwrap().y, 105.0);
    send_window_touch(
        &mut app,
        window,
        9,
        TouchPhase::Ended,
        Vec2::new(120.0, 395.0),
    );
    app.update();

    let trace = app.world().resource::<PickingTrace>();
    assert_eq!(trace.pointer_inputs.len(), 4);
    assert!(trace.hits.iter().any(|hits| {
        hits.pointer == PointerId::Touch(9) && hits.picks.iter().any(|(entity, _)| *entity == child)
    }));
    assert_eq!(trace.drag_starts.len(), 1);
    assert_eq!(trace.drags.len(), 2);
    assert_eq!(trace.drag_ends.len(), 1);
    assert!(trace.clicks.is_empty());
    assert_eq!(app.world().resource::<ActivationCount>().0, 0);
    let after_release = app.world().get::<ScrollPosition>(root).unwrap().y;
    assert!(after_release > 105.0);
    assert!(matches!(
        app.world().resource::<ScrollMotionState>().motion(root),
        Some(ScrollMotion::Coasting { .. })
    ));
    app.update();
    assert!(app.world().get::<ScrollPosition>(root).unwrap().y > after_release);
    for _ in 0..100 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<ScrollMotionState>().motion(root),
        None
    );

    send_window_touch(
        &mut app,
        window,
        10,
        TouchPhase::Started,
        Vec2::new(120.0, 500.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        10,
        TouchPhase::Ended,
        Vec2::new(120.0, 500.0),
    );
    app.update();
    assert_eq!(app.world().resource::<ActivationCount>().0, 1);
    assert_eq!(app.world().resource::<PickingTrace>().clicks.len(), 1);

    send_window_touch(
        &mut app,
        window,
        11,
        TouchPhase::Started,
        Vec2::new(120.0, 500.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        11,
        TouchPhase::Moved,
        Vec2::new(120.0, 460.0),
    );
    app.update();
    let scroll_at_cancel = app.world().get::<ScrollPosition>(root).unwrap().y;
    assert!(app.world().get::<Button>(child).is_some());
    assert!(matches!(
        app.world().resource::<ScrollMotionState>().motion(root),
        Some(ScrollMotion::Direct {
            pointer: PointerId::Touch(11),
            ..
        })
    ));
    send_window_touch(
        &mut app,
        window,
        11,
        TouchPhase::Canceled,
        Vec2::new(120.0, 460.0),
    );
    app.update();

    let trace = app.world().resource::<PickingTrace>();
    let raw_cancel_count = trace
        .pointer_inputs
        .iter()
        .filter(|input| {
            input.pointer_id == PointerId::Touch(11)
                && matches!(input.action, PointerAction::Cancel)
        })
        .count();
    assert_eq!(raw_cancel_count, 1);
    assert_eq!(trace.clicks.len(), 1);
    assert_eq!(app.world().resource::<ActivationCount>().0, 1);
    assert_eq!(
        app.world().get::<ScrollPosition>(root).unwrap().y,
        scroll_at_cancel
    );
    assert_eq!(
        app.world().resource::<ScrollMotionState>().motion(root),
        None
    );
    assert!(
        app.world().get::<Pressed>(child).is_none(),
        "raw touch cancellation releases the pressed list row"
    );
}

#[test]
fn cancelled_touch_before_drag_releases_the_pressed_control() {
    let (mut app, targets) = touch_scroll_app();
    send_window_touch(
        &mut app,
        targets.window,
        30,
        TouchPhase::Started,
        Vec2::new(120.0, 500.0),
    );
    app.update();
    assert!(app.world().get::<Pressed>(targets.scroll_row).is_some());

    send_window_touch(
        &mut app,
        targets.window,
        30,
        TouchPhase::Canceled,
        Vec2::new(120.0, 500.0),
    );
    app.update();

    assert!(app.world().get::<Pressed>(targets.scroll_row).is_none());
    assert_eq!(app.world().resource::<ActivationCount>().0, 0);
    assert_eq!(
        app.world()
            .resource::<ScrollMotionState>()
            .motion(targets.scroll_area),
        None
    );
}

#[path = "scrollarea_touch_tests/shell_and_swipes.rs"]
mod shell_and_swipes;
