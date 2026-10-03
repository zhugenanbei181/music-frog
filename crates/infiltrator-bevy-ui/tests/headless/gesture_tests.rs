//! DUAL-15-07 headless tests: synthetic `bevy::input::touch::TouchInput`
//! messages drive the widget-layer recognizer and publish the shared semantic
//! snapshot. No device is involved — the same real recognition path a mobile
//! host would feed is exercised deterministically.

use std::sync::Arc;
use std::time::Duration;

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::input::touch::{TouchInput, TouchPhase};
use bevy::math::Vec2;
use bevy::scene::{CommandsSceneExt, ScenePlugin};
use bevy::time::Time;
use bevy::ui::Node;
use bevy::ui::prelude::{Display, Val};
use bevy::ui::widget::Text;

use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandSinkHandle, DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::gesture::{
    GestureHostReport, ShellGesturePlugin, ShellGestureSnapshot, shared_outcome, shared_phase,
    touch_support,
};
use infiltrator_bevy_ui::pages::connections::ConnectionItem;
use infiltrator_bevy_ui::pages::connections_row::connection_row_scene;
use infiltrator_bevy_ui::pages::proxies::ProxyNode;
use infiltrator_bevy_ui::pages::proxies_card::proxy_node_scene;
use infiltrator_bevy_ui::route::{ActiveRoute, Route};
use infiltrator_bevy_widgets::gesture::{
    GestureOutcome, PullToRefreshIndicator, PullToRefreshState, PullToRefreshText,
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, pull_to_refresh_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_contract::shell_gesture::{
    GesturePoint, GestureSemanticEvent, GestureTouchPhase, PullPhase, SafeAreaInsets,
    SwipeDirection,
};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};

fn gesture_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(ShellGesturePlugin);
    // Deterministic frame time: `TimePlugin` recomputes the generic `Time`
    // from the real clock every frame, so a bare `Time::advance_by` in a test
    // is overwritten and spring convergence would depend on wall-clock timing.
    // Pinning the update strategy makes each `app.update()` advance exactly
    // 20 ms, matching what the spring tests intend to step.
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        Duration::from_millis(20),
    ));
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

#[test]
fn a_pull_to_refresh_under_threshold_smoothly_rebounds_with_spring() {
    let mut app = gesture_app();
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    let scene = pull_to_refresh_scene(
        &PullToRefreshState::default(),
        &UiPalette::new(&Theme::dark()),
    );
    app.world_mut().commands().spawn_scene(scene);
    app.update();

    // Pull downward by 40px (threshold is 80px)
    send(&mut app, 1, TouchPhase::Started, 10.0, 10.0);
    send(&mut app, 1, TouchPhase::Moved, 10.0, 50.0);
    app.update();

    assert_eq!(snapshot(&app).0.pull.phase, PullPhase::Pulling);

    // Verify indicator height expands and label shows "下拉刷新"
    let mut node_q = app.world_mut().query::<(&PullToRefreshIndicator, &Node)>();
    let (_, node) = node_q.iter(app.world()).next().expect("indicator exists");
    if let Val::Px(height) = node.height {
        assert!(height > 0.0, "indicator expanded on pull: {height}");
    } else {
        panic!("unexpected height value");
    }

    let mut text_q = app.world_mut().query::<(&PullToRefreshText, &Text)>();
    let (_, text) = text_q.iter(app.world()).next().expect("text exists");
    assert_eq!(text.0, "下拉刷新");

    // Release under threshold
    send(&mut app, 1, TouchPhase::Ended, 10.0, 50.0);
    app.update();

    // Multiple updates should simulate spring damping rebound towards 0.0
    for _ in 0..10 {
        app.update();
    }

    let (_, node_after) = node_q.iter(app.world()).next().expect("indicator exists");
    if let Val::Px(h) = node_after.height {
        assert!(
            h <= 1.0,
            "indicator height smoothly rebounded to near 0: {h}"
        );
    }
}

#[test]
fn a_pull_to_refresh_over_threshold_arms_and_triggers_route_refresh_command() {
    let routes_and_commands = [
        (Route::Proxies, UiCommand::TestAllProxyGroups),
        (Route::Rules, UiCommand::RefreshRuleProviders),
        (Route::Profiles, UiCommand::UpdateAllSubscriptions),
    ];

    for (route, expected_cmd) in routes_and_commands {
        let mut app = gesture_app();
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        let sink = Arc::new(DemoCommandSink::accepting());
        app.insert_resource(CommandSinkHandle(sink.clone()));
        app.insert_resource(ActiveRoute(Some(route)));

        let scene = pull_to_refresh_scene(
            &PullToRefreshState::default(),
            &UiPalette::new(&Theme::dark()),
        );
        app.world_mut().commands().spawn_scene(scene);
        app.update();

        // Downward drag by 90px (>= 80px threshold)
        send(&mut app, 1, TouchPhase::Started, 10.0, 10.0);
        send(&mut app, 1, TouchPhase::Moved, 10.0, 100.0);
        app.update();

        assert_eq!(snapshot(&app).0.pull.phase, PullPhase::Armed);

        let mut text_q = app.world_mut().query::<(&PullToRefreshText, &Text)>();
        let (_, text) = text_q.iter(app.world()).next().expect("text exists");
        assert_eq!(text.0, "释放以刷新");

        // Release to trigger refresh
        send(&mut app, 1, TouchPhase::Ended, 10.0, 100.0);
        app.update();

        assert_eq!(snapshot(&app).0.pull.phase, PullPhase::Refreshing);

        // Check indicator label changed to "正在更新..."
        let (_, text_refreshing) = text_q.iter(app.world()).next().expect("text exists");
        assert_eq!(text_refreshing.0, "正在更新...");

        // Assert command sink received the route-specific command
        let submitted = sink.submitted();
        assert!(
            submitted.contains(&expected_cmd),
            "expected route {route:?} to dispatch {expected_cmd:?}, but got {submitted:?}"
        );
    }
}

