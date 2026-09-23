//! DUAL-15-07 headless tests: synthetic `bevy::input::touch::TouchInput`
//! messages drive the widget-layer recognizer and publish the shared semantic
//! snapshot. No device is involved — the same real recognition path a mobile
//! host would feed is exercised deterministically.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::scene::ScenePlugin;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::gesture::{
    GestureHostReport, ShellGesturePlugin, ShellGestureSnapshot, shared_outcome, shared_phase,
    touch_support,
};
use infiltrator_bevy_widgets::gesture::GestureOutcome;
use infiltrator_contract::shell_gesture::{
    GesturePoint, GestureSemanticEvent, GestureTouchPhase, PullPhase, SafeAreaInsets,
    SwipeDirection,
};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};

fn gesture_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(ShellGesturePlugin);
    app.update();
    app
}

fn send(app: &mut App, id: u64, phase: TouchPhase, x: f32, y: f32) {
    app.world_mut().write_message(TouchInput {
        phase,
        position: Vec2::new(x, y),
        window: Entity::PLACEHOLDER,
        force: None,
        id,
    });
}

fn snapshot(app: &App) -> ShellGestureSnapshot {
    app.world().resource::<ShellGestureSnapshot>().clone()
}

#[test]
fn a_tap_and_a_double_tap_map_through_the_widget_recognizer() {
    let mut app = gesture_app();
    assert!(
        snapshot(&app).0.has_touch_host(),
        "the Bevy shell hosts the real recognizer"
    );
    assert!(
        touch_support().multi_touch(),
        "the shell reports multi-touch"
    );

    send(&mut app, 1, TouchPhase::Started, 10.0, 10.0);
    send(&mut app, 1, TouchPhase::Ended, 10.0, 10.0);
    app.update();
    assert_eq!(
        snapshot(&app).0.last(),
        Some(GestureSemanticEvent::Tap {
            position: GesturePoint::new(10.0, 10.0),
        })
    );

    // A second quick tap inside the double-tap window.
    send(&mut app, 2, TouchPhase::Started, 12.0, 10.0);
    send(&mut app, 2, TouchPhase::Ended, 12.0, 10.0);
    app.update();
    assert_eq!(
        snapshot(&app).0.last(),
        Some(GestureSemanticEvent::DoubleTap {
            position: GesturePoint::new(12.0, 10.0),
        })
    );
}

#[test]
fn a_horizontal_drag_maps_to_a_swipe_to_action_event() {
    let mut app = gesture_app();

    send(&mut app, 1, TouchPhase::Started, 0.0, 400.0);
    send(&mut app, 1, TouchPhase::Moved, 60.0, 400.0);
    app.update();
    let swipe = snapshot(&app).0.swipe;
    assert_eq!(swipe.direction, SwipeDirection::Trailing);
    assert_eq!(swipe.extent_permille, 750);

    // Releasing past half the action width settles onto the action.
    send(&mut app, 1, TouchPhase::Ended, 60.0, 400.0);
    app.update();
    assert_eq!(snapshot(&app).0.swipe.extent_permille, 1000);
    assert_eq!(snapshot(&app).0.swipe.direction, SwipeDirection::Trailing);
}

#[test]
fn a_downward_drag_from_the_top_band_maps_to_pull_to_refresh() {
    let mut app = gesture_app();

    send(&mut app, 1, TouchPhase::Started, 10.0, 10.0);
    send(&mut app, 1, TouchPhase::Moved, 10.0, 40.0);
    app.update();
    let pull = snapshot(&app).0.pull;
    assert_eq!(pull.phase, PullPhase::Pulling);
    assert_eq!(pull.progress_permille, 375);

    send(&mut app, 1, TouchPhase::Moved, 10.0, 90.0);
    app.update();
    let pull = snapshot(&app).0.pull;
    assert_eq!(pull.phase, PullPhase::Armed);
    assert_eq!(pull.progress_permille, 1000);

    send(&mut app, 1, TouchPhase::Ended, 10.0, 90.0);
    app.update();
    let pull = snapshot(&app).0.pull;
    assert_eq!(pull.phase, PullPhase::Refreshing);
    assert_eq!(pull.progress_permille, 1000);
}

