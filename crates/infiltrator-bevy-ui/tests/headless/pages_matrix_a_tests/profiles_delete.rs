//! Behavior cases for profiles delete.
//! test-intent: behavior

use super::*;

/// DUAL-07-14: the card actions delete a profile through the shared command
/// bus, and the active profile's delete action is never submitted — exactly
/// like the Iced card, which only offers the action for inactive profiles.
#[test]
fn test_profiles_delete_button_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    let active = app
        .world_mut()
        .query::<(Entity, &DeleteProfileButton)>()
        .iter(app.world())
        .find(|(_, button)| button.profile_id == "sub-1")
        .map(|(entity, _)| entity)
        .expect("sub-1 delete button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: active });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "the active profile is never deleted from this surface"
    );

    let inactive = app
        .world_mut()
        .query::<(Entity, &DeleteProfileButton)>()
        .iter(app.world())
        .find(|(_, button)| button.profile_id == "sub-2")
        .map(|(entity, _)| entity)
        .expect("sub-2 delete button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: inactive });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::DeleteProfile {
            id: "sub-2".to_owned(),
        }],
        "delete routes through the shared command"
    );
}
