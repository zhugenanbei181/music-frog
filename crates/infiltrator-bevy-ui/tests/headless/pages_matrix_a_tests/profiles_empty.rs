//! Behavior cases for profiles empty.
//! test-intent: behavior

use super::*;

#[test]
fn test_profiles_empty_and_edge_case_projection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let empty = ProfilesProjection {
        profiles: vec![],
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
        yaml_ast_diff: None,
        snapshot_history: None,
        apply_transaction: None,
        profile_document: None,
        profile_options: None,
        editor_read: Default::default(),
        script_sandbox: None,
        script_export: None,
    };
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(empty));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "配置订阅 · 共 0 个配置 (当前生效: 无活动配置)"
    ));
    assert!(subtree_has_text(app.world(), root, "自动更新: 未启用"));
}
