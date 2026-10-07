//! Behavior cases for settings mtu.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_settings_mtu_probe_submits_shared_application_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<ProbeTunMtuButton>>()
        .single(app.world())
        .expect("MTU probe button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::ProbeTunMtu]);
}
