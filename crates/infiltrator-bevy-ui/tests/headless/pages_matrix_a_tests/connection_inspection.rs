//! test-intent: behavior
use super::*;
use bevy::ecs::message::Messages;
use bevy::input::{ButtonInput, keyboard::KeyCode};
use infiltrator_bevy_ui::pages::connections_clipboard::CopyConnectionHostButton;
use infiltrator_bevy_ui::toast::ShellToast;
use infiltrator_bevy_widgets::drawer::DrawerCloseButton;
use infiltrator_bevy_widgets::toast::ToastKind;
use infiltrator_contract::capability::Availability;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::connection::timing_availability;

fn open(app: &mut App) {
    let entity = app
        .world_mut()
        .query::<(Entity, &ConnInspectButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 0)
        .unwrap()
        .0;
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
    app.update();
}

#[test]
fn connection_drawer_close_escape_navigation_and_stale_disconnect_are_safe() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    let original = app
        .world()
        .resource::<LastConnectionsProjection>()
        .0
        .clone();
    open(&mut app);
    assert_eq!(
        app.world().resource::<ConnectionsDrawerState>().selected,
        Some(0)
    );
    let layer = app
        .world_mut()
        .query_filtered::<Entity, With<ConnectionDrawerLayer>>()
        .single(app.world())
        .unwrap();
    assert_eq!(
        app.world().get::<Node>(layer).unwrap().display,
        Display::Flex
    );
    assert!(
        matches!(timing_availability(), Availability::Unsupported { reason } if reason.contains("DNS/TCP/TLS/TTFB"))
    );
    let close = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: close });
    app.update();
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    assert_eq!(
        app.world().get::<Node>(layer).unwrap().display,
        Display::None
    );
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert!(sink.submitted().is_empty());
    open(&mut app);
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.world_mut().insert_resource(keys);
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    open(&mut app);
    navigate_to(&mut app, Route::Logs);
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    assert_eq!(
        app.world().resource::<LastConnectionsProjection>().0,
        original
    );
    assert!(sink.submitted().is_empty());
}

#[test]
fn connection_drawer_unsupported_clipboard_does_not_report_success() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    open(&mut app);
    let copy = app
        .world_mut()
        .query_filtered::<Entity, With<CopyConnectionHostButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: copy });
    app.world_mut().flush();
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<ShellToast>>()
        .drain()
        .collect();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].kind, ToastKind::Warning);
    assert!(sink.submitted().is_empty());
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert_eq!(
        sink.submitted()
            .iter()
            .filter_map(UiCommand::to_intent)
            .collect::<Vec<_>>(),
        vec![CommandIntent::CloseConnection { id: "c-1".into() }]
    );
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert_eq!(sink.submitted().len(), 1);
}

#[test]
fn connection_drawer_refresh_follows_identity_through_reorder() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    open(&mut app);
    let mut projection = app
        .world()
        .resource::<LastConnectionsProjection>()
        .0
        .clone()
        .unwrap();
    projection.connections.swap(0, 1);
    projection.connections[1].host = "updated.example.test:443".into();
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(projection));
    app.update();
    app.update();
    let state = app.world().resource::<ConnectionsDrawerState>();
    assert_eq!(state.selected_id.as_deref(), Some("c-1"));
    assert_eq!(state.selected, Some(1));
    let host = app
        .world_mut()
        .query::<(&Text, &ConnDrawerField)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == ConnDrawerFieldKind::Host)
        .unwrap()
        .0;
    assert_eq!(host.0, "updated.example.test:443");
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert_eq!(
        sink.submitted()
            .iter()
            .filter_map(UiCommand::to_intent)
            .collect::<Vec<_>>(),
        vec![CommandIntent::CloseConnection { id: "c-1".into() }]
    );
}

#[test]
fn connection_drawer_refresh_dismisses_removed_identity_and_rejects_replay() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    navigate_to(&mut app, Route::Connections);
    open(&mut app);
    let mut projection = app
        .world()
        .resource::<LastConnectionsProjection>()
        .0
        .clone()
        .unwrap();
    projection.connections.retain(|item| item.id != "c-1");
    app.world_mut()
        .commands()
        .trigger(ConnectionsProjectionUpdated(projection));
    app.update();
    app.update();
    assert_eq!(
        *app.world().resource::<ConnectionsDrawerState>(),
        ConnectionsDrawerState::default()
    );
    let disconnect = app
        .world_mut()
        .query_filtered::<Entity, With<DrawerCloseConnectionButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: disconnect });
    app.update();
    assert!(sink.submitted().is_empty());
}
