//! Behavior cases for rules truncation.
//! test-intent: behavior

use super::*;
use infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT;

/// DUAL-11-08: the truncation fact is rendered when the published list is a
/// capped view of the profile list, and the paging indicator keeps its bounds.
#[test]
fn test_rules_truncation_note_reports_publish_cap() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.total_rules = 50_000;
    projection.truncated_rule_count = Some(45_000);
    projection.rule_publish_limit = RULE_PUBLISH_LIMIT;
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "共享发布视口已截断：已省略 45000 条（发布上限 5000 条；编辑器列表为全量并按虚拟窗口渲染）"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "分流规则 · 共 50000 条规则 · 3 个规则集"
    ));
}
