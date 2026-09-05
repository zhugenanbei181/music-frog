//! Application adapter for the pure core crash-watchdog policy.
//!
//! The policy itself lives in `infiltrator-domain`. This module owns the
//! application state mirror, process probe and serialized restart transaction
//! so the main lifecycle module does not become a second monolith.

use super::{CoreApplication, CoreState};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreWatchdogSnapshot, CoreWatchdogState};
use infiltrator_domain::watchdog::{WatchdogConfig, WatchdogMachine};
use infiltrator_ports::core_watchdog::{CoreWatchdogPort, WatchdogTick};

#[derive(Default)]
pub(super) struct WatchdogRuntime {
    pub(super) machine: WatchdogMachine,
    pub(super) retry_at: Option<std::time::Instant>,
}

impl CoreApplication {
    /// Latest crash-recovery projection shared by Iced, Bevy and native
    /// hosts. It is a read-only view; only `watchdog_tick` mutates policy.
    pub fn watchdog_snapshot(&self) -> CoreWatchdogSnapshot {
        self.inner
            .watchdog
            .lock()
            .expect("core watchdog lock")
            .machine
            .snapshot()
    }

    /// Replace the watchdog policy before a host starts its monitor. This is
    /// primarily useful for platform-specific policy injection and bounded
    /// tests; changing it also cancels any pending retry.
    pub fn configure_watchdog(&self, config: WatchdogConfig) {
        let mut watchdog = self.inner.watchdog.lock().expect("core watchdog lock");
        watchdog.machine = WatchdogMachine::new(config);
        watchdog.retry_at = None;
    }

    pub(super) fn reset_watchdog_for_start(&self) {
        let mut watchdog = self.inner.watchdog.lock().expect("core watchdog lock");
        if !matches!(
            watchdog.machine.snapshot().state,
            CoreWatchdogState::Waiting { .. } | CoreWatchdogState::Restarting { .. }
        ) {
            watchdog.machine.reset();
            watchdog.retry_at = None;
        }
    }

    pub(super) fn reset_watchdog_after_stop(&self) {
        let should_publish_reset = self.watchdog_snapshot() != CoreWatchdogSnapshot::default();
        if should_publish_reset {
            let mut watchdog = self.inner.watchdog.lock().expect("core watchdog lock");
            watchdog.machine.reset();
            watchdog.retry_at = None;
            drop(watchdog);
            self.publish_watchdog_snapshot();
        }
    }

    fn publish_watchdog_snapshot(&self) {
        let snapshot = {
            let mut mirror = self.inner.state.write().expect("core state lock");
            mirror.revision = mirror.revision.saturating_add(1);
            let watchdog = self.watchdog_snapshot();
            super::snapshot_from_state(&mirror.state, mirror.revision, watchdog)
        };
        self.push_event(super::CoreEvent::SnapshotUpdated(snapshot));
    }

