//! Behavior cases for profiles filter.
//! test-intent: behavior

use super::*;
use crate::native_input::click_entity;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::profiles_filter_dedup::{FilterDedupChoice, ReloadFilterDedup};
use infiltrator_bevy_ui::pages::profiles_filter_form::FilterFormState;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::subscription_filter_result::SubscriptionFilterApplied;
use infiltrator_contract::subscription_import::SubscriptionFilterDedup;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;

fn complete_save(app: &mut App, sink: &DemoCommandSink) {
    let request_id = app
        .world()
        .resource::<FilterFormState>()
        .request
        .expect("tracked filter request")
        .0;
    let command = sink.submitted().last().cloned().expect("submitted filter");
    let UiCommand::SaveSubscriptionFilter { source, filter } = &command else {
        panic!("filter save");
    };
    let applied = SubscriptionFilterApplied {
        source: observation(&source.profile, FIXTURE_DOCUMENT, filter.clone())
            .unwrap()
            .source,
        report: Default::default(),
    };
    app.world_mut().trigger(CommandExecutedEvent {
        command,
        request_id,
        result: Ok(CommandOutput::SubscriptionFilterApplied(applied)),
    });
    app.update();
}

#[test]
fn test_profiles_filter_panel_restamps_and_submits_shared_command() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(Arc::clone(&sink));
    let (root, _) = navigate_to(&mut app, Route::Profiles);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(subscription_fetch_projection()));
    app.update();

    assert!(
        subtree_has_text(app.world(), root, "香港"),
        "stored include filter restamps onto the panel"
    );
    assert!(
        subtree_has_text(app.world(), root, "清洗管道"),
        "filter pipeline status line renders"
    );
    assert!(
        subtree_has_text(app.world(), root, "0 */6 * * *"),
        "cron schedule is visible on the import card (DUAL-07-03)"
    );

    set_marker_text::<SubscriptionFilterIncludeField>(&mut app, "香港, 日本");
    let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SaveSubscriptionFilter {
            source: subscription_fetch_projection().profiles[0]
                .filter_source
                .clone()
                .unwrap(),
            filter: SubscriptionFilterDraft {
                include: "香港, 日本".to_owned(),
                exclude: "广告".to_owned(),
                dedup_index: 0,
                ..Default::default()
            },
        }],
        "the filter editor rides the shared command (shared pipeline)"
    );
}

#[test]
fn native_filter_strategy_preserves_keep_last_and_submits_all_four_choices_without_early_writes() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Profiles);
    let mut projection = subscription_fetch_projection();
    projection
        .profiles
        .iter_mut()
        .find(|p| p.is_active)
        .unwrap()
        .filter
        .dedup_index = 2;
    refresh_filter_source(&mut projection);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
    click_entity(&mut app, save);
    let UiCommand::SaveSubscriptionFilter { filter, .. } = &sink.submitted()[0] else {
        panic!("filter save");
    };
    assert_eq!(
        filter.dedup_index, 2,
        "unmodified save never collapses KeepLast to KeepFirst"
    );
    complete_save(&mut app, &sink);
    let choices = app
        .world_mut()
        .query::<(Entity, &FilterDedupChoice)>()
        .iter(app.world())
        .map(|(entity, mode)| (entity, mode.0))
        .collect::<Vec<_>>();
    assert_eq!(choices.len(), SubscriptionFilterDedup::ALL.len());
    for mode in SubscriptionFilterDedup::ALL {
        sink.clear();
        let entity = choices
            .iter()
            .find(|(_, choice)| *choice == mode)
            .unwrap()
            .0;
        click_entity(&mut app, entity);
        assert!(
            sink.submitted().is_empty(),
            "selection is an uncommitted draft"
        );
        click_entity(&mut app, save);
        let submitted = sink.submitted();
        assert_eq!(submitted.len(), 1);
        let UiCommand::SaveSubscriptionFilter { filter, source } = &submitted[0] else {
            panic!("filter save");
        };
        assert_eq!(filter.dedup_index, mode.index());
        assert_eq!(source.profile, "sub-fetch");
        complete_save(&mut app, &sink);
    }
}

#[test]
fn native_changed_strategy_disables_save_preserves_choice_and_explicit_reload_recovers() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink.clone());
    app.add_plugins(ButtonPlugin);
    navigate_to(&mut app, Route::Profiles);
    let mut projection = subscription_fetch_projection();
    projection
        .profiles
        .iter_mut()
        .find(|p| p.is_active)
        .unwrap()
        .filter
        .dedup_index = 2;
    refresh_filter_source(&mut projection);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection.clone()));
    app.update();
    let choice = app
        .world_mut()
        .query::<(Entity, &FilterDedupChoice)>()
        .iter(app.world())
        .find(|(_, mode)| mode.0 == SubscriptionFilterDedup::AppendIndex)
        .unwrap()
        .0;
    click_entity(&mut app, choice);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection.clone()));
    app.update();
    let state = app.world().resource::<FilterFormState>();
    assert_eq!(
        state.editor.selected(),
        Some(SubscriptionFilterDedup::AppendIndex)
    );
    assert!(!state.editor.stale());
    projection
        .profiles
        .iter_mut()
        .find(|p| p.is_active)
        .unwrap()
        .filter
        .dedup_index = 1;
    refresh_filter_source(&mut projection);
    app.world_mut()
        .commands()
        .trigger(ProfilesProjectionUpdated(projection));
    app.update();
    let save = marker_entity::<SaveSubscriptionFilterButton>(&mut app);
    assert!(app.world().get::<InteractionDisabled>(save).is_some());
    click_entity(&mut app, save);
    assert!(sink.submitted().is_empty());
    let state = app.world().resource::<FilterFormState>();
    assert!(state.editor.stale());
    assert_eq!(
        state.editor.selected(),
        Some(SubscriptionFilterDedup::AppendIndex)
    );
    let reload = marker_entity::<ReloadFilterDedup>(&mut app);
    click_entity(&mut app, reload);
    assert!(app.world().get::<InteractionDisabled>(save).is_none());
    click_entity(&mut app, save);
    let UiCommand::SaveSubscriptionFilter { filter, .. } = &sink.submitted()[0] else {
        panic!("filter save");
    };
    assert_eq!(filter.dedup_index, 1);
}

fn refresh_filter_source(projection: &mut ProfilesProjection) {
    let profile = projection
        .profiles
        .iter_mut()
        .find(|profile| profile.is_active)
        .unwrap();
    profile.filter_source =
        observation(&profile.id, FIXTURE_DOCUMENT, profile.filter.clone()).map(|o| o.source);
}
