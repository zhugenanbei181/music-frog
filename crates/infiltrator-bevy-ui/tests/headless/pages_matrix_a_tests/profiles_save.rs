//! Behavior cases for profiles save.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_profiles_save_fetch_settings_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    let field = {
        let mut fields = app
            .world_mut()
            .query::<(&SubscriptionUserAgentField, &Children)>();
        *fields
            .single(app.world())
            .expect("ua field wrapper")
            .1
            .iter()
            .next()
            .expect("ua text field")
    };
    app.world_mut()
        .get_mut::<TextField>(field)
        .expect("ua field state")
        .0
        .apply(TextFieldInput::SetText("Custom-UA/9".to_owned()));

    let save = app
        .world_mut()
        .query_filtered::<Entity, With<SaveUserAgentButton>>()
        .single(app.world())
        .expect("save button");
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SaveSubscriptionFetchSettings {
            profile_id: "sub-fetch".to_owned(),
            user_agent: Some("Custom-UA/9".to_owned()),
            insecure_skip_verify: true,
        }]
    );
}
