//! test-intent: behavior
//! Locale replay preserves OS identities; actual SDK policy clicks submit canonical intent.
use crate::native_input::click_entity;
use crate::pages_matrix_b_tests::setup_matrix_b_app;
use bevy::ecs::entity::Entity;
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::routing_projection::{process_copy, routing_summary};
use infiltrator_bevy_ui::command::{DemoCommandSink, UiCommand};
use infiltrator_bevy_ui::pages::app_routing::{
    AppNameText, AppProcessText, AppRoutingLine, AppRoutingProjection, AppRoutingProjectionUpdated,
    AppRuleText, SwitchAppRuleButton,
};
use infiltrator_bevy_ui::route::{Route, RouteChanged};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_domain::app_routing::AppRoutingMode;
use std::sync::Arc;

#[test]
fn native_routing_copy_matches_domain_policy_and_keeps_raw_identity_and_command_effects_separate() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_b_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    app.world_mut().trigger(RouteChanged(Route::AppRouting));
    app.update();
    let mut facts = AppRoutingProjection::demo();
    facts.mode = AppRoutingMode::BypassSelected;
    facts.apps[0].name = "OS 应用 {count}".into();
    facts.apps[0].process_name = "process-{mode}-{count}".into();
    app.world_mut()
        .trigger(AppRoutingProjectionUpdated(facts.clone()));
    app.update();
    let rows = app
        .world_mut()
        .query::<(
            Entity,
            Option<&AppRoutingLine>,
            Option<&AppNameText>,
            Option<&AppProcessText>,
            Option<&AppRuleText>,
        )>()
        .iter(app.world())
        .filter_map(|(entity, summary, name, process, rule)| {
            summary
                .map(|_| (entity, 0))
                .or_else(|| name.filter(|marker| marker.0 == 0).map(|_| (entity, 1)))
                .or_else(|| process.filter(|marker| marker.0 == 0).map(|_| (entity, 2)))
                .or_else(|| rule.filter(|marker| marker.0 == 0).map(|_| (entity, 3)))
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), 4);
    for language in ["en-US", "zh-CN", "en-US"] {
        app.insert_resource(UiLocale::new(language));
        app.update();
        for (entity, kind) in &rows {
            let expected = match kind {
                0 => routing_summary(facts.mode, 6, language),
                1 => "OS 应用 {count}".into(),
                2 => process_copy("process-{mode}-{count}", language),
                _ => {
                    if language == "en-US" {
                        "Proxy".into()
                    } else {
                        "代理 (Proxy)".into()
                    }
                }
            };
            assert_eq!(app.world().get::<Text>(*entity).unwrap().0, expected);
        }
        assert!(
            sink.submitted().is_empty(),
            "changing labels never saves routing policy"
        );
    }
    let button = app
        .world_mut()
        .query::<(Entity, &SwitchAppRuleButton)>()
        .iter(app.world())
        .find(|(_, button)| button.app_id == "app-1")
        .unwrap()
        .0;
    click_entity(&mut app, button);
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SetAppRule {
            app_id: "app-1".into(),
            rule: "Direct".into()
        }]
    );
    let policy = rows.iter().find(|(_, kind)| *kind == 3).unwrap().0;
    assert_eq!(
        app.world().get::<Text>(policy).unwrap().0,
        "Proxy",
        "submitting an intent cannot fabricate an observed policy change"
    );
}
