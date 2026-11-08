use super::{CoreApplication, DispatchedMessage, Inner};
use crate::speedtest_application::SpeedtestCompletionAdmission::Applied;
use crate::speedtest_application::SpeedtestCompletionAdmission::IgnoredStaleRun;
use crate::speedtest_application::SpeedtestCompletionAdmission::IgnoredStaleScope;
use infiltrator_contract::core_identity::CoreCommandOutcome;
use infiltrator_contract::snapshot::{CoreSessionObservation, CoreSnapshot, CoreWatchdogSnapshot};
use infiltrator_domain::core_state::CoreState;
use infiltrator_ports::application_runtime::ApplicationRuntime;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::CoreIdentity;
use lurker_core_contract::lifecycle::CoreLifecycle;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::mpsc::Receiver;
use std::thread::Builder;
use std::time::{SystemTime, UNIX_EPOCH};

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
                            Applied => {}
                            IgnoredStaleRun { completed, current } => log::debug!(
                                "ignored stale speedtest completion {:?}; active run is {:?}",
                                completed,
                                current
                            ),
                            IgnoredStaleScope { completed } => log::debug!(
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
    now_epoch_ms: u64,
) -> CoreSnapshot {
    let (lifecycle, generation, session_token, failure, session) = match state {
        CoreState::Idle { generation } => (
            CoreLifecycle::Stopped,
            *generation,
            None,
            None,
            CoreSessionObservation::Inactive,
        ),
        CoreState::Starting {
            generation,
            session_token,
            started_at_epoch_ms,
        } => (
            CoreLifecycle::Starting,
            *generation,
            Some(*session_token),
            None,
            active_session(*started_at_epoch_ms, now_epoch_ms),
        ),
        CoreState::Running {
            generation,
            session_token,
            endpoint: _,
            started_at_epoch_ms,
        } => (
            CoreLifecycle::Running,
            *generation,
            Some(*session_token),
            None,
            active_session(*started_at_epoch_ms, now_epoch_ms),
        ),
        CoreState::Reloading {
            generation,
            session_token,
            endpoint: _,
            started_at_epoch_ms,
        } => (
            CoreLifecycle::Ready,
            *generation,
            Some(*session_token),
            None,
            active_session(*started_at_epoch_ms, now_epoch_ms),
        ),
        CoreState::Stopping {
            generation,
            session_token,
            started_at_epoch_ms,
        } => (
            CoreLifecycle::Stopping,
            *generation,
            Some(*session_token),
            None,
            active_session(*started_at_epoch_ms, now_epoch_ms),
        ),
        CoreState::Failed {
            generation,
            session_token,
            error,
            started_at_epoch_ms: _,
        } => (
            CoreLifecycle::Failed,
            *generation,
            Some(*session_token),
            Some(Failure::new(ErrorCode::Internal, error.clone(), false)),
            CoreSessionObservation::Inactive,
        ),
    };

    CoreSnapshot {
        identity,
        lifecycle,
        generation,
        session_token,
        session,
        revision,
        routing_intent: Default::default(),
        core_version: None,
        sampled_at_epoch_ms: None,
        failure,
        upload_bps: 0.0,
        download_bps: 0.0,
        active_connections: 0,
        watchdog,
    }
}

/// The running session the owner measures at read time.
///
/// The owner is the only reader of the wall clock: the domain state carries
/// the instant the session began, and the readback carries the length the
/// owner measured when it published it.
fn active_session(started_at_epoch_ms: u64, now_epoch_ms: u64) -> CoreSessionObservation {
    CoreSessionObservation::Active {
        started_at_epoch_ms,
        elapsed_ms: now_epoch_ms.saturating_sub(started_at_epoch_ms),
    }
}

/// The wall clock the Core owner reads.
///
/// The owner is the only place that reads it: the session start is stamped
/// here, and the session length is measured here. No surface reads a clock
/// of its own.
pub(super) fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
