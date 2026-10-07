//! test-intent: behavior
//! Native buttons drive actual Core commands and host files, including cancellation and retry.
use super::content;
use crate::state::AppState;
use crate::test_mounts::native_widgets::native;
use crate::types::message::Message;
use crate::view::runtime::logs_controls::export;
use crate::view_root::interaction_regions::InteractionRegion;
use async_trait::async_trait;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{
    LogCaptureProcess, append_follow_records, populate_logs,
};
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::log_export::{LogExportArtifact, LogExportReceipt};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_desktop::log_export::DesktopLogExportPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::log_export::LogExportPort;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tempfile::tempdir;
use tokio::runtime::Builder;

#[test]
fn native_unsupported_host_keeps_the_reason_and_allows_effect_free_close() {
    let directory = tempdir().unwrap();
    let (mut state, _) = AppState::new();
    let core = install(
        &mut state,
        Arc::new(DesktopLogExportPort::new(directory.path())),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_logs(core.log_application()),
    ));
    let task = launch(&mut state);
    drive(&mut state, task);
    let task = click(&mut state, InteractionRegion::LogExportConfirm);
    drive(&mut state, task);
    assert_eq!(
        state.diag.log_export.failure.as_ref().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(!state.diag.log_export.can_retry());
    assert!(state.diag.log_export.receipt.is_none());
    assert!(!directory.path().join("exports").exists());
    let task = click(&mut state, InteractionRegion::LogExportCancel);
    drive(&mut state, task);
    assert!(!state.diag.log_export.open);
    assert!(!directory.path().join("exports").exists());
    run(core.close()).unwrap();
}
fn run<T>(future: impl Future<Output = T>) -> T {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
fn drive(state: &mut AppState, task: Task<Message>) {
    run(async {
        let mut stream = into_stream(task).expect("actual Core task");
        while let Some(action) = stream.next().await {
            if let Action::Output(message) = action {
                let _ = state.update(message);
            }
        }
    });
}
fn install(state: &mut AppState, port: Arc<dyn LogExportPort>) -> CoreApplication {
    let process = Arc::new(LogCaptureProcess::default());
    let core = CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().unwrap(),
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_logs(core.log_application())
            .with_log_export(LogExportApplication::new(
                core.log_application(),
                Some(port),
                vec![],
            )),
    ));
    run(populate_logs(&core)).unwrap();
    let snapshot = core.snapshot();
    assert!(core.log_application().ingest(
        LogSession {
            generation: snapshot.generation,
            token: snapshot.session_token.unwrap()
        },
        RuntimeStreamEvent::Item(
            "GET https://api.sub.lan/token?token=secret_sub_token_123456 HTTP/1.1".into()
        )
    ));
    state.commands = Some(core.clone());
    core
}
fn launch(state: &mut AppState) -> Task<Message> {
    let messages = native(export(state), InteractionRegion::LogExportLaunch.id(), None);
    assert_eq!(messages.len(), 1);
    state.update(messages.into_iter().next().unwrap())
}
fn click(state: &mut AppState, region: InteractionRegion) -> Task<Message> {
    let messages = native(content(state), region.id(), None);
    assert_eq!(messages.len(), 1, "enabled native action");
    state.update(messages.into_iter().next().unwrap())
}
pub(crate) fn exercise_redacted_export(state: &mut AppState) {
    let directory = tempdir().unwrap();
    let core = install(state, Arc::new(DesktopLogExportPort::new(directory.path())));
    let task = launch(state);
    assert!(state.diag.log_export.pending.is_some());
    assert!(
        native(
            content(state),
            InteractionRegion::LogExportCancel.id(),
            None
        )
        .is_empty()
    );
    drive(state, task);
    assert_eq!(state.diag.log_export.summary.as_ref().unwrap().records, 4);
    assert!(state.diag.log_export.receipt.is_none());
    assert!(!directory.path().join("exports").exists());
    let task = click(state, InteractionRegion::LogExportCancel);
    drive(state, task);
    assert!(!state.diag.log_export.open);
    assert!(!directory.path().join("exports").exists());
    let task = launch(state);
    drive(state, task);
    append_follow_records(&core).unwrap();
    let task = click(state, InteractionRegion::LogExportConfirm);
    assert!(
        native(
            content(state),
            InteractionRegion::LogExportCancel.id(),
            None
        )
        .is_empty()
    );
    let _ = state.update(Message::KeyboardChord {
        key: "Escape".into(),
        modifiers: KeyModifiers::default(),
    });
    assert!(state.diag.log_export.open && state.diag.log_export.pending.is_some());
    drive(state, task);
    let receipt = state.diag.log_export.receipt.as_ref().unwrap();
    assert!(Path::new(&receipt.path).starts_with(directory.path().join("exports")));
    let exported = fs::read_to_string(&receipt.path).expect("actual exported file");
    assert_eq!(exported.lines().count(), 4);
    assert!(!exported.contains("continued controller record"));
    assert!(!exported.contains("secret_sub_token_123456"));
    assert!(exported.contains("token=***"));
    assert_eq!(receipt.bytes_written, exported.len());
    let task = click(state, InteractionRegion::LogExportCancel);
    drive(state, task);
    assert!(!state.diag.log_export.open);
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        1
    );
    run(core.close()).unwrap();
}
#[test]
fn native_prepare_cancel_confirm_pending_and_real_redacted_receipt_preserve_fixed_scope() {
    let (mut state, _) = AppState::new();
    exercise_redacted_export(&mut state);
}
struct ControlledWriter {
    inner: DesktopLogExportPort,
    denied: AtomicBool,
}
#[async_trait]
impl LogExportPort for ControlledWriter {
    async fn save(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        if self.denied.load(Ordering::Acquire) {
            return Err(PortError::PermissionDenied(
                "export directory denied".into(),
            ));
        }
        self.inner.save(artifact).await
    }
}
#[test]
fn native_permission_failure_retries_the_same_reviewed_source_and_restarted_source_never_writes() {
    let directory = tempdir().unwrap();
    let writer = Arc::new(ControlledWriter {
        inner: DesktopLogExportPort::new(directory.path()),
        denied: AtomicBool::new(true),
    });
    let (mut state, _) = AppState::new();
    let core = install(&mut state, writer.clone());
    let task = launch(&mut state);
    drive(&mut state, task);
    let summary = state.diag.log_export.summary.clone().unwrap();
    let task = click(&mut state, InteractionRegion::LogExportConfirm);
    drive(&mut state, task);
    assert_eq!(
        state.diag.log_export.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert!(state.diag.log_export.open && state.diag.log_export.receipt.is_none());
    assert!(!directory.path().join("exports").exists());
    writer.denied.store(false, Ordering::Release);
    let task = click(&mut state, InteractionRegion::LogExportRetry);
    drive(&mut state, task);
    assert_eq!(
        state.diag.log_export.receipt.as_ref().unwrap().summary,
        summary
    );
    let task = click(&mut state, InteractionRegion::LogExportCancel);
    drive(&mut state, task);
    let task = launch(&mut state);
    drive(&mut state, task);
    run(core.execute(CommandIntent::RestartCore))
        .into_unit()
        .unwrap();
    let task = click(&mut state, InteractionRegion::LogExportConfirm);
    drive(&mut state, task);
    assert_eq!(
        state.diag.log_export.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert!(!state.diag.log_export.can_retry());
    assert!(state.diag.log_export.receipt.is_none());
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        1
    );
    let task = click(&mut state, InteractionRegion::LogExportCancel);
    drive(&mut state, task);
    assert!(!state.diag.log_export.open);
    run(core.close()).unwrap();
}
