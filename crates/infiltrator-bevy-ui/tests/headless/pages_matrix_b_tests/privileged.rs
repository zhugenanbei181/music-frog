//! Behavior cases for privileged.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;

#[test]
fn test_privileged_network_regression_button_uses_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<PrivilegedNetworkRunButton>>()
        .single(app.world())
        .expect("privileged network regression button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert!(
        sink.submitted().is_empty(),
        "the demo declares an unsupported host"
    );
    let mut projection = SettingsProjection::demo();
    projection.privileged_network = PrivilegedNetworkSnapshot::default();
    app.world_mut()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    app.world_mut().trigger(Activate { entity: button });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::RunPrivilegedNetworkRegression]
    );
}
