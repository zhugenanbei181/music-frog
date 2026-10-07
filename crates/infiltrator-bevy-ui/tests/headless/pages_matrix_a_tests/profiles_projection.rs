//! Behavior cases for profiles projection.
//! test-intent: behavior

use super::*;

#[test]
fn test_profiles_projection_in_place_update() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut updated = ProfilesProjection::demo();
    updated.auto_update_interval_hours = 3;
    updated.profiles[0].is_active = false;
    updated.profiles[1].is_active = true;
    updated.profiles[1].name = "备用容灾线路 (Active Live)".to_owned();
    updated.profiles[1].update_interval_hours = Some(3);

    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(updated));
    app.update();

    assert!(subtree_has_text(
        app.world(),
        root,
        "配置订阅 · 共 3 个配置 (当前生效: 备用容灾线路 (Active Live))"
    ));
    assert!(subtree_has_text(
        app.world(),
        root,
        "自动更新: 2 个订阅已启用 · 最短周期 3 小时"
    ));
    assert!(subtree_has_text(app.world(), root, "定时计划：每 3 小时"));
    assert!(subtree_has_text(
        app.world(),
        root,
        "备用容灾线路 (Active Live)"
    ));

    let mut btn_query = app
        .world_mut()
        .query::<(&ActivateProfileButton, &ControlVisual)>();
    let sub2_visual = btn_query
        .iter(app.world())
        .find(|(btn, _)| btn.profile_id == "sub-2")
        .map(|(_, visual)| visual.0);
    assert_eq!(sub2_visual, Some(true), "sub-2 is active visual");
}
