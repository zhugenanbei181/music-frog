//! Behavior cases for rules add.
//! test-intent: behavior

use super::rules_fixture::seed_rule_draft;
use super::*;
use bevy::ecs::query::With;
use infiltrator_bevy_ui::pages::rules_draft::RulesDraftState;
use infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES;

#[test]
fn test_rules_add_wizard_submits_shared_draft_and_type_selection() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    navigate_to(&mut app, Route::Rules);
    seed_rule_draft(&mut app);

    // DUAL-11-11: pick a non-default type chip; the shared state and caption
    // restamp together.
    let geoip_chip = app
        .world_mut()
        .query::<(Entity, &RuleTypeChip)>()
        .iter(app.world())
        .find(|(_, chip)| {
            CUSTOM_RULE_TYPE_CHOICES
                .get(chip.0)
                .is_some_and(|choice| *choice == "GEOIP")
        })
        .map(|(entity, _)| entity)
        .expect("GEOIP chip");
    activate(&mut app, geoip_chip);
    assert_eq!(
        app.world().resource::<RulesBuilderState>().rule_type,
        "GEOIP"
    );

    // Type the payload and target into the wizard fields.
    let set_field = |app: &mut App, wrapper: fn(&mut App) -> Entity, text: &str| {
        let field = wrapper(app);
        app.world_mut()
            .get_mut::<TextField>(field)
            .expect("wizard field")
            .0 = TextFieldState::new(text);
    };
    set_field(&mut app, payload_field_entity, "CN");
    set_field(&mut app, target_field_entity, "DIRECT");

    let add = app
        .world_mut()
        .query_filtered::<Entity, With<AddCustomRuleButton>>()
        .single(app.world())
        .expect("add button");
    activate(&mut app, add);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft[0].rule,
        "GEOIP,CN,DIRECT"
    );
    assert!(app.world().resource::<RulesDraftState>().model.dirty());
    assert!(sink.submitted().is_empty());
}
