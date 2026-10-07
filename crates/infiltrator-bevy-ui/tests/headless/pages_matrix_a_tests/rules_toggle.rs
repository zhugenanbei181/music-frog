//! Behavior cases for rules toggle.
//! test-intent: behavior

use super::rules_fixture::seed_rule_draft;
use super::*;
use infiltrator_bevy_ui::pages::rules_draft::RulesDraftState;

#[test]
fn test_rules_toggle_and_reorder_submit_shared_intents() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);
    seed_rule_draft(&mut app);

    // DUAL-11-09: row #1 is disabled in the demo fixture; the toggle submits the
    // shared index intent.
    let id = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .row_id(1)
        .unwrap();
    let toggle = app
        .world_mut()
        .query::<(Entity, &RuleToggleButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == Some(id))
        .map(|(entity, _)| entity)
        .expect("row #1 toggle");
    activate(&mut app, toggle);
    assert!(app.world().resource::<RulesDraftState>().model.draft[1].enabled);
    assert!(sink.submitted().is_empty());

    // DUAL-11-10: the reorder handles map to the shared move directions.
    let id = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .row_id(3)
        .unwrap();
    let up = app
        .world_mut()
        .query::<(Entity, &RuleMoveUpButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == Some(id))
        .map(|(entity, _)| entity)
        .expect("row #3 move up");
    activate(&mut app, up);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft[2].rule,
        "MATCH,DIRECT"
    );
    assert!(sink.submitted().is_empty());

    let id = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .row_id(0)
        .unwrap();
    let down = app
        .world_mut()
        .query::<(Entity, &RuleMoveDownButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == Some(id))
        .map(|(entity, _)| entity)
        .expect("row #0 move down");
    activate(&mut app, down);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft[0].rule,
        "DOMAIN-KEYWORD,tracker,REJECT"
    );
    assert!(sink.submitted().is_empty());
}
