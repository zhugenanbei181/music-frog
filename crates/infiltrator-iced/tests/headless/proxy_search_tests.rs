//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::project_groups_snapshot;
use infiltrator_application::proxy_search_projection::project_name_runs;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, ProxiesPageSnapshot, SurfaceSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use std::sync::Arc;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("real search command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal search result")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn publish(state: &mut AppState, preferences: &ProxyPreferencesApplication) {
    let prefs = preferences
        .preferences()
        .expect("shared preference state available");
    let (groups, filter_alive) = project_groups_snapshot(&state.runtime.proxies, &prefs);
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("irrelevant host capability"),
    );
    snapshot.revision = state.surface.revision() + 1;
    snapshot.pages.proxies = PageData::ready(ProxiesPageSnapshot {
        name_runs: project_name_runs(&groups, &prefs.search_query),
        search_query: prefs.search_query,
        node_details: vec![],
        groups,
        filter_alive,
        testing: false,
        active_exit: String::new(),
        sort_order: prefs.sort_order,
        compact_view: prefs.compact_view,
        custom_node: Default::default(),
    });
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn size(tree: &Tree) -> usize {
    1 + tree.children.iter().map(size).sum::<usize>()
}

#[test]
fn native_search_submits_to_the_shared_owner_replays_highlights_and_clears_without_changing_selection()
 {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    // Explicit user-provided Unicode names remain raw data and exercise native highlight replay.
    for name in ["HK-01", "HK-02"] {
        let Some(Proxy::Shadowsocks(mut node)) = state.runtime.proxies.remove(name) else {
            panic!("fixture Shadowsocks node")
        };
        node.base.name = format!("香港 {name}");
        state
            .runtime
            .proxies
            .insert(node.base.name.clone(), Proxy::Shadowsocks(node));
    }
    for proxy in state.runtime.proxies.values_mut() {
        if let Proxy::Selector(group) | Proxy::URLTest(group) = proxy {
            for name in &mut group.all {
                if name.starts_with("HK-") {
                    *name = format!("香港 {name}");
                }
            }
            if group.now.starts_with("HK-") {
                group.now = format!("香港 {}", group.now);
            }
        }
    }
    let raw = state.runtime.proxies.clone();
    let preferences = ProxyPreferencesApplication::new();
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    state.commands = Some(application);
    state.shell.demo = false;
    publish(&mut state, &preferences);
    let initial: usize = state
        .runtime
        .proxy_groups
        .iter()
        .map(|group| group.proxies.len())
        .sum();
    let original_tree = size(&Tree::new(state.view().as_widget()));
    for query in ["香港", "missing-node", ""] {
        let task = state.update(Message::FilterProxies(query.into()));
        let completion = terminal(task);
        assert_eq!(state.update(completion).units(), 0);
        assert_eq!(
            preferences
                .preferences()
                .expect("shared preference state available")
                .search_query,
            query
        );
        publish(&mut state, &preferences);
        assert_eq!(state.runtime.proxy_filter, query);
        let count: usize = state
            .runtime
            .proxy_groups
            .iter()
            .map(|group| group.proxies.len())
            .sum();
        match query {
            "香港" => {
                assert!(count > 0 && count < initial);
                assert!(
                    state
                        .runtime
                        .proxy_name_runs
                        .values()
                        .flatten()
                        .any(|run| run.highlighted && run.text == "香港")
                );
                assert!(size(&Tree::new(state.view().as_widget())) < original_tree);
                let mut refreshed = state.surface.latest().unwrap().clone();
                refreshed.revision += 1;
                let node = refreshed
                    .pages
                    .proxies
                    .data
                    .as_mut()
                    .unwrap()
                    .groups
                    .iter_mut()
                    .flat_map(|group| &mut group.proxies)
                    .next()
                    .expect("matching observed node");
                let name = node.name.clone();
                node.delay_ms = Some(142);
                assert!(state.apply_shared_surface_snapshot(refreshed));
                assert_eq!(
                    state
                        .runtime
                        .proxy_groups
                        .iter()
                        .flat_map(|group| &group.proxies)
                        .find(|node| node.name == name)
                        .unwrap()
                        .delay_ms,
                    Some(142)
                );
                assert_eq!(state.runtime.proxy_filter, query);
            }
            "missing-node" => assert_eq!(count, 0),
            _ => assert_eq!(count, initial),
        }
        assert_eq!(state.runtime.proxies, raw);
    }
}

