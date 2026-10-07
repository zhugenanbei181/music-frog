//! Behavior cases for sidebar.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_contract::system_proxy::{SystemProxyObservation, SystemProxySnapshot};
use infiltrator_contract::system_toggle::SystemToggleState;

#[test]
fn test_sidebar_system_toggles_use_shared_projection_and_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));

    let proxy = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarSystemProxyToggle>>()
        .single(app.world())
        .expect("sidebar system proxy toggle");
    let tun = app
        .world_mut()
        .query_filtered::<Entity, With<SidebarTunToggle>>()
        .single(app.world())
        .expect("sidebar TUN toggle");

    assert!(matches!(
        &app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        SystemToggleState::Enabled
    ));

    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSystemProxy { enabled: false }]
    );
    assert!(matches!(
        &app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        SystemToggleState::Pending { desired: false }
    ));

    // A second activation while the application command is in flight is
    // fenced instead of producing a duplicate command.
    app.world_mut()
        .commands()
        .trigger(Activate { entity: proxy });
    app.update();
    assert_eq!(sink.submitted().len(), 1);

    let mut next = DemoSurfaceSource::running().surface_snapshot();
    next.revision = 2;
    next.system_proxy = SystemProxySnapshot::from_observation(
        2,
        SystemProxyObservation {
            enabled: false,
            endpoint: None,
            bypass: None,
        },
    );
    next.runtime_control.tun_enabled = Some(false);
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(next));
    app.update();

    let proxy_visual = app
        .world()
        .get::<ControlVisual>(proxy)
        .expect("proxy visual");
    let tun_visual = app.world().get::<ControlVisual>(tun).expect("tun visual");
    assert!(!proxy_visual.0);
    assert!(!tun_visual.0);
    let proxy_label = app
        .world()
        .get::<Children>(proxy)
        .expect("proxy toggle children")
        .iter()
        .find_map(|child| {
            app.world()
                .get::<PillLabel>(*child)
                .and_then(|_| app.world().get::<Text>(*child))
        })
        .map(|text| text.0.clone());
    assert_eq!(proxy_label.as_deref(), Some("关"));
    assert_eq!(
        app.world()
            .resource::<SidebarToggleProjection>()
            .0
            .system_proxy,
        SystemToggleState::Disabled
    );

    app.world_mut().commands().trigger(Activate { entity: tun });
    app.update();
    assert_eq!(
        sink.submitted(),
        vec![
            UiCommand::SetSystemProxy { enabled: false },
            UiCommand::ToggleTun { enabled: true },
        ]
    );
}