#[test]
fn test_proxy_card_swipe_to_action_spring_slides_content_and_reveals_drawer() {
    let mut app = gesture_app();
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    let palette = UiPalette::new(&Theme::dark());

    let node = ProxyNode {
        name: "HK-01".to_owned(),
        node_type: "Shadowsocks".to_owned(),
        delay_ms: Some(45),
        selected: true,
        favorite: false,
        features: vec!["udp".to_owned()],
    };
    let scene = proxy_node_scene(0, 0, "DefaultGroup", &node, &palette);
    app.world_mut().commands().spawn_scene(scene);
    app.update();

    let mut item_q = app.world_mut().query::<&SwipeToActionItem>();
    let item = item_q.iter(app.world()).next().expect("swipe item exists");
    assert_eq!(item.offset_x, 0.0);
    assert!(!item.is_open());

    let mut drawer_q = app.world_mut().query::<(&SwipeActionDrawer, &Node)>();
    let (_, drawer_node) = drawer_q.iter(app.world()).next().expect("drawer exists");
    assert_eq!(drawer_node.display, Display::None);

    let mut content_q = app.world_mut().query::<(&SwipeContentContainer, &Node)>();
    let (_, content_node) = content_q.iter(app.world()).next().expect("content exists");
    assert_eq!(content_node.left, Val::Px(0.0));

    let item_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SwipeToActionItem>>()
        .single(app.world())
        .expect("swipe item entity");
    app.world_mut()
        .get_mut::<SwipeToActionItem>(item_entity)
        .expect("swipe item")
        .open_leading();

    for _ in 0..15 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        app.update();
    }

    let item_after = app.world().get::<SwipeToActionItem>(item_entity).unwrap();
    assert!(item_after.is_open());
    assert_eq!(item_after.offset_x, -88.0);

    let (_, content_after) = content_q.iter(app.world()).next().expect("content exists");
    if let Val::Px(x) = content_after.left {
        assert!(x < -70.0, "content slid left to reveal drawer, x = {x}");
    } else {
        panic!("expected Val::Px for left");
    }

    let (_, drawer_after) = drawer_q.iter(app.world()).next().expect("drawer exists");
    assert_eq!(drawer_after.display, Display::Flex);

    app.world_mut()
        .get_mut::<SwipeToActionItem>(item_entity)
        .expect("swipe item")
        .close();

    for _ in 0..25 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        app.update();
    }

    let (_, content_closed) = content_q.iter(app.world()).next().expect("content exists");
    if let Val::Px(x) = content_closed.left {
        assert!(x.abs() < 1.0, "content rebounded to origin, x = {x}");
    }

    let (_, drawer_closed) = drawer_q.iter(app.world()).next().expect("drawer exists");
    assert_eq!(drawer_closed.display, Display::None);
}

#[test]
fn test_connection_row_swipe_to_action_spring_slides_content_and_reveals_drawer() {
    let mut app = gesture_app();
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    let palette = UiPalette::new(&Theme::dark());

    let conn = ConnectionItem {
        id: "conn-999".to_owned(),
        host: "api.github.com:443".to_owned(),
        process: "curl".to_owned(),
        rule: "Proxy".to_owned(),
        rule_payload: "DOMAIN-SUFFIX".to_owned(),
        chain: "Proxy -> HK-01".to_owned(),
        chains: vec!["Proxy".to_owned(), "HK-01".to_owned()],
        network: "tcp".to_owned(),
        source_ip: "127.0.0.1".to_owned(),
        source_port: "54321".to_owned(),
        destination_ip: "140.82.121.4".to_owned(),
        destination_port: "443".to_owned(),
        destination_geo_ip: None,
        destination_ip_asn: String::new(),
        upload_bps: 1024.0,
        download_bps: 4096.0,
        upload_total: 10000,
        download_total: 50000,
    };

    let scene = connection_row_scene(0, &conn, &palette);
    app.world_mut().commands().spawn_scene(scene);
    app.update();

    let item_entity = app
        .world_mut()
        .query_filtered::<Entity, bevy::ecs::query::With<SwipeToActionItem>>()
        .single(app.world())
        .expect("connection row swipe item");

    let mut drawer_q = app.world_mut().query::<(&SwipeActionDrawer, &Node)>();
    let (_, drawer_node) = drawer_q.iter(app.world()).next().expect("drawer exists");
    assert_eq!(drawer_node.display, Display::None);

    let mut content_q = app.world_mut().query::<(&SwipeContentContainer, &Node)>();
    let (_, content_node) = content_q.iter(app.world()).next().expect("content exists");
    assert_eq!(content_node.left, Val::Px(0.0));

    app.world_mut()
        .get_mut::<SwipeToActionItem>(item_entity)
        .expect("swipe item")
        .open_leading();

    for _ in 0..15 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        app.update();
    }

    let (_, content_after) = content_q.iter(app.world()).next().expect("content exists");
    if let Val::Px(x) = content_after.left {
        assert!(x < -70.0, "content slid left to reveal drawer, x = {x}");
    }

    let (_, drawer_after) = drawer_q.iter(app.world()).next().expect("drawer exists");
    assert_eq!(drawer_after.display, Display::Flex);

    app.world_mut()
        .get_mut::<SwipeToActionItem>(item_entity)
        .expect("swipe item")
        .close();

    for _ in 0..25 {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(20));
        app.update();
    }

    let (_, drawer_closed) = drawer_q.iter(app.world()).next().expect("drawer exists");
    assert_eq!(drawer_closed.display, Display::None);
}
