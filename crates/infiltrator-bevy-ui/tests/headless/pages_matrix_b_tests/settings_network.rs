//! Behavior cases for settings network.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_application::network_status_projection::{roaming_interfaces, roaming_route};
use infiltrator_bevy_ui::pages::settings::settings_network_roaming::NetworkRoamingRouteLine;
use infiltrator_bevy_widgets::localization::UiLocale;

#[test]
fn test_settings_network_roaming_projection_and_actions_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut projection = SettingsProjection::demo();
    projection.network_roaming = NetworkRoamingSnapshot {
        status: NetworkRoamingStatus::Stable,
        interfaces: vec![NetworkInterfaceSnapshot {
            name: "eth0".to_owned(),
            kind: NetworkInterfaceKind::Ethernet,
            is_up: true,
            is_default_gateway: true,
            gateway_ip: Some("192.0.2.1".to_owned()),
            ip_addresses: vec!["192.0.2.10/24".to_owned()],
            mtu: Some(1500),
            metric: Some(100),
            dns_servers: Vec::new(),
        }],
        active_interface: Some("eth0".to_owned()),
        default_gateway: Some("192.0.2.1".to_owned()),
        tun_interface: Some("Meta".to_owned()),
        physical_mtu: Some(1500),
        recommended_tun_mtu: Some(1420),
        tcp_mss: Some(1380),
        ..NetworkRoamingSnapshot::default()
    };
    let roaming = projection.network_roaming.clone();
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        &roaming_route(&roaming, "zh-CN")
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        &roaming_interfaces(&roaming, "zh-CN")
    ));

    let route_line = app
        .world_mut()
        .query_filtered::<Entity, With<NetworkRoamingRouteLine>>()
        .single(app.world())
        .expect("mounted roaming route line");
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(
        app.world().get::<Text>(route_line).unwrap().0,
        roaming_route(&roaming, "en-US")
    );
    assert!(subtree_has_text(
        app.world(),
        root,
        "Physical MTU 1500 → TUN 1420 · MSS 1380"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "eth0 [up] gw=192.0.2.1"
    ));

    let refresh = app
        .world_mut()
        .query_filtered::<Entity, With<NetworkRoamingRefreshButton>>()
        .single(app.world())
        .expect("network roaming refresh button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: refresh });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::RefreshNetworkRoaming]);

    sink.clear();
    let repair = app
        .world_mut()
        .query_filtered::<Entity, With<NetworkRoamingRepairButton>>()
        .single(app.world())
        .expect("network roaming repair button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: repair });
    app.update();
    assert_eq!(sink.submitted(), vec![UiCommand::RepairNetworkRoutes]);
}
