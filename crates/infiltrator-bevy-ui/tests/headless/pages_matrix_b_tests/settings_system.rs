//! Behavior cases for settings system.
//! test-intent: behavior

use super::*;
use bevy::ecs::hierarchy::Children;

#[test]
fn test_settings_system_proxy_checkbox_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Settings);

    let source = {
        let mut toggles = app.world_mut().query::<(&SystemProxyToggle, &Children)>();
        *toggles
            .single(app.world())
            .expect("system proxy toggle")
            .1
            .iter()
            .next()
            .expect("system proxy checkbox")
    };
    app.world_mut().commands().trigger(ValueChange {
        source,
        value: false,
        is_final: true,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSystemProxy { enabled: false }]
    );
}

#[test]
fn test_settings_system_proxy_recovery_status_projects_shared_result() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.system_proxy_recovery.status = SystemProxyRecoveryStatus::Restored {
        previous: SystemProxyObservation::default(),
        restored: SystemProxyObservation::default(),
    };
    projection.system_proxy_recovery.revision = 9;
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection.clone()));
    app.update();
    assert!(subtree_has_text(app.world(), root, "启动已清理孤儿代理"));

    projection.system_proxy_recovery.status = SystemProxyRecoveryStatus::SkippedExternal {
        expected: SystemProxyDesiredState {
            enabled: true,
            endpoint: Some("127.0.0.1:7890".to_owned()),
            bypass: None,
        },
        observed: SystemProxyObservation {
            enabled: true,
            endpoint: Some("127.0.0.1:9999".to_owned()),
            bypass: None,
        },
    };
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "检测到外部修改，未覆盖"
    ));
}