#[test]
fn rejected_search_keeps_the_draft_and_stale_completion_cannot_overwrite_a_new_request() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let (application, handler) = recording_application();
    state.commands = Some(application);
    let failure = Failure::new(ErrorCode::Permission, "search denied", false);
    *handler.1.lock().unwrap() = Some(failure.clone());
    let task = state.update(Message::FilterProxies("hk".into()));
    let completion = terminal(task);
    let _ = state.update(completion);
    assert_eq!(state.runtime.proxy_search.failure, Some(failure));
    assert_eq!(state.runtime.proxy_filter, "hk");
    *handler.1.lock().unwrap() = None;
    let old = state.update(Message::FilterProxies("old".into()));
    assert_eq!(
        state.update(Message::FilterProxies("new".into())).units(),
        0
    );
    let completion = terminal(old);
    let current = state.update(completion);
    assert!(state.runtime.proxy_search.pending.is_some());
    assert_eq!(state.runtime.proxy_filter, "new");
    let _ = state.update(Message::ProxySearchFinished {
        token: 0,
        query: "old".into(),
        result: Err(Failure::new(ErrorCode::NotReady, "stale", true)),
    });
    assert!(state.runtime.proxy_search.failure.is_none());
    let completion = terminal(current);
    let _ = state.update(completion);
    assert!(state.runtime.proxy_search.pending.is_none());
    assert!(state.runtime.proxy_search.failure.is_none());
}

#[test]
fn composed_group_and_preference_controls_execute_the_shared_owner_and_render_only_observed_changes()
 {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let preferences = ProxyPreferencesApplication::new();
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    state.commands = Some(application);
    state.shell.demo = false;
    publish(&mut state, &preferences);
    let group = state.runtime.proxy_groups[0].name.clone();
    let original = size(&Tree::new(state.view().as_widget()));
    for expanded in [false, true] {
        let previous = state.runtime.proxy_groups[0].expanded;
        let task = state.update(Message::ToggleProxyGroupExpanded(group.clone()));
        assert_eq!(state.runtime.proxy_groups[0].expanded, previous);
        let completion = terminal(task);
        assert_eq!(state.update(completion).units(), 0);
        assert_eq!(preferences.is_group_expanded(&group).unwrap(), expanded);
        publish(&mut state, &preferences);
        assert_eq!(state.runtime.proxy_groups[0].expanded, expanded);
        let rendered = size(&Tree::new(state.view().as_widget()));
        if expanded {
            assert_eq!(rendered, original);
        } else {
            assert!(rendered < original);
        }
    }
    let alive = state.update(Message::ToggleFilterAlive(true));
    assert!(!state.runtime.filter_alive_only);
    assert_eq!(
        state.update(Message::ToggleProxyCompactView).units(),
        0,
        "a second preference waits for the first real completion"
    );
    let completion = terminal(alive);
    let compact = state.update(completion);
    assert!(preferences.filter_alive().unwrap());
    assert!(state.runtime.proxy_preferences.pending.is_some());
    let completion = terminal(compact);
    let _ = state.update(completion);
    assert!(preferences.compact_view().unwrap());
    publish(&mut state, &preferences);
    assert!(state.runtime.filter_alive_only);
    assert!(state.runtime.proxy_compact_view);
    assert!(state.runtime.proxy_preferences.pending.is_none());
}
