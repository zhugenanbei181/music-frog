//! Behavior cases for settings core.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use lurker_core_contract::identity::EngineId;

#[test]
fn test_settings_core_rollback_button_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.core_versions.rollback = CoreRollbackSnapshot {
        engine_id: EngineId::new("mihomo").unwrap(),
        current: Some("v1.19.30".to_owned()),
        target: Some("v1.19.29".to_owned()),
        history: vec!["v1.19.29".to_owned()],
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
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

    assert_eq!(sink.submitted(), vec![UiCommand::RollbackCore]);
}