    async fn watchdog_tick_locked(
        &self,
    ) -> Result<WatchdogTick, infiltrator_ports::error::PortError> {
        let _operation = self.inner.operation.lock().await;
        let state = self.current_state();
        let Some(current_token) = state_session_token(&state) else {
            let should_reset = self.watchdog_snapshot() != CoreWatchdogSnapshot::default();
            if should_reset {
                let mut watchdog = self.inner.watchdog.lock().expect("core watchdog lock");
                watchdog.machine.reset();
                watchdog.retry_at = None;
                drop(watchdog);
                self.publish_watchdog_snapshot();
            }
            return Ok(WatchdogTick::Healthy);
        };

        if matches!(
            &state,
            CoreState::Starting { .. } | CoreState::Stopping { .. } | CoreState::Reloading { .. }
        ) {
            return Ok(WatchdogTick::Healthy);
        }

        let watchdog = self.watchdog_snapshot();
        if watchdog
            .session_token
            .is_some_and(|token| token != current_token)
        {
            let mut runtime = self.inner.watchdog.lock().expect("core watchdog lock");
            runtime.machine.reset();
            runtime.retry_at = None;
            drop(runtime);
            self.publish_watchdog_snapshot();
            return Ok(WatchdogTick::Healthy);
        }

        if let CoreWatchdogState::Tripped { attempts } = &watchdog.state {
            return Ok(WatchdogTick::Tripped {
                session_token: watchdog.session_token,
                attempts: *attempts,
            });
        }

        if let CoreWatchdogState::Waiting { attempt, .. } = &watchdog.state {
            let attempt = *attempt;
            let now = std::time::Instant::now();
            let retry_at = self
                .inner
                .watchdog
                .lock()
                .expect("core watchdog lock")
                .retry_at;
            if retry_at.is_some_and(|deadline| deadline > now) {
                let retry_in = retry_at
                    .expect("retry deadline")
                    .saturating_duration_since(now);
                return Ok(WatchdogTick::Waiting {
                    session_token: current_token,
                    attempt,
                    retry_in_ms: retry_in.as_millis().try_into().unwrap_or(u64::MAX),
                });
            }

            {
                let mut runtime = self.inner.watchdog.lock().expect("core watchdog lock");
                runtime
                    .machine
                    .begin_restart(current_token, attempt)
                    .map_err(|error| {
                        infiltrator_ports::error::PortError::Failed(error.to_string())
                    })?;
                runtime.retry_at = None;
            }
            self.publish_watchdog_snapshot();

            let previous_session = current_token;
            let restart = self.start_locked().await;
            match restart {
                Ok(()) => {
                    let current_session = self.session_token().ok_or_else(|| {
                        infiltrator_ports::error::PortError::Failed(
                            "watchdog restart completed without a session".to_string(),
                        )
                    })?;
                    let mut runtime = self.inner.watchdog.lock().expect("core watchdog lock");
                    runtime
                        .machine
                        .restart_succeeded(previous_session, current_session, attempt)
                        .map_err(|error| {
                            infiltrator_ports::error::PortError::Failed(error.to_string())
                        })?;
                    runtime.retry_at = None;
                    drop(runtime);
                    self.publish_watchdog_snapshot();
                    Ok(WatchdogTick::Recovered {
                        session_token: current_session,
                        attempts: attempt,
                    })
                }
                Err(failure) => {
                    let current_session = self.session_token().unwrap_or(previous_session);
                    let decision = {
                        let mut runtime = self.inner.watchdog.lock().expect("core watchdog lock");
                        let decision = runtime
                            .machine
                            .restart_failed(
                                previous_session,
                                current_session,
                                attempt,
                                failure.message.clone(),
                            )
                            .map_err(|error| {
                                infiltrator_ports::error::PortError::Failed(error.to_string())
                            })?;
                        runtime.retry_at = match &decision {
                            infiltrator_domain::watchdog::WatchdogDecision::Retry {
                                delay, ..
                            } => Some(std::time::Instant::now() + *delay),
                            infiltrator_domain::watchdog::WatchdogDecision::Trip { .. }
                            | infiltrator_domain::watchdog::WatchdogDecision::Ignore => None,
                        };
                        decision
                    };
                    self.publish_watchdog_snapshot();
                    decision_to_tick(decision)
                }
            }
        } else if matches!(&state, CoreState::Failed { .. }) {
            Ok(WatchdogTick::Healthy)
        } else {
            match self.inner.process.status().await? {
                super::CoreLifecycle::Starting
                | super::CoreLifecycle::Ready
                | super::CoreLifecycle::Running => Ok(WatchdogTick::Healthy),
                super::CoreLifecycle::Stopped
                | super::CoreLifecycle::Stopping
                | super::CoreLifecycle::Failed => {
                    let error = "managed mihomo process exited unexpectedly".to_string();
                    self.apply_domain_event(
                        infiltrator_domain::core_state::CoreEvent::ProcessExitedUnexpectedly {
                            session_token: current_token,
                            error: error.clone(),
                        },
                    );
                    let decision = {
                        let mut runtime = self.inner.watchdog.lock().expect("core watchdog lock");
                        let decision = runtime.machine.process_exited(current_token, error);
                        runtime.retry_at = match &decision {
                            infiltrator_domain::watchdog::WatchdogDecision::Retry {
                                delay, ..
                            } => Some(std::time::Instant::now() + *delay),
                            infiltrator_domain::watchdog::WatchdogDecision::Trip { .. }
                            | infiltrator_domain::watchdog::WatchdogDecision::Ignore => None,
                        };
                        decision
                    };
                    self.publish_watchdog_snapshot();
                    decision_to_tick(decision)
                }
            }
        }
    }
}

fn decision_to_tick(
    decision: infiltrator_domain::watchdog::WatchdogDecision,
) -> Result<WatchdogTick, infiltrator_ports::error::PortError> {
    match decision {
        infiltrator_domain::watchdog::WatchdogDecision::Retry {
            session_token,
            attempt,
            delay,
        } => Ok(WatchdogTick::Waiting {
            session_token,
            attempt,
            retry_in_ms: delay.as_millis().try_into().unwrap_or(u64::MAX),
        }),
        infiltrator_domain::watchdog::WatchdogDecision::Trip {
            session_token,
            attempts,
        } => Ok(WatchdogTick::Tripped {
            session_token: Some(session_token),
            attempts,
        }),
        infiltrator_domain::watchdog::WatchdogDecision::Ignore => Ok(WatchdogTick::Healthy),
    }
}

fn state_session_token(state: &CoreState) -> Option<SessionToken> {
    match state {
        CoreState::Idle { .. } => None,
        CoreState::Starting { session_token, .. }
        | CoreState::Running { session_token, .. }
        | CoreState::Reloading { session_token, .. }
        | CoreState::Stopping { session_token, .. }
        | CoreState::Failed { session_token, .. } => Some(*session_token),
    }
}

#[async_trait::async_trait]
impl CoreWatchdogPort for CoreApplication {
    fn watchdog_snapshot(&self) -> CoreWatchdogSnapshot {
        CoreApplication::watchdog_snapshot(self)
    }

    async fn watchdog_tick(&self) -> Result<WatchdogTick, infiltrator_ports::error::PortError> {
        self.watchdog_tick_locked().await
    }
}
