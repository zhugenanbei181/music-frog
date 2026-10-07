//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::proxy_inspection_fixtures::{INSPECTION_NODE, observed_proxy};
use infiltrator_application::proxy_inspection_projection::project_proxy_inspection;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageData, ProxiesPageSnapshot, SurfaceSnapshot};
use infiltrator_domain::proxy::Proxy;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use tokio::runtime::Builder;

fn state() -> AppState {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    state
        .runtime
        .proxies
        .insert(INSPECTION_NODE.into(), observed_proxy());
    state
}
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("tracked probe task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("probe must deliver a terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}

#[test]
fn native_inspection_uses_shared_observations_and_close_escape_and_navigation_never_select_nodes() {
    let mut state = state();
    let proxies = state.runtime.proxies.clone();
    assert_eq!(
        state
            .update(Message::InspectProxy(Some(INSPECTION_NODE.into())))
            .units(),
        0
    );
    assert_eq!(
        state.proxy_inspection(INSPECTION_NODE),
        Some(project_proxy_inspection(INSPECTION_NODE, &observed_proxy()))
    );
    assert_eq!(
        state.runtime.inspecting_proxy.as_deref(),
        Some(INSPECTION_NODE)
    );
    assert_eq!(state.update(Message::InspectProxy(None)).units(), 0);
    assert!(state.runtime.inspecting_proxy.is_none());
    let _ = state.update(Message::InspectProxy(Some(INSPECTION_NODE.into())));
    assert_eq!(
        state
            .update(Message::KeyboardChord {
                key: "Escape".into(),
                modifiers: KeyModifiers::default()
            })
            .units(),
        0
    );
    assert!(state.runtime.inspecting_proxy.is_none());
    let _ = state.update(Message::InspectProxy(Some(INSPECTION_NODE.into())));
    let _ = state.update(Message::Navigate(Route::Profiles));
    assert!(state.runtime.inspecting_proxy.is_none());
    assert_eq!(state.runtime.proxies, proxies);
}

#[test]
fn native_inspection_follows_stable_identity_on_refresh_and_rejects_a_removed_node() {
    let mut state = state();
    let _ = state.update(Message::InspectProxy(Some(INSPECTION_NODE.into())));
    let mut facts = state.runtime.proxies.clone();
    let mut updated = observed_proxy();
    if let Proxy::Shadowsocks(proxy) = &mut updated {
        proxy.base.delay = Some(91)
    }
    facts.insert(INSPECTION_NODE.into(), updated);
    let _ = state.update(Message::ProxiesLoaded(Ok(facts.clone())));
    assert_eq!(
        state.runtime.inspecting_proxy.as_deref(),
        Some(INSPECTION_NODE)
    );
    assert_eq!(
        state.proxy_inspection(INSPECTION_NODE).unwrap().delay_ms,
        Some(91)
    );
    facts.remove(INSPECTION_NODE);
    let _ = state.update(Message::ProxiesLoaded(Ok(facts)));
    assert!(state.runtime.inspecting_proxy.is_none());
    assert_eq!(
        state
            .update(Message::InspectProxy(Some(INSPECTION_NODE.into())))
            .units(),
        0
    );
    assert!(state.runtime.inspecting_proxy.is_none());
}

#[test]
fn native_probe_preserves_failure_retries_the_same_intent_and_ignores_feedback_after_close() {
    let mut state = state();
    let (application, handler) = recording_application();
    state.commands = Some(application);
    state.shell.demo = false;
    let _ = state.update(Message::InspectProxy(Some(INSPECTION_NODE.into())));
    let failure = Failure::new(ErrorCode::Permission, "grant proxy probe permission", true);
    *handler.1.lock().unwrap() = Some(failure.clone());
    let task = state.update(Message::TestInspectedProxy);
    assert_eq!(state.update(Message::TestInspectedProxy).units(), 0);
    assert!(state.runtime.inspection_probe.pending.is_some());
    let result = terminal(task);
    assert_eq!(state.update(result).units(), 0);
    assert_eq!(state.runtime.inspection_probe.failure, Some(failure));
    assert_eq!(
        state.runtime.inspecting_proxy.as_deref(),
        Some(INSPECTION_NODE)
    );
    *handler.1.lock().unwrap() = None;
    let task = state.update(Message::TestInspectedProxy);
    let result = terminal(task);
    assert_eq!(state.update(result).units(), 1);
    assert!(state.runtime.inspection_probe.failure.is_none());
    let task = state.update(Message::TestInspectedProxy);
    assert_eq!(state.update(Message::InspectProxy(None)).units(), 0);
    let result = terminal(task);
    assert_eq!(state.update(result).units(), 0);
    assert!(state.runtime.inspecting_proxy.is_none());
    assert!(state.runtime.inspection_probe.pending.is_none());
    assert_eq!(
        *handler.0.lock().unwrap(),
        vec![
            CommandIntent::TestNodeDelay {
                node: INSPECTION_NODE.into(),
                url: Some("http://www.gstatic.com/generate_204".into()),
                timeout_ms: Some(5000),
            };
            3
        ]
    );
}

#[test]
fn failed_and_loading_reader_snapshots_keep_the_real_detail_disable_probe_and_recover_before_confirmed_deletion()
 {
    let mut state = state();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("unrelated host capability"),
    );
    snapshot.revision = 1;
    let mut page = ProxiesPageSnapshot {
        name_runs: Default::default(),
        search_query: String::new(),
        groups: vec![],
        testing: false,
        active_exit: String::new(),
        filter_alive: Default::default(),
        sort_order: Default::default(),
        compact_view: false,
        custom_node: Default::default(),
        node_details: vec![project_proxy_inspection(INSPECTION_NODE, &observed_proxy())],
    };
    snapshot.pages.proxies = PageData::ready(page.clone());
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    let _ = state.update(Message::InspectProxy(Some(INSPECTION_NODE.into())));
    let observed = state.proxy_inspection(INSPECTION_NODE).unwrap();
    let failure = Failure::new(ErrorCode::Network, "controller read failed", true);
    for bad in [
        PageData::failed(failure.clone()),
        PageData::unavailable(failure.clone()),
        PageData::loading(),
    ] {
        snapshot.revision += 1;
        snapshot.pages.proxies = bad;
        assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
        assert_eq!(
            state.runtime.inspecting_proxy.as_deref(),
            Some(INSPECTION_NODE)
        );
        assert_eq!(
            state.proxy_inspection(INSPECTION_NODE),
            Some(observed.clone())
        );
        assert!(!state.runtime.inspection_read.can_probe());
        assert_eq!(state.update(Message::TestInspectedProxy).units(), 0);
        assert!(state.runtime.inspection_probe.pending.is_none());
    }
    page.node_details[0].delay_ms = Some(91);
    snapshot.revision += 1;
    snapshot.pages.proxies = PageData::ready(page.clone());
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert!(state.runtime.inspection_read.can_probe());
    assert_eq!(
        state.proxy_inspection(INSPECTION_NODE).unwrap().delay_ms,
        Some(91)
    );
    assert!(state.runtime.inspection_read.failure.is_none());
    page.node_details.clear();
    snapshot.revision += 1;
    snapshot.pages.proxies = PageData::empty(page);
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert!(state.runtime.inspecting_proxy.is_none());
    assert!(state.runtime.inspection_read.detail.is_none());
}
