//! Behavior cases for rules game.
//! test-intent: behavior

use super::rules_fixture::seed_rule_draft;
use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::rules_draft::RulesDraftState;
use infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET;

#[test]
fn test_rules_game_presets_submit_shared_target() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);
    seed_rule_draft(&mut app);

    // DUAL-11-12: the target field defaults to the shared constant; the inject
    // button forwards whatever the field holds.
    let target = target_field_entity(&mut app);
    assert_eq!(
        app.world()
            .get::<TextField>(target)
            .expect("target")
            .0
            .text(),
        DEFAULT_RULE_TARGET
    );
    app.world_mut()
        .get_mut::<TextField>(target)
        .expect("target")
        .0 = TextFieldState::new("Game-Proxy");

    let inject = app
        .world_mut()
        .query_filtered::<Entity, With<InjectGamePresetsButton>>()
        .single(app.world())
        .expect("inject button");
    activate(&mut app, inject);
    let rules = &app.world().resource::<RulesDraftState>().model.draft;
    assert!(rules.len() > 4);
    assert!(rules[0].rule.contains("Game-Proxy"));
    assert!(sink.submitted().is_empty());

    // The wizard caption is a real i18n-free bare-Chinese label on Bevy.
    let caption = app
        .world_mut()
        .query::<(&Text, &RuleBuilderSelection)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .next()
        .expect("selection caption");
    assert!(caption.contains("已选类型"));
}
