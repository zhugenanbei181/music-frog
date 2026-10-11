use super::{CoreApplication, DispatchedMessage, Inner};
use infiltrator_contract::core_identity::{CoreCommandOutcome, CoreIdentity};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot, CoreWatchdogSnapshot};
use infiltrator_domain::core_state::CoreState;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::mpsc::Receiver;
use std::thread::Builder;

pub(super) fn spawn_dispatch_worker(
    inner: Weak<Inner>,
    dispatch_rx: Receiver<DispatchedMessage>,
    runtime: Arc<dyn ApplicationRuntime>,
) {
    let _ = Builder::new()
        .name("infiltrator-application".to_owned())
        .spawn(move || {
            while let Ok(message) = dispatch_rx.recv() {
                let Some(inner) = inner.upgrade() else {
                    return;
                };
                match message {
                    DispatchedMessage::Command(command) => {
                        runtime.block_on(Box::pin(async move {
                            let application = CoreApplication { inner };
                            let result = application
                                .execute_scoped_with_id(command.request_id, command.request)
                                .await;
                            let outcome = CoreCommandOutcome {
                                scope: application.snapshot().scope(),
                                result,
                            };
                            let _ = command.reply.send(outcome);
                        }));
                    }
                    DispatchedMessage::SpeedtestCompleted {
                        scope,
                        run_id,
                        outcome,
                    } => {
                        let application = CoreApplication { inner };
                        match application.finish_speedtest_task(scope, run_id, outcome) {
                            crate::speedtest_application::SpeedtestCompletionAdmission::Applied => {}
                            crate::speedtest_application::SpeedtestCompletionAdmission::IgnoredStaleRun {
                                completed,
                                current,
                            } => log::debug!(
                                "ignored stale speedtest completion {:?}; active run is {:?}",
                                completed,
                                current
                            ),
                            crate::speedtest_application::SpeedtestCompletionAdmission::IgnoredStaleScope {
                                completed,
                            } => log::debug!(
                                "ignored speedtest completion {:?} from a stale Core scope",
                                completed
                            ),
                        }
                    }
                }
            }
        });
}

pub(super) fn invalid_state_failure(operation: &str) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!("operation `{operation}` is not valid in the current core state"),
        false,
    )
}

pub(super) fn snapshot_from_state(
    state: &CoreState,
    revision: u64,
    watchdog: CoreWatchdogSnapshot,
    identity: Option<CoreIdentity>,
) -> CoreSnapshot {
    let (lifecycle, generation, session_token, failure) = match state {
        CoreState::Idle { generation } => (CoreLifecycle::Stopped, *generation, None, None),
        CoreState::Starting {
            generation,
            session_token,
        } => (
            CoreLifecycle::Starting,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Running {
            generation,
            session_token,
            ..
        } => (
            CoreLifecycle::Running,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Reloading {
            generation,
            session_token,
            ..
        } => (
            CoreLifecycle::Ready,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Stopping {
            generation,
            session_token,
        } => (
            CoreLifecycle::Stopping,
            *generation,
            Some(*session_token),
            None,
        ),
        CoreState::Failed {
            generation,
            session_token,
            error,
        } => (
            CoreLifecycle::Failed,
            *generation,
            Some(*session_token),
            Some(Failure::new(ErrorCode::Internal, error.clone(), false)),
        ),
    };

    CoreSnapshot {
        identity,
        lifecycle,
        generation,
        session_token,
        revision,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        memory_bytes: None,
        watchdog,
    }
}
