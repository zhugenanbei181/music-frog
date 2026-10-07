//! Behavior cases for app.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_app_routing_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::AppRouting);

    assert!(subtree_has_text(
        app.world(),
        root,
        "应用分流 · 仅代理选中应用 (已配置 6 个应用)"
    ));
    assert!(subtree_has_text(app.world(), root, "Google Chrome"));
    assert!(subtree_has_text(app.world(), root, "代理 (Proxy)"));
    assert!(subtree_has_text(app.world(), root, "Steam"));
    assert!(subtree_has_text(app.world(), root, "直连 (Direct)"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "Windows UWP 回环隔离豁免工具 (UWP Loopback Exemption)"
    ));
    assert!(subtree_has_text(app.world(), root, "全选豁免 (Exempt All)"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "已扫描 3 个 UWP AppContainer · 已豁免 2 个"
    ));
}

#[test]
fn test_app_routing_uwp_actions_submit_shared_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::AppRouting);

    let exempt_all = {
        let mut actions = app.world_mut().query::<(Entity, &UwpActionButton)>();
        actions
            .iter(app.world())
            .find(|(_, action)| action.0 == UwpAction::ExemptAll)
            .map(|(entity, _)| entity)
            .expect("UWP exempt-all action")
    };
    app.world_mut()
        .commands()
        .trigger(Activate { entity: exempt_all });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAllUwpExemptions { exempt: true }]
    );
}

#[test]
fn test_app_routing_add_rule_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::AppRouting);

    let btn_entity = app
        .world_mut()
        .query_filtered::<Entity, With<AddAppRouteButton>>()
        .single(app.world())
        .expect("add app rule button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAppRule {
            app_id: "new-app".to_owned(),
            rule: "Proxy".to_owned(),
        }]
    );
}

#[test]
fn test_app_routing_switch_rule_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::AppRouting);

    let mut query = app.world_mut().query::<(Entity, &SwitchAppRuleButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.app_id == "app-1")
        .expect("app-1 switch rule button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAppRule {
            app_id: "app-1".to_owned(),
            rule: "Direct".to_owned(),
        }]
    );
}

#[test]
fn test_app_routing_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink);
    let (root, _) = navigate_to(&mut app, Route::AppRouting);

    let mut updated = AppRoutingProjection::demo();
    updated.mode = AppRoutingMode::BypassSelected;
    updated.apps[0].rule = AppRoutingRule::Block;

    app.world_mut()
        .commands()
        .trigger(AppRoutingProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "应用分流 · 选中应用直连，其余代理 (已配置 6 个应用)"
    ));
    assert!(subtree_has_text(app.world(), root, "拦截 (Block)"));
}
