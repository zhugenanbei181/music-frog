//! Behavior cases for settings save.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_settings_save_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<SaveSettingsButton>>()
        .single(app.world())
        .expect("save settings button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateSetting {
            key: "apply".to_owned(),
            value: "true".to_owned(),
        }]
    );
}
