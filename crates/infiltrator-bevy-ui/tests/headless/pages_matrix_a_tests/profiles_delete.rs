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

/// A refused delete is visible on the profiles page: the page renders the
/// owner's typed answer instead of pretending the action never happened.
#[test]
fn a_refused_delete_is_rendered_on_the_profiles_page() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

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

    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::DeleteProfile {
            id: "sub-2".to_owned(),
        },
        request_id: RequestId::new(1),
        result: Err(Failure::new(
            ErrorCode::NotReady,
            "the profile is still in use",
            true,
        )),
    });
    app.update();

    let line = app
        .world_mut()
        .query_filtered::<Entity, With<ProfileDeleteOutcomeText>>()
        .single(app.world())
        .expect("the delete outcome line");
    let rendered = app
        .world()
        .get::<Text>(line)
        .expect("the delete outcome text")
        .0
        .clone();
    assert!(
        rendered.contains("the profile is still in use"),
        "the refused delete is visible on the page, got {rendered}"
    );
}
