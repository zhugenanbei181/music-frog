//! test-intent: behavior
use crate::command_harness::{
    attach_product_application_with_accepted_owner_snapshot, recording_application,
};
use crate::probe_settings_store::ProbeSettingsStore;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::runtime_probe_settings::project_settings;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::runtime_settings::RuntimeProbeSettings;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_iced::state::AppState;
use infiltrator_iced::state::runtime::ProbeSettingsDialogState;
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
    attach_product_application_with_accepted_owner_snapshot(&mut state, application);
    replay(&mut state, &store);
    let original = state
        .runtime
        .runtime_probe_settings_editor
        .applied()
        .cloned();
    let closed_size = tree_size(&Tree::new(state.view().as_widget()));
    assert_eq!(state.update(Message::OpenRuntimeProbeSettings).units(), 0);
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Editing
    );
    assert!(tree_size(&Tree::new(state.view().as_widget())) > closed_size);
    let _ = state.update(Message::UpdateRuntimeProbeTimeoutDraft("0".into()));
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .validation()
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(state.update(Message::ApplyRuntimeProbeSettings).units(), 0);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::CancelRuntimeProbeSettings);
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Closed
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .applied()
            .cloned(),
        original
    );
    let _ = state.update(Message::OpenRuntimeProbeSettings);
    let _ = state.update(Message::UpdateRuntimeProbeUrlDraft(
        "https://probe.example.test/check".into(),
    ));
    let _ = state.update(Message::UpdateRuntimeProbeTimeoutDraft("60000".into()));
    replay(&mut state, &store);
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .draft()
            .unwrap()
            .timeout_ms,
        "60000",
        "facts refresh retains edited inputs"
    );
    store.reject.store(true, Ordering::SeqCst);
    let task = state.update(Message::ApplyRuntimeProbeSettings);
    let pending = state
        .runtime
        .runtime_probe_settings_editor
        .pending()
        .unwrap()
        .token;
    assert_eq!(state.update(Message::ApplyRuntimeProbeSettings).units(), 0);
    let _ = state.update(Message::CancelRuntimeProbeSettings);
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Editing
    );
    let _ = state.update(Message::RuntimeProbeSettingsApplied {
        token: pending + 1,
        result: Ok(()),
    });
    assert!(
        state
            .runtime
            .runtime_probe_settings_editor
            .pending()
            .is_some()
    );
    let result = terminal(task);
    let _ = state.update(result);
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Editing
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .applied()
            .cloned(),
        original
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .draft()
            .unwrap()
            .timeout_ms,
        "60000"
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .failure()
            .unwrap()
            .code,
        ErrorCode::Storage
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.reject.store(false, Ordering::SeqCst);
    let task = state.update(Message::ApplyRuntimeProbeSettings);
    let result = terminal(task);
    let _ = state.update(result);
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Closed
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        store
            .settings
            .lock()
            .unwrap()
            .runtime_panel
            .probe_settings
            .timeout_ms(),
        60_000
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .applied()
            .unwrap()
            .timeout_ms(),
        60_000
    );
    let _ = state.update(Message::RuntimeProbeSettingsApplied {
        token: pending,
        result: Err(Failure::new(ErrorCode::Storage, "obsolete result", true)),
    });
    assert!(
        state
            .runtime
            .runtime_probe_settings_editor
            .failure()
            .is_none()
    );
    replay(&mut state, &store);
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .applied()
            .unwrap()
            .url(),
        "https://probe.example.test/check"
    );
}

#[test]
fn escape_and_navigation_discard_only_unsubmitted_drafts_and_never_reopen_after_late_failure() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Proxies));
    state.runtime.runtime_probe_settings_editor.observe(
        &infiltrator_contract::runtime_settings::RuntimeProbeSettingsState::Available {
            settings: RuntimeProbeSettings::default(),
        },
    );
    let original = state
        .runtime
        .runtime_probe_settings_editor
        .applied()
        .cloned();
    let _ = state.update(Message::OpenRuntimeProbeSettings);
    let _ = state.update(Message::UpdateRuntimeProbeTimeoutDraft("1234".into()));
    let _ = state.update(Message::KeyboardChord {
        key: "Escape".into(),
        modifiers: Default::default(),
    });
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Closed
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .applied()
            .cloned(),
        original
    );
    let _ = state.update(Message::OpenRuntimeProbeSettings);
    let _ = state.update(Message::UpdateRuntimeProbeTimeoutDraft("60000".into()));
    let pending = state.runtime.runtime_probe_settings_editor.begin().unwrap();
    let _ = state.update(Message::KeyboardChord {
        key: "Escape".into(),
        modifiers: Default::default(),
    });
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Editing
    );
    assert!(
        state
            .runtime
            .runtime_probe_settings_editor
            .pending()
            .is_some()
    );
    let _ = state.update(Message::Navigate(Route::Settings));
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Closed
    );
    assert!(
        state
            .runtime
            .runtime_probe_settings_editor
            .pending()
            .is_some()
    );
    let _ = state.update(Message::RuntimeProbeSettingsApplied {
        token: pending.token,
        result: Err(Failure::new(ErrorCode::Storage, "write failed", true)),
    });
    assert_eq!(
        state.runtime.probe_settings_dialog,
        ProbeSettingsDialogState::Closed
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .applied()
            .cloned(),
        original
    );
    assert_eq!(
        state
            .runtime
            .runtime_probe_settings_editor
            .draft()
            .unwrap()
            .timeout_ms,
        "60000"
    );
}
