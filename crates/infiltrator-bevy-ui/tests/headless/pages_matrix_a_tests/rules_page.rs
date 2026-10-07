//! Behavior cases for rules page.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES;

#[test]
fn test_rules_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    app.insert_resource(UiLocale::new("zh-CN"));
    let (root, _) = navigate_to(&mut app, Route::Rules);

    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 2842 条规则 · 3 个规则集"
    ));
    assert!(subtree_has_text(app.world(), root, "最终匹配目标: DIRECT"));
    assert!(subtree_has_text(app.world(), root, "刷新规则集"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "geosite-geolocation-!cn"
    ));
    assert!(subtree_has_text(app.world(), root, "google.com"));
    assert!(subtree_has_text(app.world(), root, "1420 次本地追踪命中"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "实时分流追踪器沙盒 (Live Rule Tracer)"
    ));
    assert!(subtree_has_text(app.world(), root, "执行模拟追踪"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "命中规则 #42：DOMAIN-SUFFIX,github.com,PROXY → PROXY"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "没有抓包或嗅探结果；目标来自输入的查询。"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 二进制规则集治理与解构 (MRS Ruleset Engine)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "一键解构导入为本地规则"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "添加自定义规则向导 (Add Custom Rule)"
    ));
    assert!(subtree_has_text(app.world(), root, "一键注入游戏分流预设"));
    assert!(subtree_has_text(app.world(), root, "+ 确认添加规则"));
    // DUAL-11-09: the shared disabled flag reaches the row label.
    assert!(subtree_has_text(app.world(), root, "已停用"));
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<RuleToggleButton>>()
            .iter(app.world())
            .count()
            == 5,
        "one toggle control per demo rule"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<RuleTypeChip>>()
            .iter(app.world())
            .count()
            == CUSTOM_RULE_TYPE_CHOICES.len(),
        "wizard exposes the shared rule-type vocabulary"
    );
    assert!(subtree_has_text(
        app.world(),
        root,
        "geoip-cn.mrs · 8500 条目 · ipcidr · 校验通过 · sha256 e3b0c44298fc"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "geosite-geolocation-!cn.mrs · 28400 条目 · domain · 校验通过 · sha256 cbf529a4d5d4"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 加速就绪 · 3 个规则集 · 37472 条规则"
    ));
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<RulesMrsRoot>>()
            .iter(app.world())
            .next()
            .is_some(),
        "RulesMrsRoot marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<UnpackRuleProviderButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "UnpackRuleProviderButton marker exists"
    );
}

/// DUAL-11-06/07: the rendered Rules page carries the observed cache line.
#[test]
fn test_rules_page_renders_observed_provider_cache_fact() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Rules);
    let root = app
        .world_mut()
        .query_filtered::<Entity, With<RulesPageRoot>>()
        .single(app.world())
        .expect("rules page root");
    assert!(subtree_has_text(
        app.world(),
        root,
        "~/.config/mihomo-rs/configs/rules · 3 个缓存文件 · 1048576 字节"
    ));
}
