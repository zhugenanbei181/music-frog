//! Behavior cases for rules refresh.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_rules_refresh_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<RefreshRuleProvidersButton>>()
        .single(app.world())
        .expect("refresh rules button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::RefreshRuleProviders]);
}
