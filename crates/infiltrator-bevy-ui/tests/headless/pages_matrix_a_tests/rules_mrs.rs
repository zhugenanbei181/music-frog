//! Behavior cases for rules mrs.
//! test-intent: behavior

use super::*;

#[test]
fn test_rules_mrs_renders_shared_snapshot_not_fabricated() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Rules);

    let mut projection = RulesProjection::demo();
    projection.mrs_acceleration = MrsAccelerationSnapshot::ready(
        1,
        1,
        vec![rules_mrs_test_item(
            "custom-test.mrs",
            MrsBehaviorKind::IpCidr,
            1234,
        )],
        true,
    );
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(projection));
    app.update();

    // DUAL-11-03: the row is rendered from the shared snapshot, digest included.
    assert!(subtree_has_text(
        app.world(),
        root,
        "custom-test.mrs · 1234 条目 · ipcidr · 校验通过 · sha256 deadbeefcafe"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 加速就绪 · 1 个规则集"
    ));
    // The previously hardcoded fabricated item must be gone.
    assert!(!subtree_has_text(app.world(), root, "14,200 条目"));

    // An unsupported snapshot renders its honest typed status, not a fake list.
    let mut unsupported = RulesProjection::demo();
    unsupported.mrs_acceleration =
        MrsAccelerationSnapshot::unsupported(1, 1, "内核未配置规则集提供者网关");
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(unsupported));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        root,
        "MRS 不受支持: 内核未配置规则集提供者网关"
    ));
}
