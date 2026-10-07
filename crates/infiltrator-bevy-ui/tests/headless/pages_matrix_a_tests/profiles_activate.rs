//! Behavior cases for profiles activate.
//! test-intent: behavior

use super::*;

#[test]
fn test_profiles_activate_button_submits_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);

    let mut query = app.world_mut().query::<(Entity, &ActivateProfileButton)>();
    let (btn_entity, _) = query
        .iter(app.world())
        .find(|(_, btn)| btn.profile_id == "sub-2")
        .expect("sub-2 activate button");

    app.world_mut()
        .commands()
        .trigger(Activate { entity: btn_entity });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ActivateProfile {
            id: "sub-2".to_owned(),
        }]
    );
}
