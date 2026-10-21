//! Behavior cases for settings prepare.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_settings_prepare_tun_and_toggles_submit_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let prepare_btn = app
        .world_mut()
        .query_filtered::<Entity, With<PrepareTunPermissionButton>>()
        .single(app.world())
        .expect("prepare tun button");
    app.world_mut().commands().trigger(Activate {
        entity: prepare_btn,
    });
    app.update();

    let tray_toggle = app
        .world_mut()
        .query_filtered::<Entity, With<CloseToTrayToggle>>()
        .single(app.world())
        .expect("close to tray toggle");
    app.world_mut().commands().trigger(Activate {
        entity: tray_toggle,
    });
    app.update();

    let notif_toggle = app
        .world_mut()
        .query_filtered::<Entity, With<SystemNotificationsToggle>>()
        .single(app.world())
        .expect("system notifications toggle");
    app.world_mut().commands().trigger(Activate {
        entity: notif_toggle,
    });
    app.update();

    let debug_button = {
        let world = app.world_mut();
        let mut query = world.query::<(Entity, &CoreLogLevelButton)>();
        query
            .iter(world)
            .find(|(_, button)| button.level == CoreLogLevel::Debug)
            .map(|(entity, _)| entity)
            .expect("debug core log level button")
    };
    app.world_mut().commands().trigger(Activate {
        entity: debug_button,
    });
    app.update();

    let service_button = app
        .world_mut()
        .query_filtered::<Entity, With<ServiceModeButton>>()
        .single(app.world())
        .expect("service mode button");
    app.world_mut().commands().trigger(Activate {
        entity: service_button,
    });
    app.update();

    let port_button = app
        .world_mut()
        .query_filtered::<Entity, With<PortConflictButton>>()
        .single(app.world())
        .expect("port conflict repair button");
    app.world_mut().commands().trigger(Activate {
        entity: port_button,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![
            // The TUN permission banner asks the shared owner for the
            // privileged service mode, exactly like the service-mode row.
            UiCommand::PrepareServiceMode,
            UiCommand::UpdateSetting {
                key: "close_to_tray".to_owned(),
                value: "false".to_owned(),
            },
            UiCommand::UpdateSetting {
                key: "notifications_enabled".to_owned(),
                value: "false".to_owned(),
            },
            UiCommand::SetCoreLogLevel(CoreLogLevel::Debug),
            UiCommand::PrepareServiceMode,
            UiCommand::RepairPortConflicts,
        ]
    );
}

/// The TUN permission banner and the service-mode row are two doors to the
/// *same* shared owner command: the banner no longer writes a settings-store
/// key that no owner answers.
#[test]
fn the_tun_permission_banner_submits_the_shared_service_mode_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let banner = app
        .world_mut()
        .query_filtered::<Entity, With<PrepareTunPermissionButton>>()
        .single(app.world())
        .expect("the TUN permission banner's prepare button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: banner });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::PrepareServiceMode],
        "the banner submits the shared service-mode command"
    );
    assert!(
        !sink.submitted().iter().any(|command| matches!(
            command,
            UiCommand::UpdateSetting { key, .. } if key == "tun_privilege"
        )),
        "no settings-store key stands in for the shared command"
    );
}
