//! test-intent: behavior
//! The demo performs the business's own gesture and the surface state proves it.
//!
//! Every test here drives the real demo activation (the same dispatch the
//! capture pipeline uses) and then reads the real surface state. A demo that
//! only repainted its page could not satisfy any of these assertions.
use super::*;
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use futures_util::StreamExt;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_presentation::traffic_readout::TrafficFreshness;
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::routing_intent::RoutingIntent;
use std::collections::VecDeque;
use tokio::runtime::Builder;

fn demo_env(page: Route, scenario: FeatureId) -> DemoEnv {
    DemoEnv {
        enabled: true,
        page,
        pane: EditorPane::Profile,
        providers_tab: false,
        lang: "zh-CN".to_string(),
        skin: iced::Theme::Dark,
        window_size: DEFAULT_WINDOW,
        capture_marker: None,
        scenario: Some(scenario),
    }
}

/// Drive the demo activation's task to the end, feeding every message the
/// surface produces back into the real update pipeline.
fn drain(state: &mut AppState, task: Task<Message>) {
    let mut pending = VecDeque::from([task]);
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            while let Some(task) = pending.pop_front() {
                let Some(mut stream) = into_stream(task) else {
                    continue;
                };
                while let Some(action) = stream.next().await {
                    if let Action::Output(message) = action {
                        pending.push_back(update(state, message));
                    }
                }
            }
        });
}

#[test]
fn the_proxy_search_narrows_the_shared_list_and_highlights_the_match() {
    let env = demo_env(Route::Proxies, FeatureId::ProxiesSearchHighlight);
    let (state, _task) = AppState::demo(&env);
    assert_eq!(state.runtime.proxy_filter, "HK");
    let (group, members) = state
        .runtime
        .filtered_groups
        .first()
        .expect("the search keeps the matching group");
    assert_eq!(group, "PROXIES");
    assert_eq!(members.len(), 2);
    assert!(
        state
            .runtime
            .proxy_name_runs
            .get("HK-01")
            .is_some_and(|runs| runs.iter().any(|run| run.highlighted && run.text == "HK"))
    );
}

#[test]
fn the_proxy_group_starts_folded_and_the_demo_opens_it() {
    // The fixture folds the first group, so the fold business has something
    // to open. The fold is the shared proxies model's fact, so the fixture is
    // read before the capture opens it.
    let mut fixture_env = demo_env(Route::Proxies, FeatureId::ProxiesSearchHighlight);
    fixture_env.scenario = None;
    let (fixture, _task) = AppState::demo(&fixture_env);
    let group_id = fixture
        .runtime
        .proxy_groups
        .iter()
        .find(|group| group.name == "PROXIES")
        .and_then(|group| group.id.clone())
        .expect("typed demo collection");
    assert!(
        !fixture.runtime.proxies_model.group_expanded(&group_id),
        "the demo fixture folds the first group so the fold has something to open"
    );

    let (opened, _task) =
        AppState::demo(&demo_env(Route::Proxies, FeatureId::ProxiesGroupExpanded));
    assert!(
        opened.runtime.proxies_model.group_expanded(&group_id),
        "the demo opened the group the fold business is about"
    );
}

#[test]
fn the_overview_card_order_moves_one_real_card() {
    let env = demo_env(Route::Overview, FeatureId::OverviewCardReorder);
    let (state, _task) = AppState::demo(&env);
    let order = &state.diag.overview_card_order;
    assert_eq!(order.first(), Some(&OverviewCardKind::Traffic));
    assert_ne!(order, &OverviewCardKind::DEFAULT_ORDER.to_vec());
}

#[test]
fn the_rule_search_narrows_the_shared_rule_list() {
    let env = demo_env(Route::Rules, FeatureId::RulesSearchHighlight);
    let (state, _task) = AppState::demo(&env);
    assert_eq!(state.editor.rules_filter, "google.com");
    assert_eq!(state.editor.rules_filtered_indices.len(), 1);
    assert!(state.editor.rule_list.draft.len() > 1);
}

#[test]
fn the_aggregation_wizard_opens_on_the_profiles_page() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesAggregatorWizard);
    let (state, _task) = AppState::demo(&env);
    assert!(state.profile.aggregator_modal_open);
}

