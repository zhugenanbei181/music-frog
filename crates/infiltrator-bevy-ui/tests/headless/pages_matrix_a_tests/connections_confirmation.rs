//! Behavior cases for connections confirmation.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use infiltrator_bevy_ui::command_palette::{CommandPaletteState, ExecuteSelectedPaletteAction};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_catalogue::CommandTarget;

#[test]
fn test_connections_close_all_requires_confirmation() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<CloseAllConnectionsButton>>()
        .single(app.world())
        .expect("close all connections button");

    // First click only arms the destructive action.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();
    assert!(sink.submitted().is_empty());
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<CloseAllConfirmationRoot>>()
        .single(app.world())
        .expect("confirmation layer");
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::Flex
    );
    // The original trigger remains an opener, even when clicked repeatedly.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
    let confirm = confirmation_action(&mut app, CloseAllConfirmationAction::Confirm);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: confirm });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::CloseAllConnections]);
    assert_eq!(
        sink.submitted()
            .iter()
            .filter_map(UiCommand::to_intent)
            .collect::<Vec<_>>(),
        vec![CommandIntent::CloseAllConnections]
    );
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    // Replayed/stale activation after the dialog closed cannot execute twice.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: confirm });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::CloseAllConnections]);
}

#[test]
fn close_all_modal_cancel_escape_and_navigation_have_no_side_effects() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);
    let opener = app
        .world_mut()
        .query_filtered::<Entity, With<CloseAllConnectionsButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: opener });
    app.update();
    assert!(app.world().resource::<ConnectionsCloseAllState>().armed);
    let cancel = confirmation_action(&mut app, CloseAllConfirmationAction::Cancel);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    assert!(!app.world().resource::<ConnectionsCloseAllState>().armed);
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
    app.world_mut()
        .commands()
        .trigger(Activate { entity: opener });
    app.update();
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.world_mut().insert_resource(keys);
    app.update();
    assert!(!app.world().resource::<ConnectionsCloseAllState>().armed);
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: opener });
    app.update();
    assert!(app.world().resource::<ConnectionsCloseAllState>().armed);
    navigate_to(&mut app, Route::Logs);
    assert!(!app.world().resource::<ConnectionsCloseAllState>().armed);
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
}

#[test]
fn missing_close_all_confirmation_state_never_executes_destruction() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Connections);
    let opener = app
        .world_mut()
        .query_filtered::<Entity, With<CloseAllConnectionsButton>>()
        .single(app.world())
        .unwrap();
    app.world_mut()
        .remove_resource::<ConnectionsCloseAllState>();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: opener });
    app.world_mut().flush();
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
}

#[test]
fn command_palette_close_all_opens_confirmation_on_its_real_route() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Logs);
    {
        let mut palette = app.world_mut().resource_mut::<CommandPaletteState>();
        palette.open();
        palette.set_query("close");
        let target = palette.current_selected_action().unwrap().target.clone();
        assert_eq!(target, CommandTarget::CloseAllConnections);
    }
    app.world_mut()
        .commands()
        .trigger(ExecuteSelectedPaletteAction);
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Connections)
    );
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
    assert!(app.world().resource::<ConnectionsCloseAllState>().armed);
    let cancel = confirmation_action(&mut app, CloseAllConfirmationAction::Cancel);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    assert!(!app.world().resource::<ConnectionsCloseAllState>().armed);
    assert_eq!(sink.submitted(), Vec::<UiCommand>::new());
}