#[test]
fn a_two_finger_sequence_publishes_a_pinch_event() {
    let mut app = gesture_app();

    send(&mut app, 1, TouchPhase::Started, 100.0, 100.0);
    send(&mut app, 2, TouchPhase::Started, 200.0, 100.0);
    app.update();

    // The first move only records the baseline separation.
    send(&mut app, 1, TouchPhase::Moved, 50.0, 100.0);
    app.update();
    assert!(
        !matches!(
            snapshot(&app).0.last(),
            Some(GestureSemanticEvent::Pinch { .. })
        ),
        "the first sample establishes the baseline"
    );

    send(&mut app, 2, TouchPhase::Moved, 250.0, 100.0);
    app.update();
    let pinch = snapshot(&app).0.pinch;
    assert_eq!(pinch.scale_permille, 1333);
    assert_eq!(pinch.zoom_permille, 1333);
    assert_eq!(pinch.focal, GesturePoint::new(150.0, 100.0));
}

#[test]
fn a_cancelled_touch_clears_the_active_set_without_a_gesture() {
    let mut app = gesture_app();
    send(&mut app, 1, TouchPhase::Started, 5.0, 5.0);
    app.update();
    assert_eq!(
        app.world().resource::<GestureHostReport>().tracked_touches,
        1
    );

    send(&mut app, 1, TouchPhase::Canceled, 5.0, 5.0);
    app.update();
    let state = snapshot(&app);
    assert_eq!(
        state.0.last(),
        Some(GestureSemanticEvent::Touch {
            phase: GestureTouchPhase::Canceled,
            position: GesturePoint::new(5.0, 5.0),
        })
    );
    assert_eq!(
        app.world().resource::<GestureHostReport>().tracked_touches,
        0
    );
}

#[test]
fn host_declared_insets_flow_into_the_shared_snapshot() {
    let mut app = gesture_app();
    assert!(snapshot(&app).0.insets.is_zero());
    assert_eq!(
        app.world().resource::<GestureHostReport>().insets,
        SafeAreaInsets::ZERO
    );

    app.world_mut().resource_mut::<GestureHostReport>().insets =
        SafeAreaInsets::new(44.0, 0.0, 34.0, 0.0);
    app.update();

    let state = snapshot(&app);
    assert_eq!(state.0.insets, SafeAreaInsets::new(44.0, 0.0, 34.0, 0.0));
    assert_eq!(
        state.0.last(),
        Some(GestureSemanticEvent::SafeAreaInsets(SafeAreaInsets::new(
            44.0, 0.0, 34.0, 0.0
        )))
    );
}

#[test]
fn raw_bevy_phases_and_widget_outcomes_map_onto_the_shared_vocabulary() {
    assert_eq!(
        shared_phase(TouchPhase::Started),
        GestureTouchPhase::Started
    );
    assert_eq!(shared_phase(TouchPhase::Moved), GestureTouchPhase::Moved);
    assert_eq!(shared_phase(TouchPhase::Ended), GestureTouchPhase::Ended);
    assert_eq!(
        shared_phase(TouchPhase::Canceled),
        GestureTouchPhase::Canceled
    );

    assert_eq!(
        shared_outcome(GestureOutcome::Tap(Vec2::new(1.0, 2.0))),
        GestureSemanticEvent::Tap {
            position: GesturePoint::new(1.0, 2.0),
        }
    );
    assert_eq!(
        shared_outcome(GestureOutcome::LongPress(Vec2::new(3.0, 4.0))),
        GestureSemanticEvent::LongPress {
            position: GesturePoint::new(3.0, 4.0),
        }
    );
    assert_eq!(
        shared_outcome(GestureOutcome::Pan {
            delta: Vec2::new(5.0, 0.0),
            current: Vec2::new(5.0, 0.0),
        }),
        GestureSemanticEvent::pan(GesturePoint::new(5.0, 0.0), GesturePoint::new(5.0, 0.0))
    );
}

#[test]
fn the_mounted_shell_consumes_touch_through_the_shared_recognizer() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(ShellPlugin::new(ThemePreference::Fixed(ThemeSkin::Dark)));
    app.update();

    assert!(
        app.world().contains_resource::<ShellGestureSnapshot>(),
        "ShellPlugin mounts the real touch consumer"
    );
    assert!(
        app.world().contains_resource::<GestureHostReport>(),
        "ShellPlugin mounts the host report"
    );

    send(&mut app, 1, TouchPhase::Started, 20.0, 20.0);
    send(&mut app, 1, TouchPhase::Ended, 20.0, 20.0);
    app.update();
    assert_eq!(
        snapshot(&app).0.last(),
        Some(GestureSemanticEvent::Tap {
            position: GesturePoint::new(20.0, 20.0),
        }),
        "the mounted shell publishes the shared semantic event"
    );
}
