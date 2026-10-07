//! Behavior cases for profiles update.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_profiles_update_button_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(ProfilesProjection::demo()));
    app.update();

    let update = app
        .world_mut()
        .query::<(Entity, &UpdateProfileButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == 1)
        .map(|(entity, _)| entity)
        .expect("sub-2 update button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: update });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::UpdateProfile {
            id: "sub-2".to_owned(),
        }],
        "per-profile update routes through the shared command (retry/backoff + single-flight)"
    );
}

#[test]
fn test_profiles_update_all_toolbar_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);

    assert!(
        subtree_has_text(app.world(), root, "一键更新全部订阅"),
        "toolbar exposes the batch update entry"
    );

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<UpdateAllSubscriptionsButton>>()
        .single(app.world())
        .expect("update-all button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.update();

    assert_eq!(sink.submitted(), vec![UiCommand::UpdateAllSubscriptions]);
}
