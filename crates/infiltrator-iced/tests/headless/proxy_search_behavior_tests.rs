//! test-intent: behavior
//! Native proxy search submits the typed owner preference, narrows and highlights
//! the published window, keeps the last good observation when the owner refuses,
//! retries, and re-applies the active query after a refresh.
use crate::command_harness::recording_application;
use crate::outbound_fixtures::{OutboundFixture, OutboundFixtureInput, standard_members};
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_iced::state::AppState;
use infiltrator_iced::state::proxy_search::ProxySearchPhase;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreScope;
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::preferences::{
    OutboundHealthFilter, OutboundLayout, OutboundSortOrder, OutboundUiPreferenceIntent,
    OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
};
use lurker_core_contract::page::PageData;
use std::sync::Arc;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("native search command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("native search must deliver a terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}

fn visible_members(state: &AppState) -> Vec<String> {
    state
        .runtime
        .filtered_groups
        .iter()
        .flat_map(|(_, members)| members.iter().cloned())
        .collect()
}

fn confirmed_preferences(
    scope: &CoreScope,
    revision: u64,
    query: &str,
) -> OutboundUiPreferencesSnapshot {
    OutboundUiPreferencesSnapshot::new(
        scope.clone(),
        SnapshotRevision::new(revision),
        OutboundUiPreferencesValues {
            search_query: query.to_owned(),
            favorite_outbound_ids: Vec::new(),
            group_order: Vec::new(),
            collapsed_group_ids: Vec::new(),
            sort_order: OutboundSortOrder::default(),
            health_filter: OutboundHealthFilter::default(),
            layout: OutboundLayout::default(),
        },
    )
    .expect("confirmed owner preference readback")
}

#[test]
fn native_search_submits_the_typed_owner_preference_and_recovers_from_a_refused_write() {
    let (mut state, _) = AppState::new();
    state.shell.current_route = Route::Proxies;
    let (application, handler) = recording_application();
    let core = application.snapshot();
    let scope = core.scope().expect("identified owner scope").clone();
    let members = standard_members();
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 51,
        members: &members,
        selected: None,
        favorites: &[],
        health_filter: OutboundHealthFilter::default(),
        sort_order: OutboundSortOrder::default(),
        layout: OutboundLayout::default(),
    });
    fixture.attach_to(&mut state, &application);
    let baseline = visible_members(&state);
    assert!(
        baseline.len() >= 3,
        "the published window is the search domain"
    );

    // success: the typed preference reaches the owner and the narrowed window is published.
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            confirmed_preferences(&scope, 52, "香港"),
        ));
    let task = state.update(Message::FilterProxies("香港".into()));
    assert!(matches!(
        state.runtime.proxy_search.phase,
        ProxySearchPhase::Pending { .. }
    ));
    let message = terminal(task);
    let intents = handler.0.lock().unwrap().clone();
    let applied = intents
        .iter()
        .find_map(|intent| match intent {
            CommandIntent::ApplyOutboundUiPreference { request } => Some(request.clone()),
            _ => None,
        })
        .expect("native search submits the typed owner preference");
    assert_eq!(
        applied.intent(),
        &OutboundUiPreferenceIntent::SetSearchQuery("香港".into())
    );
    let Message::OutboundUiPreferenceFinished { result, .. } = &message else {
        panic!("search must finish with the typed preference readback")
    };
    assert!(result.is_ok(), "the owner confirmed the search preference");
    let _ = state.update(message);
    assert!(matches!(
        state.runtime.proxy_search.phase,
        ProxySearchPhase::Synchronized { .. }
    ));
    let narrowed = visible_members(&state);
    assert!(narrowed.len() < baseline.len());
    assert_eq!(narrowed, vec!["hong-kong".to_owned()]);
    let runs = state
        .runtime
        .proxy_name_runs
        .get("hong-kong")
        .expect("the matched member publishes its text runs");
    assert!(
        runs.iter()
            .any(|run| run.highlighted && run.text.contains("香港")),
        "the matching member is highlighted"
    );

    // empty: an unmatched query publishes an honest empty window.
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            confirmed_preferences(&scope, 53, "no-such-node"),
        ));
    let message = terminal(state.update(Message::FilterProxies("no-such-node".into())));
    let _ = state.update(message);
    assert!(visible_members(&state).is_empty());

    // clear: an empty query restores the published window.
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            confirmed_preferences(&scope, 54, ""),
        ));
    let message = terminal(state.update(Message::FilterProxies(String::new())));
    let _ = state.update(message);
    assert_eq!(visible_members(&state).len(), baseline.len());

    // error: a refused write keeps the last good observation and surfaces the typed failure.
    *handler.1.lock().unwrap() = Some(Failure::new(
        ErrorCode::Permission,
        "search preference denied by the owner",
        false,
    ));
    let message = terminal(state.update(Message::FilterProxies("香港".into())));
    let Message::OutboundUiPreferenceFinished { result, .. } = &message else {
        panic!("a refused search must finish with the typed failure")
    };
    assert!(result.is_err());
    let _ = state.update(message);
    assert!(matches!(
        state.runtime.proxy_search.phase,
        ProxySearchPhase::Failed { .. }
    ));
    assert!(
        state
            .runtime
            .proxies_model
            .preference_write_phase()
            .failure()
            .is_some()
    );
    let owner_query = state
        .runtime
        .outbound_ui_preferences
        .data
        .as_ref()
        .map(|preferences| preferences.search_query.clone())
        .unwrap_or_default();
    assert_eq!(
        owner_query, "",
        "a refused search leaves the confirmed owner preference unchanged"
    );
    assert_eq!(
        state.runtime.proxy_filter, "香港",
        "the refused draft is kept so the user can retry it"
    );

    // retry: the same query succeeds after the owner accepts the write again.
    *handler.1.lock().unwrap() = None;
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            confirmed_preferences(&scope, 55, "香港"),
        ));
    let message = terminal(state.update(Message::FilterProxies("香港".into())));
    let _ = state.update(message);
    assert!(matches!(
        state.runtime.proxy_search.phase,
        ProxySearchPhase::Synchronized { .. }
    ));
    let retried = visible_members(&state);
    assert_eq!(retried, vec!["hong-kong".to_owned()]);

    // refresh: a newer published window keeps the active query applied.
    let mut surface = state.surface.latest().cloned().expect("owner surface");
    surface.revision = state.surface.revision() + 1;
    surface.outbound_inventory = PageData::ready(fixture.inventory.clone());
    surface.outbound_details = PageData::ready(fixture.details.clone());
    surface.outbound_ui_preferences = PageData::ready(confirmed_preferences(&scope, 56, "香港"));
    assert!(state.apply_shared_surface_snapshot(surface));
    let refreshed = visible_members(&state);
    assert_eq!(refreshed, vec!["hong-kong".to_owned()]);

    // the recorded intents prove the surface never wrote the owner state directly.
    let recorded = handler.0.lock().unwrap().clone();
    assert!(
        recorded
            .iter()
            .all(|intent| matches!(intent, CommandIntent::ApplyOutboundUiPreference { .. })),
        "native search only submits the typed owner preference"
    );
    let _ = Arc::new(());
}
