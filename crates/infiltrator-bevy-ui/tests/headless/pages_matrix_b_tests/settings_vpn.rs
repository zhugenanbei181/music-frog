//! Behavior cases for settings vpn.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_settings_vpn_projection_and_actions_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.vpn = VpnSessionSnapshot {
        state: VpnSessionState::PermissionRequired,
        foreground: false,
        revision: 3,
        ..VpnSessionSnapshot::default()
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(app.world(), root, "等待授权"));

    let start = app
        .world_mut()
        .query_filtered::<Entity, With<VpnStartButton>>()
        .single(app.world())
        .expect("VPN start button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: start });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::StartVpn]);

    sink.clear();
    let stop = app
        .world_mut()
        .query_filtered::<Entity, With<VpnStopButton>>()
        .single(app.world())
        .expect("VPN stop button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: stop });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "permission waiting does not imply a running service to stop"
    );
    let mut running = SettingsProjection::demo();
    running.vpn = VpnSessionSnapshot {
        state: VpnSessionState::Running,
        revision: 4,
        ..Default::default()
    };
    app.world_mut().trigger(SettingsProjectionUpdated(running));
    app.update();
    app.world_mut().trigger(Activate { entity: stop });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::StopVpn]);
}
