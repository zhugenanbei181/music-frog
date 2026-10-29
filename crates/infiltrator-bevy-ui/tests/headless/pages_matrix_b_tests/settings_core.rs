//! Behavior cases for settings core.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use bevy::ui_widgets::RequestClose;
use infiltrator_bevy_ui::pages::settings::settings_dialogs::{
    CoreRollbackCancelButton, CoreRollbackConfirmButton, CoreRollbackDialog,
    CoreRollbackDialogMessage,
};
use infiltrator_bevy_widgets::modal::{ModalStage, ModalState};
use lurker_core_contract::identity::EngineId;

fn rollback_projection() -> SettingsProjection {
    let mut projection = SettingsProjection::demo();
    projection.core_versions.rollback = CoreRollbackSnapshot {
        engine_id: EngineId::new("mihomo").unwrap(),
        current: Some("v1.19.30".to_owned()),
        target: Some("v1.19.29".to_owned()),
        history: vec!["v1.19.29".to_owned()],
    };
    projection
}

fn open_rollback(app: &mut App, sink: &Arc<DemoCommandSink>) -> Entity {
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(rollback_projection()));
    app.update();

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<CoreRollbackButton>>()
        .single(app.world())
        .expect("core rollback button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "opening confirmation is not a command"
    );
    app.world_mut()
        .query_filtered::<Entity, With<CoreRollbackDialog>>()
        .single(app.world())
        .expect("core rollback dialog")
}

#[test]
fn core_rollback_first_activation_opens_confirmation_without_submitting() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let dialog = open_rollback(&mut app, &sink);
    let state = app
        .world()
        .get::<ModalState>(dialog)
        .expect("typed rollback stage");
    assert_eq!(state.stage, ModalStage::Open);

    let message = app
        .world_mut()
        .query_filtered::<&Text, With<CoreRollbackDialogMessage>>()
        .single(app.world())
        .expect("rollback version readback");
    assert_eq!(message.0, "v1.19.30 -> v1.19.29");
}

#[test]
fn core_rollback_confirm_submits_once_and_closes() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);
    let dialog = open_rollback(&mut app, &sink);

    let confirm = app
        .world_mut()
        .query_filtered::<Entity, With<CoreRollbackConfirmButton>>()
        .single(app.world())
        .expect("rollback confirmation button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: confirm });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RollbackCore]);
    assert_eq!(
        app.world()
            .get::<ModalState>(dialog)
            .expect("typed rollback stage")
            .stage,
        ModalStage::Hidden
    );

    app.world_mut()
        .commands()
        .trigger(RequestClose { source: dialog });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::RollbackCore],
        "a second close cannot submit the command again"
    );
}

#[test]
fn core_rollback_cancel_and_escape_never_submit() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);
    let dialog = open_rollback(&mut app, &sink);

    let cancel = app
        .world_mut()
        .query_filtered::<Entity, With<CoreRollbackCancelButton>>()
        .single(app.world())
        .expect("rollback cancel button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: cancel });
    app.update();
    assert!(sink.submitted().is_empty());
    assert_eq!(
        app.world()
            .get::<ModalState>(dialog)
            .expect("typed rollback stage")
            .stage,
        ModalStage::Dismissed
    );

    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(rollback_projection()));
    app.update();
    let button = app
        .world_mut()
        .query_filtered::<Entity, With<CoreRollbackButton>>()
        .single(app.world())
        .expect("core rollback button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();
    app.world_mut()
        .commands()
        .trigger(RequestClose { source: dialog });
    app.update();
    assert!(sink.submitted().is_empty());
    assert_eq!(
        app.world()
            .get::<ModalState>(dialog)
            .expect("typed rollback stage")
            .stage,
        ModalStage::Dismissed
    );
}
