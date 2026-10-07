//! Behavior cases for settings page.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_settings_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Settings);

    assert!(subtree_has_text(
        app.world(),
        root,
        "系统与内核全局设置 · 统一策略中枢"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "开机自动启动 (Autostart on Boot)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "设置系统代理 (Set System Proxy)"
    ));
    assert!(subtree_has_text(app.world(), root, "端口: 7890"));
    assert!(subtree_has_text(app.world(), root, "gVisor"));
    assert!(subtree_has_text(app.world(), root, "127.0.0.1:9090"));

    // TUN permission alert banner assertions
    assert!(subtree_has_text(app.world(), root, "准备 TUN 权限"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "启用 TUN 前需要为 mihomo 配置平台权限；完成后请重新开启 TUN。"
    ));

    // OS integration settings assertions
    assert!(subtree_has_text(app.world(), root, "关闭窗口最小化到托盘"));
    assert!(subtree_has_text(app.world(), root, "系统通知"));
    assert!(subtree_has_text(app.world(), root, "浅色模式"));
    assert!(subtree_has_text(app.world(), root, "深色模式"));
    assert!(subtree_has_text(app.world(), root, "护眼森林"));
    assert!(subtree_has_text(app.world(), root, "AMOLED"));
    assert!(subtree_has_text(app.world(), root, "跟随系统"));
    assert!(subtree_has_text(app.world(), root, "简体中文"));
    assert!(subtree_has_text(app.world(), root, "英语"));

    // Verify component markers exist in page tree
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<PrepareTunPermissionButton>>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<CloseToTrayToggle>>()
            .iter(app.world())
            .next()
            .is_some()
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<SystemNotificationsToggle>>()
            .iter(app.world())
            .next()
            .is_some()
    );
}
