//! Behavior cases for settings projection.
//! test-intent: behavior

use super::*;
use bevy::ecs::hierarchy::Children;
use infiltrator_bevy_ui::route::PageRoot;
use infiltrator_bevy_widgets::localization::UiLocale;

#[test]
fn test_settings_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    app.insert_resource(UiLocale::new("zh-CN"));
    let (root, _) = navigate_to(&mut app, Route::Settings);

    let mut updated = SettingsProjection::demo();
    updated.mixed_port = Some(7899);
    updated.controller_port = Some(9191);
    updated.log_level = Some("debug".to_owned());
    updated.tun_auto_route = Some(false);
    updated.tun_strict_route = Some(true);
    updated.offline_startup = OfflineStartupSnapshot::ready(LocalAssetStatus::Missing);
    updated.mtu = MtuNegotiationSnapshot::ready(
        2,
        PhysicalMtuSnapshot {
            interface: "eth0".to_owned(),
            mtu: 1500,
        },
        1420,
        1380,
    );

    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(app.world(), root, "端口: 7899"));
    assert!(subtree_has_text(app.world(), root, "127.0.0.1:9191"));
    assert!(subtree_has_text(app.world(), root, "DEBUG"));
    assert!(subtree_has_text(app.world(), root, "离线优先"));
    assert!(subtree_has_text(app.world(), root, "可启动但已降级"));
    assert!(subtree_has_text(app.world(), root, "物理=1500"));

    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert!(app.world().get::<PageRoot>(root).is_some());
    assert!(subtree_has_text(app.world(), root, "Port: 7899"));
    assert!(subtree_has_text(app.world(), root, "physical=1500"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Offline-ready · degraded"
    ));

    let route_sources: Vec<(TunRouteToggleKind, Entity)> = {
        let mut toggles = app.world_mut().query::<(&TunRouteToggle, &Children)>();
        toggles
            .iter(app.world())
            .map(|(toggle, children)| {
                (
                    toggle.0,
                    *children.iter().next().expect("route toggle checkbox"),
                )
            })
            .collect()
    };
    for (kind, source) in route_sources {
        let checked = app.world().get::<Checked>(source).is_some();
        assert_eq!(
            checked,
            matches!(kind, TunRouteToggleKind::StrictRoute),
            "route checkbox should follow the shared projection"
        );
    }
}
