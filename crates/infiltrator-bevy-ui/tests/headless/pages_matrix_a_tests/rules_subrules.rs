//! Behavior cases for rules subrules.
//! test-intent: behavior

use super::rules_fixture::seed_rule_draft;
use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::rules_draft::RulesDraftState;
use infiltrator_domain::rules::logical::{LOGICAL_OPERATOR_CHOICES, SUB_RULE_CONDITION_PRESETS};

/// DUAL-11-02: the Bevy logical builder delegates every mutation to the shared
/// draft and submits the shared canonical expression through the command bus.
#[test]
fn test_rules_subrules_builder_submits_shared_logical_intent() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Rules);
    seed_rule_draft(&mut app);

    // A preset condition is appended by the shared reduction; the rebuilt list
    // shows the new row and the preview reflects the canonical expression.
    let preset = app
        .world_mut()
        .query_filtered::<Entity, With<SubRulePresetButton>>()
        .iter(app.world())
        .next()
        .expect("preset button");
    activate(&mut app, preset);
    let draft = app.world().resource::<RulesSubRuleState>().draft.clone();
    assert_eq!(draft.conditions.len(), 3);
    assert_eq!(draft.conditions[2], SUB_RULE_CONDITION_PRESETS[0]);
    assert_eq!(count_with::<SubRuleConditionRow>(&mut app), 3);

    // Selecting an operator goes through the shared vocabulary.
    let not_chip = app
        .world_mut()
        .query::<(Entity, &SubRuleOperatorChip)>()
        .iter(app.world())
        .find(|(_, chip)| {
            LOGICAL_OPERATOR_CHOICES
                .get(chip.0)
                .is_some_and(|operator| *operator == "NOT")
        })
        .map(|(entity, _)| entity)
        .expect("NOT chip");
    activate(&mut app, not_chip);
    assert_eq!(
        app.world().resource::<RulesSubRuleState>().draft.operator,
        "NOT"
    );

    // NOT accepts exactly one condition, so the insert is gated by the shared
    // builder and nothing is submitted while the draft is invalid.
    let insert = app
        .world_mut()
        .query_filtered::<Entity, With<SubRuleInsertButton>>()
        .iter(app.world())
        .next()
        .expect("insert button");
    activate(&mut app, insert);
    assert!(sink.submitted().is_empty());
    assert!(subtree_has_text(app.world(), root, "校验未通过"));

    // Removing conditions down to one lets the shared builder build.
    while app
        .world()
        .resource::<RulesSubRuleState>()
        .draft
        .conditions
        .len()
        > 1
    {
        let remove = app
            .world_mut()
            .query_filtered::<Entity, With<SubRuleRemoveConditionButton>>()
            .iter(app.world())
            .next()
            .expect("remove button");
        activate(&mut app, remove);
        app.update();
    }

    // The target field is the live source of the composition target.
    let target = app
        .world_mut()
        .query_filtered::<&Children, With<SubRuleTargetField>>()
        .single(app.world())
        .expect("target wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("target text field");
    app.world_mut()
        .get_mut::<TextField>(target)
        .expect("target field")
        .0 = TextFieldState::new("AI");

    activate(&mut app, insert);
    app.update();
    let inserted = &app.world().resource::<RulesDraftState>().model.draft[0];
    assert_eq!(inserted.rule, "NOT,((DOMAIN-SUFFIX,google.com)),AI");
    assert!(app.world().resource::<RulesDraftState>().model.dirty());
    assert!(sink.submitted().is_empty());
}
