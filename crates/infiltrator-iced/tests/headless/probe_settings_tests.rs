//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::probe_settings_store::ProbeSettingsStore;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::proxy_probe_options_projection::project_settings;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::runtime::Builder;

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("native apply submits actual command");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("terminal durable result")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn replay(state: &mut AppState, store: &ProbeSettingsStore) {
    let mut snapshot = state.surface.latest().cloned().unwrap_or_else(|| {
        SurfaceSnapshot::unavailable(
            SurfaceKind::IcedDesktop,
            HostKind::Desktop,
            Failure::unsupported("irrelevant host capability"),
        )
    });
    snapshot.revision = state.surface.revision() + 1;
    snapshot.probe_settings = project_settings(Some(&Ok(store.settings.lock().unwrap().clone())));
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn tree_size(tree: &Tree) -> usize {
    1 + tree.children.iter().map(tree_size).sum::<usize>()
}

#[test]
fn native_parameter_modal_cancels_without_writing_and_retries_durable_failure_without_publishing_drafts()
 {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    let store = Arc::new(ProbeSettingsStore::default());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_settings(SettingsApplication::new(store.clone())),
    ));
    state.commands = Some(application);
    state.shell.demo = false;
    replay(&mut state, &store);
    let original = state.runtime.probe_options_editor.applied.clone();
    let closed_size = tree_size(&Tree::new(state.view().as_widget()));
    assert_eq!(state.update(Message::OpenProxyProbeOptions).units(), 0);
    assert!(state.runtime.probe_options_open);
    assert!(tree_size(&Tree::new(state.view().as_widget())) > closed_size);
    let _ = state.update(Message::UpdateDelayTimeoutMs("60000".into()));
    assert_eq!(
        state
            .runtime
            .probe_options_editor
            .validation
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(state.update(Message::ApplyProxyProbeOptions).units(), 0);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::CancelProxyProbeOptions);
    assert!(!state.runtime.probe_options_open);
    assert_eq!(state.runtime.probe_options_editor.applied, original);
    let _ = state.update(Message::OpenProxyProbeOptions);
    let _ = state.update(Message::UpdateDelayTestUrl(
        "https://probe.example.test/check".into(),
    ));
    let _ = state.update(Message::UpdateDelayTimeoutMs("32767".into()));
    replay(&mut state, &store);
    assert_eq!(
        state.runtime.probe_options_editor.draft.timeout_ms, "32767",
        "facts refresh retains edited inputs"
    );
    store.reject.store(true, Ordering::SeqCst);
    let task = state.update(Message::ApplyProxyProbeOptions);
    let pending = state
        .runtime
        .probe_options_editor
        .pending
        .as_ref()
        .unwrap()
        .token;
    assert_eq!(state.update(Message::ApplyProxyProbeOptions).units(), 0);
    let _ = state.update(Message::CancelProxyProbeOptions);
    assert!(state.runtime.probe_options_open);
    let _ = state.update(Message::ProxyProbeOptionsApplied {
        token: pending + 1,
        result: Ok(()),
    });
    assert!(state.runtime.probe_options_editor.pending.is_some());
    let result = terminal(task);
    let _ = state.update(result);
    assert!(state.runtime.probe_options_open);
    assert_eq!(state.runtime.probe_options_editor.applied, original);
    assert_eq!(state.runtime.probe_options_editor.draft.timeout_ms, "32767");
    assert_eq!(
        state
            .runtime
            .probe_options_editor
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Storage
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.reject.store(false, Ordering::SeqCst);
    let task = state.update(Message::ApplyProxyProbeOptions);
    let result = terminal(task);
    let _ = state.update(result);
    assert!(!state.runtime.probe_options_open);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        store
            .settings
            .lock()
            .unwrap()
            .runtime_panel
            .delay_timeout_ms,
        32767
    );
    assert_eq!(
        state
            .runtime
            .probe_options_editor
            .applied
            .as_ref()
            .unwrap()
            .timeout_ms,
        32767
    );
    let _ = state.update(Message::ProxyProbeOptionsApplied {
        token: pending,
        result: Err(Failure::new(ErrorCode::Storage, "obsolete result", true)),
    });
    assert!(state.runtime.probe_options_editor.failure.is_none());
    replay(&mut state, &store);
    assert_eq!(
        state
            .runtime
            .probe_options_editor
            .applied
            .as_ref()
            .unwrap()
            .test_url,
        "https://probe.example.test/check"
    );
}

#[test]
fn escape_and_navigation_discard_only_unsubmitted_drafts_and_never_reopen_after_late_failure() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    state.runtime.probe_options_editor.can_persist = true;
    let original = state.runtime.probe_options_editor.applied.clone();
    let _ = state.update(Message::OpenProxyProbeOptions);
    let _ = state.update(Message::UpdateDelayTimeoutMs("1234".into()));
    let _ = state.update(Message::KeyboardChord {
        key: "Escape".into(),
        modifiers: Default::default(),
    });
    assert!(!state.runtime.probe_options_open);
    assert_eq!(state.runtime.probe_options_editor.applied, original);
    assert!(!state.runtime.probe_options_editor.dirty);
    let _ = state.update(Message::OpenProxyProbeOptions);
    let _ = state.update(Message::UpdateDelayTimeoutMs("32767".into()));
    let pending = state.runtime.probe_options_editor.begin().unwrap();
    let _ = state.update(Message::KeyboardChord {
        key: "Escape".into(),
        modifiers: Default::default(),
    });
    assert!(state.runtime.probe_options_open);
    assert!(state.runtime.probe_options_editor.pending.is_some());
    let _ = state.update(Message::Navigate(Route::Settings));
    assert!(!state.runtime.probe_options_open);
    assert!(state.runtime.probe_options_editor.pending.is_some());
    let _ = state.update(Message::ProxyProbeOptionsApplied {
        token: pending.token,
        result: Err(Failure::new(ErrorCode::Storage, "write failed", true)),
    });
    assert!(!state.runtime.probe_options_open);
    assert_eq!(state.runtime.probe_options_editor.applied, original);
    assert_eq!(state.runtime.probe_options_editor.draft.timeout_ms, "32767");
}