#[test]
fn the_profile_delete_asks_for_the_confirmation_first() {
    let env = demo_env(Route::Profiles, FeatureId::ProfilesDeleteConfirm);
    let (state, _task) = AppState::demo(&env);
    assert!(matches!(
        state.shell.confirmation,
        Some(ConfirmAction::DeleteProfile(_))
    ));
}

#[test]
fn the_routing_intent_capture_lands_the_owner_network_refusal() {
    let env = demo_env(Route::Overview, FeatureId::ShellRoutingIntentControl);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let actions = &state.runtime.routing_intent_actions;
    assert!(
        actions
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Network)
    );
    assert_eq!(
        actions.retry_target(),
        Some(RoutingIntent::RouteAllThroughProxy)
    );
    assert!(actions.pending_change().is_none());
    assert_eq!(
        actions.render_snapshot().current,
        Some(RoutingIntent::FollowRules)
    );
}

#[test]
fn the_routing_intent_capture_keeps_the_owner_authentication_refusal() {
    let env = demo_env(Route::Overview, FeatureId::ShellRoutingIntentAuthentication);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let actions = &state.runtime.routing_intent_actions;
    assert!(
        actions
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Authentication)
    );
    assert!(actions.needs_controller_settings());
    assert!(actions.retry_target().is_none());
}

#[test]
fn the_telemetry_capture_observes_the_stale_zero_readback() {
    let env = demo_env(Route::Runtime, FeatureId::RuntimeTelemetryObservation);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    // The stale zero readback is a typed fact; only its copy is localised, so
    // the assertion never compares a copy.
    let readout = state.traffic_readout();
    assert_eq!(readout.freshness, TrafficFreshness::Failed);
    assert!(matches!(
        state.shell.readout.upload_bps,
        ShellObservation::Stale(rate) | ShellObservation::Current(rate) if rate == 0.0
    ));
    assert!(
        state
            .shell
            .readout
            .rate_failure
            .as_ref()
            .is_some_and(|failure| failure.message.contains("isolated telemetry read denied"))
    );
}

#[test]
fn the_hosts_capture_lands_the_owner_permission_refusal() {
    let env = demo_env(Route::Dns, FeatureId::DnsHostsEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let editor = &state.editor.dns_hosts_editor;
    assert!(editor.is_open());
    assert!(!editor.is_pending());
    assert!(
        editor
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    );
    assert!(editor.is_importing_legacy());
    assert_eq!(editor.rows.len(), 3);
}

#[test]
fn the_query_capture_lands_the_owner_answer() {
    let env = demo_env(Route::Dns, FeatureId::DnsQueryDetails);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let model = &state.diag.dns_query;
    assert!(model.is_open());
    assert!(!model.is_pending());
    assert!(model.current_failure().is_none());
    assert_eq!(model.requested_operation(), model.snapshot.report_id);
    assert!(
        model
            .snapshot
            .report
            .as_ref()
            .is_some_and(|report| report.response.answers.len() == 2)
    );
}

#[test]
fn the_tracer_capture_lands_the_owner_decision_chain() {
    let env = demo_env(Route::Rules, FeatureId::RulesTracerDrawer);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let model = &state.editor.rule_trace;
    assert_eq!(state.editor.rules_model.tab(), RulesTab::Tracer);
    assert!(!model.busy());
    assert!(model.current_failure().is_none());
    assert!(model.snapshot.report.as_ref().is_some_and(|report| {
        report
            .decision_chain
            .as_ref()
            .is_some_and(|chain| chain.matched_rule_raw == "DOMAIN-SUFFIX,google.com,PROXY")
    }));
}

#[test]
fn the_rule_list_capture_lands_the_owner_commit_refusal() {
    let env = demo_env(Route::Rules, FeatureId::RulesListEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let model = &state.editor.rule_list;
    assert_eq!(state.editor.rules_model.tab(), RulesTab::List);
    assert!(model.dirty());
    assert!(model.can_save());
    assert!(!model.is_pending());
    assert!(
        model
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    );
}

#[test]
fn the_filter_capture_lands_the_owner_permission_refusal() {
    let env = demo_env(Route::Editor, FeatureId::ProfilesFilterEditor);
    let (mut state, task) = AppState::demo(&env);
    drain(&mut state, task);
    let editor = &state.editor.filter_editor;
    assert!(editor.pending().is_none());
    assert!(
        editor
            .failure()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    );
}
