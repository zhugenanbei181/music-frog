//! test-intent: behavior
use crate::command_harness::{
    attach_product_application_with_accepted_owner_snapshot, install_lifecycle_test_capabilities,
};
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::core_control::{CoreControlAction, CoreControlLabel};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_iced::state::AppState;
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
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("test engine ID"),
            CoreInstanceId::new("iced-core-control").expect("test instance ID"),
            None,
        ))
    }

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
    let (mut state, _) = AppState::new();
    let process = Arc::new(Process::default());
    let application = CoreApplication::new(
        process.clone(),
        process.clone(),
        tokio_application_runtime().unwrap(),
    );
    install_lifecycle_test_capabilities(&application);
    state.runtime.core_lifecycle = application.snapshot().lifecycle_snapshot();
    state.runtime.status = RuntimeStatus::Stopped;
    attach_product_application_with_accepted_owner_snapshot(&mut state, application);
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
    let start = state.update(Message::StartCore);
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Pending
    );
    assert_eq!(state.update(Message::StartCore).units(), 0);
    assert_eq!(state.update(Message::StopCore).units(), 0);
    let result = terminal(start);
    assert_eq!(process.starts.load(Ordering::SeqCst), 1);
    assert_eq!(state.runtime.status, RuntimeStatus::Stopped);
    assert_eq!(state.update(result).units(), 3);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(
        state.core_control_projection().action,
        Some(CoreControlAction::Stop)
    );
    let stop = state.update(Message::StopCore);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert_eq!(state.update(Message::StopCore).units(), 0);
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
    let result = terminal(state.update(Message::StartCore));
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
    let result = terminal(state.update(Message::StartCore));
    assert_eq!(state.update(result).units(), 3);
    assert_eq!(process.starts.load(Ordering::SeqCst), 2);
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
    assert!(state.runtime.lifecycle_failure.is_none());
    let snapshot = state.commands.as_ref().unwrap().snapshot();
    let _ = state.update(Message::CoreControlFinished {
        action: CoreControlAction::Stop,
        token: state.runtime.lifecycle_token.wrapping_sub(1),
        result: Ok(()),
        requested_scope: None,
        terminal_scope: None,
        snapshot: Box::new(snapshot),
    });
    assert_eq!(state.runtime.status, RuntimeStatus::Running);
}

#[test]
fn missing_core_owner_disables_lifecycle_controls_without_local_bootstrap() {
    let (mut state, _) = AppState::new();
    assert!(state.runtime.lifecycle_pending.is_none());
    assert_eq!(
        state.core_control_projection().label,
        CoreControlLabel::Unavailable
    );
    let lifecycle = state.runtime.core_lifecycle.clone();
    assert_eq!(state.update(Message::StartCore).units(), 0);
    assert_eq!(state.update(Message::StopCore).units(), 0);
    assert_eq!(state.runtime.core_lifecycle, lifecycle);
    assert!(state.runtime.lifecycle_pending.is_none());
}
