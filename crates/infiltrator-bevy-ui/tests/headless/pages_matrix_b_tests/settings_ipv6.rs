//! Behavior cases for settings ipv6.
//! test-intent: behavior

use super::*;
use bevy::ecs::hierarchy::Children;

#[test]
fn test_settings_ipv6_routing_projects_and_submits_live_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.ipv6_routing = Some(Ipv6RoutingSnapshot::new(7, false, true));
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "已禁用 IPv6 · TUN 已启用"
    ));

    let source = {
        let mut toggles = app.world_mut().query::<(&Ipv6RoutingToggle, &Children)>();
        *toggles
            .single(app.world())
            .expect("IPv6 routing toggle")
            .1
            .iter()
            .next()
            .expect("IPv6 routing checkbox")
    };
    app.world_mut().commands().trigger(ValueChange {
        source,
        value: true,
        is_final: true,
    });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetIpv6Routing { enabled: true }]
    );
}
