//! Behavior cases for rules geo.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;
use bevy::ui_widgets;

#[test]
fn test_rules_geo_databases_button_submits_shared_intent() {
    use infiltrator_bevy_ui::pages::rules_mrs::UpgradeGeoDatabasesButton;

    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);

    let button = app
        .world_mut()
        .query_filtered::<Entity, With<UpgradeGeoDatabasesButton>>()
        .single(app.world())
        .expect("geo database update button");
    assert!(
        app.world().get::<ui_widgets::Button>(button).is_some(),
        "the entry point is a real button"
    );
    activate(&mut app, button);
    assert_eq!(sink.submitted(), vec![UiCommand::UpgradeGeoDatabases]);
    // The shared contract carries the intent both surfaces dispatch.
    assert_eq!(
        UiCommand::UpgradeGeoDatabases.to_intent(),
        Some(CommandIntent::UpgradeGeoDatabases)
    );
}
