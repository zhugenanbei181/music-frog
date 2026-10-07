//! Behavior cases for profiles page.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_profiles_page_mounting_and_default_state() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    assert!(subtree_has_text(
        app.world(),
        root,
        "配置订阅 · 共 3 个配置 (当前生效: Primary VIP)"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动更新: 2 个订阅已启用 · 最短周期 6 小时"
    ));
    assert!(subtree_has_text(app.world(), root, "Primary VIP"));
    assert!(subtree_has_text(app.world(), root, "当前生效中"));
    assert!(subtree_has_text(app.world(), root, "点击启用"));
    assert!(subtree_has_text(app.world(), root, "导入本地配置文件"));
    assert!(subtree_has_text(app.world(), root, "选择文件"));
    assert!(subtree_has_text(app.world(), root, "+ 导入本地文件"));
    assert!(subtree_has_text(app.world(), root, "导入后立即激活"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "订阅请求设置 (Subscription User-Agent)"
    ));
    assert!(subtree_has_text(app.world(), root, "保存请求设置"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "多订阅节点聚合器 (Profile Aggregator)"
    ));
    assert!(subtree_has_text(app.world(), root, "预览聚合结果"));
    assert!(subtree_has_text(app.world(), root, "保存为新配置"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "配置历史快照比对 (Snapshot Visual Diff)"
    ));
    assert!(subtree_has_text(app.world(), root, "一键安全还原此快照"));
    assert!(subtree_has_text(app.world(), root, "脚本指令 DSL 控制台"));
    assert!(subtree_has_text(app.world(), root, "预设:"));
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<ProfilesImportRoot>>()
            .iter(app.world())
            .next()
            .is_some(),
        "ProfilesImportRoot marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<ChooseLocalFileButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "ChooseLocalFileButton marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<ImportLocalFileButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "ImportLocalFileButton marker exists"
    );
    assert!(
        app.world_mut()
            .query_filtered::<Entity, With<SaveUserAgentButton>>()
            .iter(app.world())
            .next()
            .is_some(),
        "SaveUserAgentButton marker exists"
    );
}
