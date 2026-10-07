//! Behavior cases for profiles auto.
//! test-intent: behavior

use super::*;
use bevy::ui::Checked;

/// DUAL-07-09: the auto-reload control mirrors the shared projection and
/// submits the shared command; the checkbox follows the persisted value.
#[test]
fn test_profiles_auto_reload_toggle_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    let mut projection = subscription_fetch_projection();
    projection.profiles[0].auto_reload_core = true;
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "更新后自动重载内核"),
        "the reload control exists on the profile management surface"
    );
    assert!(
        subtree_has_text(app.world(), root, "内核重载：更新成功后请求重载内核"),
        "the reload status line renders the persisted preference"
    );
    let checked = {
        let mut toggles = app
            .world_mut()
            .query::<(&SubscriptionAutoReloadToggle, &Children)>();
        let (_, children) = toggles.single(app.world()).expect("reload toggle");
        children
            .iter()
            .any(|child| app.world().get::<Checked>(*child).is_some())
    };
    assert!(checked, "the checkbox reflects the shared snapshot");

    // Untick + save: the command carries the edited preference.
    if let Some(child) = {
        let mut toggles = app
            .world_mut()
            .query::<(&SubscriptionAutoReloadToggle, &Children)>();
        toggles
            .single(app.world())
            .expect("reload toggle")
            .1
            .iter()
            .next()
            .copied()
    } {
        app.world_mut().entity_mut(child).remove::<Checked>();
    }
    let save = marker_entity::<SaveSubscriptionAutoReloadButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetSubscriptionAutoReload {
            profile_id: "sub-fetch".to_owned(),
            enabled: false,
        }],
        "the reload preference rides the shared command"
    );
}
