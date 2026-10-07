//! Behavior cases for rules search.
//! test-intent: behavior

use super::*;
use bevy::ecs::query::With;

#[test]
fn test_rules_search_hides_non_matching_rows() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    navigate_to(&mut app, Route::Rules);

    // DUAL-11-08/13: a 1,000-rule projection only ever mounts the window; the
    // keyword filter mounts only the rows that match it.
    app.world_mut()
        .commands()
        .trigger(RulesProjectionUpdated(rules_projection_with(1_000)));
    app.update();
    let window = app.world().resource::<RulesViewState>().rendered_rows;
    assert!(window > 0 && window < 1_000);
    assert_eq!(mounted_rule_rows(&mut app).len(), window);

    let field_entity = rules_search_field(&mut app);
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("host-42.example");
    app.update();

    // The matching row is mounted; nothing else is. The demo row #1 is not
    // part of this projection at all.
    let rows = mounted_rule_rows(&mut app);
    assert_eq!(rows, vec![42]);
    let container = app
        .world_mut()
        .query_filtered::<Entity, With<RulesWindowRows>>()
        .single(app.world())
        .expect("window rows container");
    assert!(subtree_has_text(app.world(), container, "host-42.example"));

    // A keyword that matches nothing mounts no rows and says so.
    app.world_mut()
        .get_mut::<TextField>(field_entity)
        .expect("text field")
        .0 = TextFieldState::new("no-such-host");
    app.update();
    assert!(mounted_rule_rows(&mut app).is_empty());
}
