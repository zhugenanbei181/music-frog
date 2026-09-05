//! Pure crash-watchdog policy for a managed core session.
//!
//! This module owns only deterministic recovery decisions. Timers, process
//! status probes and the actual restart belong to the application/host layer;
//! the policy therefore remains usable by desktop, Android, iOS and any
//! future native surface without selecting an executor.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreWatchdogSnapshot, CoreWatchdogState};
use std::time::Duration;

/// Recovery policy for one application's crash loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchdogConfig {
    /// First retry delay. The default is short enough that a detected exit is
    /// recovered well inside the three-second user-visible outage budget.
    pub initial_retry_delay: Duration,
    /// Exponential backoff ceiling.
    pub max_retry_delay: Duration,
    /// Maximum restart attempts before the circuit opens.
    pub max_restart_attempts: u32,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(3),
            max_restart_attempts: 3,
        }
    }
}

impl WatchdogConfig {
    pub fn delay_for_attempt(self, attempt: u32) -> Duration {
        let attempt = attempt.max(1);
        let shift = attempt.saturating_sub(1).min(31);
        let multiplier = 1_u32 << shift;
        self.initial_retry_delay
            .checked_mul(multiplier)
            .unwrap_or(self.max_retry_delay)
            .min(self.max_retry_delay)
    }
}

/// The next action the application/host should perform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchdogDecision {
    Ignore,
    Retry {
        session_token: SessionToken,
        attempt: u32,
        delay: Duration,
    },
    Trip {
        session_token: SessionToken,
        attempts: u32,
    },
}

/// Validation failures from a delayed recovery attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchdogTransitionError {
    StaleSession,
    InvalidState,
}

impl std::fmt::Display for WatchdogTransitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleSession => formatter.write_str("stale watchdog session token"),
            Self::InvalidState => formatter.write_str("invalid watchdog state transition"),
        }
    }
}

impl std::error::Error for WatchdogTransitionError {}

/// Deterministic state machine backing the application watchdog.
#[derive(Clone, Debug)]
pub struct WatchdogMachine {
    config: WatchdogConfig,
    snapshot: CoreWatchdogSnapshot,
}

impl Default for WatchdogMachine {
    fn default() -> Self {
        Self::new(WatchdogConfig::default())
    }
}

impl WatchdogMachine {
    pub fn new(config: WatchdogConfig) -> Self {
        Self {
            config,
            snapshot: CoreWatchdogSnapshot::default(),
        }
    }

    pub fn config(&self) -> WatchdogConfig {
        self.config
    }

    pub fn snapshot(&self) -> CoreWatchdogSnapshot {
        self.snapshot.clone()
    }

    /// Record a process exit and schedule the first recovery attempt. A
    /// duplicate exit notification while a retry is already in flight is a
    /// no-op, so two host observers cannot create parallel restart loops.
    pub fn process_exited(
        &mut self,
        session_token: SessionToken,
        error: impl Into<String>,
    ) -> WatchdogDecision {
        if !session_token.is_valid()
            || matches!(
                self.snapshot.state,
                CoreWatchdogState::Waiting { .. }
                    | CoreWatchdogState::Restarting { .. }
                    | CoreWatchdogState::Tripped { .. }
            )
            || self
                .snapshot
                .session_token
                .is_some_and(|active| active != session_token)
        {
            return WatchdogDecision::Ignore;
        }

        let attempt = 1;
        self.snapshot.session_token = Some(session_token);
        self.snapshot.consecutive_failures = attempt;
        self.snapshot.last_error = Some(Failure::new(ErrorCode::Internal, error, true));
        if self.config.max_restart_attempts == 0 {
            self.snapshot.state = CoreWatchdogState::Tripped { attempts: 0 };
            return WatchdogDecision::Trip {
                session_token,
                attempts: 0,
            };
        }

        let delay = self.config.delay_for_attempt(attempt);
        self.snapshot.state = CoreWatchdogState::Waiting {
            attempt,
            retry_in_ms: duration_millis(delay),
        };
        WatchdogDecision::Retry {
            session_token,
            attempt,
            delay,
        }
    }

    pub fn update_retry_remaining(
        &mut self,
        session_token: SessionToken,
        attempt: u32,
        retry_in: Duration,
    ) -> Result<(), WatchdogTransitionError> {
        self.check_waiting(session_token, attempt)?;
        self.snapshot.state = CoreWatchdogState::Waiting {
            attempt,
            retry_in_ms: duration_millis(retry_in),
        };
        Ok(())
    }

    pub fn begin_restart(
        &mut self,
        session_token: SessionToken,
        attempt: u32,
    ) -> Result<(), WatchdogTransitionError> {
        self.check_waiting(session_token, attempt)?;
        self.snapshot.state = CoreWatchdogState::Restarting { attempt };
        Ok(())
    }

    pub fn restart_succeeded(
        &mut self,
        previous_session: SessionToken,
        current_session: SessionToken,
        attempt: u32,
    ) -> Result<(), WatchdogTransitionError> {
        if self.snapshot.session_token != Some(previous_session)
            || !current_session.is_valid()
            || current_session == previous_session
            || !matches!(
                self.snapshot.state,
                CoreWatchdogState::Restarting {
                    attempt: active_attempt
                } if active_attempt == attempt
            )
        {
            return Err(if self.snapshot.session_token != Some(previous_session) {
                WatchdogTransitionError::StaleSession
            } else {
                WatchdogTransitionError::InvalidState
            });
        }
        self.snapshot.session_token = Some(current_session);
        self.snapshot.state = CoreWatchdogState::Recovered { attempts: attempt };
        self.snapshot.consecutive_failures = 0;
        self.snapshot.last_error = None;
        Ok(())
    }

