//! test-intent: behavior
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::core_control::{CoreControlAction, CoreControlLabel};
use infiltrator_contract::error::{ErrorCode, Failure, InfiltratorError};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_iced::types::runtime::RuntimeStatus;
use infiltrator_ports::core_process::{CoreProcess, CoreReadiness};
use infiltrator_ports::error::PortError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use tokio::runtime::Builder;

#[derive(Default)]
struct Process {
    running: AtomicBool,
    deny_start: AtomicBool,
    starts: AtomicUsize,
    stops: AtomicUsize,
}
#[async_trait::async_trait]
impl CoreProcess for Process {
    async fn start(&self) -> Result<(), PortError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        if self.deny_start.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "grant process permission".into(),
            ));
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn stop(&self) -> Result<(), PortError> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(if self.running.load(Ordering::SeqCst) {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }
    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}
#[async_trait::async_trait]
impl CoreReadiness for Process {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("test-core".into())
    }
}
fn composed() -> (AppState, Arc<Process>) {
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    let process = Arc::new(Process::default());
    let application = CoreApplication::new(
        process.clone(),
        process.clone(),
        tokio_application_runtime().unwrap(),
    );
    state.runtime.core_lifecycle = application.snapshot().lifecycle_snapshot();
    state.runtime.status = RuntimeStatus::Stopped;
    state.commands = Some(application);
    state.shell.demo = false;
    (state, process)
}
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("native lifecycle task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("native lifecycle task must deliver its terminal message")
            };
            assert!(stream.next().await.is_none());
            message
        })
}

#[test]
fn composed_native_start_stop_executes_the_actual_ports_and_keeps_the_command_owner() {
    let (mut state, process) = composed();
    let start = state.update(Message::StartProxy);
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Pending
    );
    assert_eq!(state.update(Message::StartProxy).units(), 0);
    assert_eq!(state.update(Message::StopProxy).units(), 0);
    let result = terminal(start);
    assert_eq!(process.starts.load(Ordering::SeqCst), 1);
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert_eq!(state.update(result).units(), 3);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(
        state.core_control_projection().action,
        Some(CoreControlAction::Stop)
    );
    let stop = state.update(Message::StopProxy);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(state.update(Message::StopProxy).units(), 0);
    let result = terminal(stop);
    assert_eq!(process.stops.load(Ordering::SeqCst), 1);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(state.update(result).units(), 0);
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert_eq!(
        state.core_control_projection().action,
        Some(CoreControlAction::Start)
    );
    assert_eq!(
        state.commands.as_ref().unwrap().snapshot().lifecycle,
        CoreLifecycle::Stopped
    );
    assert!(state.runtime.lifecycle_pending.is_none());
}

#[test]
fn composed_permission_failure_retains_its_code_and_retry_runs_the_same_owner() {
    let (mut state, process) = composed();
    process.deny_start.store(true, Ordering::SeqCst);
    let result = terminal(state.update(Message::StartProxy));
    assert_eq!(state.update(result).units(), 0);
    let failure = Failure::from(PortError::PermissionDenied(
        "grant process permission".into(),
    ));
    assert_eq!(state.runtime.lifecycle_failure, Some(failure.clone()));
    assert_eq!(state.core_control_projection().issue, Some(failure));
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Retry
    );
    process.deny_start.store(false, Ordering::SeqCst);
    let result = terminal(state.update(Message::StartProxy));
    assert_eq!(state.update(result).units(), 3);
    assert_eq!(process.starts.load(Ordering::SeqCst), 2);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert!(state.runtime.lifecycle_failure.is_none());
    let snapshot = state.commands.as_ref().unwrap().snapshot();
    let _ = state.update(Message::CoreControlFinished {
        action: CoreControlAction::Stop,
        token: state.runtime.lifecycle_token.wrapping_sub(1),
        result: Ok(()),
        snapshot: Box::new(snapshot),
    });
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
}

#[test]
fn stop_is_pending_until_terminal_feedback_and_failures_do_not_claim_stopped() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    // Exercise production admission; returned tasks remain unpolled in this fixture.
    state.shell.demo = false;
    assert_eq!(
        state.core_control_projection().action,
        Some(CoreControlAction::Stop)
    );
    assert_eq!(state.update(Message::StopProxy).units(), 1);
    let token = state.runtime.lifecycle_token;
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Pending
    );
    assert_eq!(state.update(Message::StopProxy).units(), 0);
    assert_eq!(state.update(Message::StartProxy).units(), 0);
    let failure = Failure::new(ErrorCode::Permission, "grant shutdown permission", false);
    assert_eq!(
        state
            .update(Message::ProxyStopFinished(Err(failure.clone()), token))
            .units(),
        0
    );
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(state.core_control_projection().issue, Some(failure));
    assert!(
        state
            .shell
            .error_msg
            .as_ref()
            .unwrap()
            .contains("grant shutdown permission")
    );
    assert_eq!(state.update(Message::StopProxy).units(), 1);
    let retry = state.runtime.lifecycle_token;
    assert_eq!(
        state
            .update(Message::ProxyStopFinished(Ok(()), retry))
            .units(),
        0
    );
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert_eq!(
        state.core_control_projection().action,
        Some(CoreControlAction::Start)
    );
    assert!(state.runtime.lifecycle_failure.is_none());
}

#[test]
fn old_stop_results_cannot_finish_a_new_request_and_duplicate_start_is_rejected() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Overview));
    state.shell.demo = false;
    assert_eq!(state.update(Message::StopProxy).units(), 1);
    let old = state.runtime.lifecycle_token;
    let _ = state.update(Message::ProxyStopFinished(
        Err(Failure::new(ErrorCode::Permission, "refused", false)),
        old,
    ));
    assert_eq!(state.update(Message::StopProxy).units(), 1);
    let current = state.runtime.lifecycle_token;
    assert_ne!(old, current);
    let _ = state.update(Message::ProxyStopFinished(Ok(()), old));
    assert_eq!(
        state.runtime.lifecycle_pending,
        Some(CoreControlAction::Stop)
    );
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    let _ = state.update(Message::ProxyStopFinished(Ok(()), current));
    assert_eq!(state.update(Message::StartProxy).units(), 1);
    let start = state.runtime.lifecycle_token;
    assert_eq!(state.update(Message::StartProxy).units(), 0);
    assert_eq!(state.runtime.lifecycle_token, start);
    let _ = state.update(Message::ProxyStarted(
        Err(InfiltratorError::Mihomo("boot failed".into())),
        start,
    ));
    assert!(state.runtime.lifecycle_pending.is_none());
    assert_eq!(
        state.core_control_projection().action,
        Some(CoreControlAction::Start)
    );
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Retry
    );
}
