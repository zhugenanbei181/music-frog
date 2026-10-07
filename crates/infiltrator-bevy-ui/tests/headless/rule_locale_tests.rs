//! test-intent: behavior
//! Shared facts replay in place during actual native editing and IME preedit.
use crate::native_input::{press, type_text};
use crate::rule_list_action_tests::setup;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ui::widget::Text;
use bevy::window::{Ime, PrimaryWindow};
use infiltrator_application::rule_mrs_projection::mrs_acceleration_status_line;
use infiltrator_application::rule_provider_projection::{
    default_action, provider_cache_line, rules_summary,
};
use infiltrator_bevy_ui::pages::rules::LastRulesProjection;
use infiltrator_bevy_ui::pages::rules_builder_input::{RuleBuilderField, RuleBuilderFieldKind};
use infiltrator_bevy_ui::pages::rules_mrs::{MrsStatusText, ProviderCacheText};
use infiltrator_bevy_ui::pages::rules_projection::{RulesLine, RulesLineKind};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_shared::locales::{Lang, Localizer};
use std::sync::atomic::Ordering;

#[test]
fn native_rule_facts_locale_replay_preserves_fact_entities_input_selection_ime_and_zero_writes() {
    let (mut app, _, store) = setup();
    let input = app
        .world_mut()
        .query::<(Entity, &RuleBuilderField)>()
        .iter(app.world())
        .find(|(_, field)| field.kind == RuleBuilderFieldKind::Payload)
        .unwrap()
        .0;
    press(&mut app, input);
    type_text(&mut app, "unsent-{source}.test");
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "组合输入".into(),
        cursor: Some((0, 6)),
    });
    app.update();
    let input_before = app.world().get::<TextField>(input).unwrap().0.clone();
    let focus_before = app.world().get::<TextFieldFocused>(input).unwrap().0;
    let facts = app
        .world_mut()
        .query::<(
            Entity,
            Option<&RulesLine>,
            Option<&MrsStatusText>,
            Option<&ProviderCacheText>,
        )>()
        .iter(app.world())
        .filter_map(|(entity, line, mrs, cache)| {
            line.map(|line| (entity, Some(line.0), false, false))
                .or_else(|| mrs.map(|_| (entity, None, true, false)))
                .or_else(|| cache.map(|_| (entity, None, false, true)))
        })
        .collect::<Vec<_>>();
    assert!(!facts.is_empty());
    let projection = app
        .world()
        .resource::<LastRulesProjection>()
        .0
        .clone()
        .unwrap();
    for language in ["en-US", "zh-CN", "en-US"] {
        app.insert_resource(UiLocale::new(language));
        app.update();
        for (entity, line, mrs, cache) in &facts {
            let expected = match line {
                Some(RulesLineKind::Summary) => Some(rules_summary(
                    projection.total_rules,
                    projection.providers.len(),
                    language,
                )),
                Some(RulesLineKind::DefaultAction) => {
                    Some(default_action(&projection.default_action, language))
                }
                _ if *mrs => Some(mrs_acceleration_status_line(
                    &Lang(language),
                    &projection.mrs_acceleration,
                )),
                _ if *cache => Some(provider_cache_line(
                    &projection.provider_cache,
                    &Lang(language),
                )),
                _ => None,
            };
            if let Some(expected) = expected {
                assert_eq!(app.world().get::<Text>(*entity).unwrap().0, expected);
            }
        }
        let mut expected_input = input_before.clone();
        expected_input.set_placeholder(Lang(language).tr("field_rule_match").into_owned());
        assert_eq!(
            app.world().get::<TextField>(input).unwrap().0,
            expected_input
        );
        assert_eq!(
            app.world().get::<TextFieldFocused>(input).unwrap().0,
            focus_before
        );
        assert_eq!(
            app.world().resource::<LastRulesProjection>().0.as_ref(),
            Some(&projection)
        );
        assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    }
}