    pub fn restart_failed(
        &mut self,
        previous_session: SessionToken,
        current_session: SessionToken,
        attempt: u32,
        error: impl Into<String>,
    ) -> Result<WatchdogDecision, WatchdogTransitionError> {
        if self.snapshot.session_token != Some(previous_session)
            || !current_session.is_valid()
            || !matches!(
                self.snapshot.state,
                CoreWatchdogState::Restarting {
                    attempt: active_attempt
                } if active_attempt == attempt
            )
        {
            return Err(if self.snapshot.session_token != Some(previous_session) {
                WatchdogTransitionError::StaleSession
            } else {
                WatchdogTransitionError::InvalidState
            });
        }

        let next_attempt = attempt.saturating_add(1);
        self.snapshot.session_token = Some(current_session);
        self.snapshot.consecutive_failures = attempt;
        self.snapshot.last_error = Some(Failure::new(ErrorCode::Internal, error, true));
        if next_attempt > self.config.max_restart_attempts {
            self.snapshot.state = CoreWatchdogState::Tripped { attempts: attempt };
            return Ok(WatchdogDecision::Trip {
                session_token: current_session,
                attempts: attempt,
            });
        }

        let delay = self.config.delay_for_attempt(next_attempt);
        self.snapshot.state = CoreWatchdogState::Waiting {
            attempt: next_attempt,
            retry_in_ms: duration_millis(delay),
        };
        self.snapshot.consecutive_failures = next_attempt;
        Ok(WatchdogDecision::Retry {
            session_token: current_session,
            attempt: next_attempt,
            delay,
        })
    }

    /// Cancel a stale recovery loop after a manual stop or a new user-started
    /// session. This is intentionally explicit so an old retry cannot revive
    /// a core after the user has turned it off.
    pub fn reset(&mut self) {
        self.snapshot = CoreWatchdogSnapshot::default();
    }

    fn check_waiting(
        &self,
        session_token: SessionToken,
        attempt: u32,
    ) -> Result<(), WatchdogTransitionError> {
        if self.snapshot.session_token != Some(session_token) {
            return Err(WatchdogTransitionError::StaleSession);
        }
        if !matches!(
            self.snapshot.state,
            CoreWatchdogState::Waiting {
                attempt: active_attempt,
                ..
            } if active_attempt == attempt
        ) {
            return Err(WatchdogTransitionError::InvalidState);
        }
        Ok(())
    }
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(value: u128) -> SessionToken {
        SessionToken::new(value)
    }

    #[test]
    fn default_policy_starts_fast_and_backs_off_exponentially() {
        let config = WatchdogConfig::default();
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(400));
        assert!(config.delay_for_attempt(8) <= config.max_retry_delay);
    }

    #[test]
    fn crash_recovery_trips_after_restart_failures() {
        let mut machine = WatchdogMachine::new(WatchdogConfig {
            initial_retry_delay: Duration::from_millis(10),
            max_retry_delay: Duration::from_secs(1),
            max_restart_attempts: 2,
        });
        let old = token(1);
        let first = machine.process_exited(old, "exit");
        assert!(matches!(first, WatchdogDecision::Retry { attempt: 1, .. }));
        machine.begin_restart(old, 1).unwrap();
        let second = machine
            .restart_failed(old, token(2), 1, "boot failed")
            .unwrap();
        assert!(matches!(second, WatchdogDecision::Retry { attempt: 2, .. }));
        machine.begin_restart(token(2), 2).unwrap();
        let third = machine
            .restart_failed(token(2), token(3), 2, "boot failed again")
            .unwrap();
        assert_eq!(
            third,
            WatchdogDecision::Trip {
                session_token: token(3),
                attempts: 2,
            }
        );
        assert!(matches!(
            machine.snapshot().state,
            CoreWatchdogState::Tripped { attempts: 2 }
        ));
        assert_eq!(
            machine.process_exited(token(3), "crash loop still active"),
            WatchdogDecision::Ignore
        );
    }

    #[test]
    fn successful_restart_switches_session_and_clears_failure() {
        let mut machine = WatchdogMachine::default();
        let old = token(4);
        machine.process_exited(old, "crash");
        machine.begin_restart(old, 1).unwrap();
        machine.restart_succeeded(old, token(5), 1).unwrap();
        let snapshot = machine.snapshot();
        assert_eq!(snapshot.session_token, Some(token(5)));
        assert_eq!(snapshot.consecutive_failures, 0);
        assert_eq!(snapshot.last_error, None);
        assert!(matches!(
            snapshot.state,
            CoreWatchdogState::Recovered { attempts: 1 }
        ));
    }

    #[test]
    fn duplicate_or_stale_notifications_are_ignored_or_rejected() {
        let mut machine = WatchdogMachine::default();
        let current = token(6);
        let stale = token(5);
        assert_eq!(
            machine.process_exited(SessionToken::ZERO, "invalid"),
            WatchdogDecision::Ignore
        );
        assert!(matches!(
            machine.process_exited(current, "crash"),
            WatchdogDecision::Retry { .. }
        ));
        assert_eq!(
            machine.process_exited(current, "duplicate"),
            WatchdogDecision::Ignore
        );
        assert_eq!(
            machine.update_retry_remaining(stale, 1, Duration::ZERO),
            Err(WatchdogTransitionError::StaleSession)
        );
    }
}
