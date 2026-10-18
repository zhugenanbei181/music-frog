//! Headless touch tests for shell fixed bars and alternating swipes.

use super::*;
use bevy::input::touch::TouchPhase;
use bevy::math::Vec2;
use bevy::picking::pointer::PointerId;
use bevy::ui::ScrollPosition;
use infiltrator_bevy_widgets::scrollarea::ScrollMotionPreference;

#[test]
fn fixed_shell_bars_stay_outside_the_independent_touch_scroll_area() {
    let (mut app, targets) = touch_scroll_app();
    let window = targets.window;
    let scroll_area = targets.scroll_area;

    send_window_touch(
        &mut app,
        window,
        20,
        TouchPhase::Started,
        Vec2::new(200.0, 50.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        20,
        TouchPhase::Ended,
        Vec2::new(200.0, 50.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        21,
        TouchPhase::Started,
        Vec2::new(200.0, 750.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        21,
        TouchPhase::Ended,
        Vec2::new(200.0, 750.0),
    );
    app.update();
    assert_eq!(app.world().resource::<ActivationCount>().0, 2);
    assert!(
        app.world()
            .resource::<PickingTrace>()
            .hits
            .iter()
            .any(|hits| {
                hits.pointer == PointerId::Touch(20)
                    && hits
                        .picks
                        .iter()
                        .any(|(entity, _)| *entity == targets.fixed_top)
            })
    );
    assert!(
        app.world()
            .resource::<PickingTrace>()
            .hits
            .iter()
            .any(|hits| {
                hits.pointer == PointerId::Touch(21)
                    && hits
                        .picks
                        .iter()
                        .any(|(entity, _)| *entity == targets.fixed_bottom)
            })
    );

    send_window_touch(
        &mut app,
        window,
        22,
        TouchPhase::Started,
        Vec2::new(200.0, 50.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        22,
        TouchPhase::Moved,
        Vec2::new(200.0, 70.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        22,
        TouchPhase::Ended,
        Vec2::new(200.0, 70.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        23,
        TouchPhase::Started,
        Vec2::new(200.0, 750.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        23,
        TouchPhase::Moved,
        Vec2::new(200.0, 730.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        23,
        TouchPhase::Ended,
        Vec2::new(200.0, 730.0),
    );
    app.update();
    assert_eq!(
        app.world().get::<ScrollPosition>(scroll_area).unwrap().y,
        0.0
    );
    assert_eq!(app.world().resource::<ActivationCount>().0, 2);

    send_window_touch(
        &mut app,
        window,
        24,
        TouchPhase::Started,
        Vec2::new(200.0, 500.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        24,
        TouchPhase::Moved,
        Vec2::new(200.0, 400.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        24,
        TouchPhase::Moved,
        Vec2::new(200.0, 395.0),
    );
    app.update();
    assert_eq!(
        app.world().get::<ScrollPosition>(scroll_area).unwrap().y,
        105.0
    );
    send_window_touch(
        &mut app,
        window,
        24,
        TouchPhase::Ended,
        Vec2::new(200.0, 395.0),
    );
    app.update();
    for _ in 0..100 {
        app.update();
    }
    let scroll_after_fling = app.world().get::<ScrollPosition>(scroll_area).unwrap().y;
    assert!(scroll_after_fling > 100.0);
    assert!(scroll_after_fling <= 400.0);
    assert_eq!(
        app.world()
            .get::<ScrollPosition>(targets.fixed_top)
            .unwrap()
            .0,
        Vec2::ZERO
    );
    assert_eq!(
        app.world()
            .get::<ScrollPosition>(targets.fixed_bottom)
            .unwrap()
            .0,
        Vec2::ZERO
    );

    send_window_touch(
        &mut app,
        window,
        25,
        TouchPhase::Started,
        Vec2::new(200.0, 50.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        25,
        TouchPhase::Ended,
        Vec2::new(200.0, 50.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        26,
        TouchPhase::Started,
        Vec2::new(200.0, 750.0),
    );
    app.update();
    send_window_touch(
        &mut app,
        window,
        26,
        TouchPhase::Ended,
        Vec2::new(200.0, 750.0),
    );
    app.update();
    assert_eq!(app.world().resource::<ActivationCount>().0, 4);
    assert_eq!(
        app.world().get::<ScrollPosition>(scroll_area).unwrap().y,
        scroll_after_fling
    );
}

#[test]
fn ten_alternating_touch_swipes_reach_both_bounds_without_activating_the_row() {
    let (mut app, targets) = touch_scroll_app();
    *app.world_mut().resource_mut::<ScrollMotionPreference>() = ScrollMotionPreference::Reduced;

    for swipe in 0..10 {
        let (start_y, end_y, expected_position) = if swipe % 2 == 0 {
            (600.0, 120.0, 400.0)
        } else {
            (120.0, 600.0, 0.0)
        };
        let pointer_id = 40 + swipe;
        send_window_touch(
            &mut app,
            targets.window,
            pointer_id,
            TouchPhase::Started,
            Vec2::new(200.0, start_y),
        );
        app.update();
        send_window_touch(
            &mut app,
            targets.window,
            pointer_id,
            TouchPhase::Moved,
            Vec2::new(200.0, end_y),
        );
        app.update();

        assert_eq!(
            app.world()
                .get::<ScrollPosition>(targets.scroll_area)
                .unwrap()
                .y,
            expected_position,
            "swipe {swipe} reaches its clamped logical endpoint"
        );
        assert_eq!(app.world().resource::<ActivationCount>().0, 0);

        send_window_touch(
            &mut app,
            targets.window,
            pointer_id,
            TouchPhase::Ended,
            Vec2::new(200.0, end_y),
        );
        app.update();
    }

    assert!(
        app.world()
            .resource::<PickingTrace>()
            .hits
            .iter()
            .any(|hits| {
                hits.pointer == PointerId::Touch(49)
                    && hits
                        .picks
                        .iter()
                        .any(|(entity, _)| *entity == targets.scroll_row)
            })
    );
    assert_eq!(
        app.world()
            .get::<ScrollPosition>(targets.scroll_area)
            .unwrap()
            .y,
        0.0
    );
    assert_eq!(app.world().resource::<ActivationCount>().0, 0);
}
