//! Behavior cases for rules empty.
//! test-intent: behavior

use super::*;
use bevy::a11y::AccessibilityNode;
use bevy::ui::InteractionDisabled;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;

#[test]
fn test_rules_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.insert_resource(UiLocale::new("zh-CN"));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let empty = RulesProjection {
        total_rules: 0,
        default_action: "DIRECT".to_owned(),
        providers: vec![],
        rules: vec![],
        tracer: Default::default(),
        hit_audit: Default::default(),
        mrs_acceleration: Default::default(),
        truncated_rule_count: None,
        rule_publish_limit: RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    };
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 0 条规则 · 0 个规则集"
    ));
    assert!(subtree_has_text(app.world(), root, "最终匹配目标: DIRECT"));
    // An absent observation is not an observed empty counter.
    assert!(subtree_has_text(app.world(), root, "本地追踪统计 · 未观测"));
    // Empty tracer snapshot renders the honest empty state, never a
    // fabricated replay.
    assert!(subtree_has_text(app.world(), root, "输入目标并运行模拟"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "本地规则模拟；未发送流量，也未观测协议嗅探。"
    ));
    app.update();
    let clear = app
        .world_mut()
        .query::<(Entity, &ClearRuleHitCountersButton)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    assert!(app.world().get::<ButtonDisabled>(clear).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(clear).is_some());
    assert!(
        app.world()
            .get::<AccessibilityNode>(clear)
            .unwrap()
            .is_disabled()
    );
    let submitted = sink.submitted();
    app.world_mut().trigger(Activate { entity: clear });
    app.update();
    assert_eq!(
        sink.submitted(),
        submitted,
        "even an injected activation cannot clear an unobserved counter"
    );
}
