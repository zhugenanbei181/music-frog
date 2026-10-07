//! test-intent: behavior
use crate::command_harness::{recording_application, rejecting_application};
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::project_groups_snapshot;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, ProxiesPageSnapshot, SurfaceSnapshot};
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
            let mut stream = into_stream(task).expect("Apply submits a real order command");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal command result")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn publish(state: &mut AppState, preferences: &ProxyPreferencesApplication) {
    let prefs = preferences.preferences().unwrap();
    let (groups, filter_alive) = project_groups_snapshot(&state.runtime.proxies, &prefs);
    let mut snapshot = state.surface.latest().cloned().unwrap_or_else(|| {
        SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("unrelated host capability"),
        )
    });
    snapshot.revision = state.surface.revision() + 1;
    snapshot.pages.proxies = PageData::ready(ProxiesPageSnapshot {
        groups,
        filter_alive,
        name_runs: Default::default(),
        search_query: String::new(),
        node_details: vec![],
        testing: false,
        active_exit: String::new(),
        sort_order: prefs.sort_order,
        compact_view: prefs.compact_view,
        custom_node: Default::default(),
    });
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn tree_size(tree: &Tree) -> usize {
    1 + tree.children.iter().map(tree_size).sum::<usize>()
}

#[test]
fn native_order_modal_moves_full_drafts_cancel_is_inert_and_real_rejected_apply_retries_without_reordering_selection()
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
    let original: Vec<_> = state
        .runtime
        .proxy_groups
        .iter()
        .map(|group| group.name.clone())
        .collect();
    assert!(original.len() > 1);
    let raw = state.runtime.proxies.clone();
    let closed = tree_size(&Tree::new(state.view().as_widget()));
    assert_eq!(
        state
            .update(Message::MoveProxyGroupUp(original[1].clone()))
            .units(),
        0
    );
    assert!(state.runtime.group_order_open);
    assert!(tree_size(&Tree::new(state.view().as_widget())) > closed);
    assert_eq!(state.runtime.group_order_editor.draft[0], original[1]);
    assert!(preferences.custom_group_order().unwrap().is_empty());
    assert_eq!(state.update(Message::CancelProxyGroupOrder).units(), 0);
    assert!(!state.runtime.group_order_open);
    assert_eq!(state.runtime.group_order_editor.draft, original);
    assert!(preferences.custom_group_order().unwrap().is_empty());
    let _ = state.update(Message::MoveProxyGroupUp(original[1].clone()));
    let mut unavailable = state.surface.latest().unwrap().clone();
    unavailable.revision += 1;
    unavailable.pages.proxies =
        PageData::failed(Failure::new(ErrorCode::Network, "read failed", true));
    assert!(state.apply_shared_surface_snapshot(unavailable));
    assert!(!state.runtime.group_order_editor.can_apply());
    assert_eq!(state.update(Message::ApplyProxyGroupOrder).units(), 0);
    let expected = state.runtime.group_order_editor.draft.clone();
    publish(&mut state, &preferences);
    assert_eq!(state.runtime.group_order_editor.draft, expected);
    let (rejecting, _) = rejecting_application();
    state.commands = Some(rejecting);
    let task = state.update(Message::ApplyProxyGroupOrder);
    let token = state
        .runtime
        .group_order_editor
        .pending
        .as_ref()
        .unwrap()
        .token;
    let _ = state.update(Message::CancelProxyGroupOrder);
    assert!(state.runtime.group_order_open);
    let _ = state.update(Message::ProxyGroupOrderApplied {
        token: token + 1,
        result: Ok(()),
    });
    assert!(state.runtime.group_order_editor.pending.is_some());
    let completion = terminal(task);
    let _ = state.update(completion);
    assert!(state.runtime.group_order_open);
    assert_eq!(state.runtime.group_order_editor.draft, expected);
    assert!(state.runtime.group_order_editor.failure.is_some());
    assert!(preferences.custom_group_order().unwrap().is_empty());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    state.commands = Some(application);
    let task = state.update(Message::ApplyProxyGroupOrder);
    let _ = state.update(terminal(task));
    assert!(!state.runtime.group_order_open);
    assert_eq!(preferences.custom_group_order().unwrap(), expected);
    assert_eq!(
        state
            .runtime
            .proxy_groups
            .iter()
            .map(|group| group.name.clone())
            .collect::<Vec<_>>(),
        original,
        "command success cannot invent a controller observation"
    );
    publish(&mut state, &preferences);
    assert_eq!(
        state
            .runtime
            .proxy_groups
            .iter()
            .map(|group| group.name.clone())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(state.runtime.proxies, raw);
}
