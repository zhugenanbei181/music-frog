use lurker_core_contract::session::SessionToken;
use serde::{Deserialize, Serialize};

/// The lifecycle state of the mihomo application domain.
///
/// The state machine stores only business facts. Process handles, futures,
/// timers, and controller clients belong to an adapter/application layer.
///
/// Every session-bearing state carries the instant the session began. The
/// session start is a session fact, not a presentation detail: the owner
/// stamps it once when it starts the session, it travels with the session
/// token through every later transition, and it disappears with the session.
/// A surface can therefore measure how long the current session has been
/// running without inferring a start from a lifecycle stage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreState {
    Idle {
        generation: u64,
    },
    Starting {
        generation: u64,
        session_token: SessionToken,
        started_at_epoch_ms: u64,
    },
    Running {
        generation: u64,
        session_token: SessionToken,
        endpoint: String,
        started_at_epoch_ms: u64,
    },
    Reloading {
        generation: u64,
        session_token: SessionToken,
        endpoint: String,
        started_at_epoch_ms: u64,
    },
    Stopping {
        generation: u64,
        session_token: SessionToken,
        started_at_epoch_ms: u64,
    },
    Failed {
        generation: u64,
        session_token: SessionToken,
        error: String,
        started_at_epoch_ms: u64,
    },
}

/// Domain events that drive [`CoreStateMachine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreEvent {
    StartRequested {
        session_token: SessionToken,
        /// The instant the owner begins the session. The owner reads the
        /// clock; the reducer only carries the fact.
        started_at_epoch_ms: u64,
    },
    StartFailed {
        session_token: SessionToken,
        error: String,
    },
    ReadinessSuccess {
        session_token: SessionToken,
        endpoint: String,
    },
    ReadinessTimeout {
        session_token: SessionToken,
    },
    ProcessExitedUnexpectedly {
        session_token: SessionToken,
        error: String,
    },
    ReloadRequested {
        session_token: SessionToken,
    },
    ReloadSuccess {
        session_token: SessionToken,
    },
    ReloadFailed {
        session_token: SessionToken,
        error: String,
    },
    StopRequested {
        session_token: SessionToken,
    },
    StopCompleted {
        session_token: SessionToken,
    },
    StopFailed {
        session_token: SessionToken,
        error: String,
    },
}

/// Pure reducer for the mihomo lifecycle state.
pub struct CoreStateMachine;

impl CoreStateMachine {
    /// Progresses the state machine given the current state and an event.
    /// Returns the new state and an optional warning/effect string.
    pub fn step(state: &CoreState, event: CoreEvent) -> (CoreState, Option<String>) {
        match (state, event) {
            (
                CoreState::Idle { generation },
                CoreEvent::StartRequested {
                    session_token,
                    started_at_epoch_ms,
                },
            ) => (
                CoreState::Starting {
                    generation: generation.saturating_add(1).max(1),
                    session_token,
                    started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Failed { generation, .. },
                CoreEvent::StartRequested {
                    session_token,
                    started_at_epoch_ms,
                },
            ) => (
                CoreState::Starting {
                    generation: generation.saturating_add(1).max(1),
                    session_token,
                    started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                },
                CoreEvent::ReadinessSuccess {
                    session_token: event_token,
                    endpoint,
                },
            ) if session_token == &event_token => (
                CoreState::Running {
                    generation: *generation,
                    session_token: *session_token,
                    endpoint,
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                },
                CoreEvent::StartFailed {
                    session_token: event_token,
                    error,
                },
            ) if session_token == &event_token => (
                CoreState::Failed {
                    generation: *generation,
                    session_token: *session_token,
                    error,
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                },
                CoreEvent::ReadinessTimeout {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Failed {
                    generation: *generation,
                    session_token: *session_token,
                    error: "Readiness probe timed out".to_string(),
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Running {
                    generation,
                    session_token,
                    endpoint,
                    started_at_epoch_ms,
                },
                CoreEvent::ReloadRequested {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Reloading {
                    generation: *generation,
                    session_token: *session_token,
                    endpoint: endpoint.clone(),
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Reloading {
                    generation,
                    session_token,
                    endpoint,
                    started_at_epoch_ms,
                },
                CoreEvent::ReloadSuccess {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Running {
                    generation: *generation,
                    session_token: *session_token,
                    endpoint: endpoint.clone(),
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Reloading {
                    generation,
                    session_token,
                    endpoint,
                    started_at_epoch_ms,
                },
                CoreEvent::ReloadFailed {
                    session_token: event_token,
                    error,
                },
            ) if session_token == &event_token => (
                CoreState::Running {
                    generation: *generation,
                    session_token: *session_token,
                    endpoint: endpoint.clone(),
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                Some(error),
            ),
            (
                CoreState::Running {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                    ..
                },
                CoreEvent::ProcessExitedUnexpectedly {
                    session_token: event_token,
                    error,
                },
            ) if session_token == &event_token => (
                CoreState::Failed {
                    generation: *generation,
                    session_token: *session_token,
                    error,
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                }
                | CoreState::Running {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                    ..
                }
                | CoreState::Reloading {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                    ..
                }
                | CoreState::Failed {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                    ..
                },
                CoreEvent::StopRequested {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Stopping {
                    generation: *generation,
                    session_token: *session_token,
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (
                CoreState::Stopping {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                },
                CoreEvent::StopCompleted {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Idle {
                    generation: *generation,
                },
                None,
            ),
            (
                CoreState::Stopping {
                    generation,
                    session_token,
                    started_at_epoch_ms,
                },
                CoreEvent::StopFailed {
                    session_token: event_token,
                    error,
                },
            ) if session_token == &event_token => (
                CoreState::Failed {
                    generation: *generation,
                    session_token: *session_token,
                    error,
                    started_at_epoch_ms: *started_at_epoch_ms,
                },
                None,
            ),
            (current_state, _) => (current_state.clone(), None),
        }
    }

    /// Verifies the invariant that every non-idle generation is non-zero.
    pub fn verify_invariants(state: &CoreState) -> bool {
        match state {
            CoreState::Idle { .. } => true,
            CoreState::Starting {
                generation,
                session_token,
                started_at_epoch_ms: _,
            }
            | CoreState::Running {
                generation,
                session_token,
                ..
            }
            | CoreState::Reloading {
                generation,
                session_token,
                ..
            }
            | CoreState::Stopping {
                generation,
                session_token,
                started_at_epoch_ms: _,
            }
            | CoreState::Failed {
                generation,
                session_token,
                ..
            } => *generation > 0 && session_token.is_valid(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One fixed session start, so every assertion can name the instant the
    /// owner stamped.
    const SESSION_START: u64 = 1_700_000_000_000;

    fn token(value: u128) -> SessionToken {
        SessionToken::new(value)
    }

    #[test]
    fn full_lifecycle_success() {
        let mut state = CoreState::Idle { generation: 0 };
        assert!(CoreStateMachine::verify_invariants(&state));

        let session_token = token(1);
        (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::StartRequested {
                session_token,
                started_at_epoch_ms: SESSION_START,
            },
        );
        assert_eq!(
            state,
            CoreState::Starting {
                generation: 1,
                session_token,

                started_at_epoch_ms: SESSION_START,
            }
        );

        (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::ReadinessSuccess {
                session_token,
                endpoint: "http://127.0.0.1:8080".to_string(),
            },
        );
        assert_eq!(
            state,
            CoreState::Running {
                generation: 1,
                session_token,
                endpoint: "http://127.0.0.1:8080".to_string(),

                started_at_epoch_ms: SESSION_START,
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::ReloadRequested { session_token });
        assert_eq!(
            state,
            CoreState::Reloading {
                generation: 1,
                session_token,
                endpoint: "http://127.0.0.1:8080".to_string(),

                started_at_epoch_ms: SESSION_START,
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::ReloadSuccess { session_token });
        assert_eq!(
            state,
            CoreState::Running {
                generation: 1,
                session_token,
                endpoint: "http://127.0.0.1:8080".to_string(),

                started_at_epoch_ms: SESSION_START,
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::StopRequested { session_token });
        assert_eq!(
            state,
            CoreState::Stopping {
                generation: 1,
                session_token,

                started_at_epoch_ms: SESSION_START,
            }
        );
        (state, _) = CoreStateMachine::step(&state, CoreEvent::StopCompleted { session_token });
        assert_eq!(state, CoreState::Idle { generation: 1 });
        assert!(CoreStateMachine::verify_invariants(&state));
    }

    #[test]
    fn failed_generation_can_restart() {
        let old_token = token(1);
        let new_token = token(2);
        let state = CoreState::Starting {
            generation: 1,
            session_token: old_token,

            started_at_epoch_ms: SESSION_START,
        };
        let (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::ReadinessTimeout {
                session_token: old_token,
            },
        );
        assert!(matches!(
            state,
            CoreState::Failed {
                generation: 1,
                session_token: token,
                ..
            } if token == old_token
        ));

        let (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::StartRequested {
                session_token: new_token,
                started_at_epoch_ms: SESSION_START + 1,
            },
        );
        assert_eq!(
            state,
            CoreState::Starting {
                generation: 2,
                session_token: new_token,
                started_at_epoch_ms: SESSION_START + 1,
            }
        );
        assert!(CoreStateMachine::verify_invariants(&state));
    }

    #[test]
    fn reload_failure_retains_running_state_and_reports_warning() {
        let state = CoreState::Reloading {
            generation: 5,
            session_token: token(5),
            endpoint: "http://127.0.0.1:8080".to_string(),

            started_at_epoch_ms: SESSION_START,
        };
        let (state, warning) = CoreStateMachine::step(
            &state,
            CoreEvent::ReloadFailed {
                session_token: token(5),
                error: "invalid config".to_string(),
            },
        );
        assert_eq!(
            state,
            CoreState::Running {
                generation: 5,
                session_token: token(5),
                endpoint: "http://127.0.0.1:8080".to_string(),

                started_at_epoch_ms: SESSION_START,
            }
        );
        assert_eq!(warning.as_deref(), Some("invalid config"));
    }

    #[test]
    fn start_and_stop_failures_keep_the_current_generation() {
        let session_token = token(7);
        let state = CoreState::Starting {
            generation: 7,
            session_token,

            started_at_epoch_ms: SESSION_START,
        };
        let (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::StartFailed {
                session_token,
                error: "process exited".to_string(),
            },
        );
        assert_eq!(
            state,
            CoreState::Failed {
                generation: 7,
                session_token,
                error: "process exited".to_string(),

                started_at_epoch_ms: SESSION_START,
            }
        );

        let (state, _) = CoreStateMachine::step(&state, CoreEvent::StopRequested { session_token });
        assert_eq!(
            state,
            CoreState::Stopping {
                generation: 7,
                session_token,

                started_at_epoch_ms: SESSION_START,
            }
        );

        let (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::StopFailed {
                session_token,
                error: "permission denied".to_string(),
            },
        );
        assert_eq!(
            state,
            CoreState::Failed {
                generation: 7,
                session_token,
                error: "permission denied".to_string(),

                started_at_epoch_ms: SESSION_START,
            }
        );
    }

    #[test]
    fn unexpected_events_are_noops() {
        let state = CoreState::Idle { generation: 0 };
        let (next, effect) = CoreStateMachine::step(
            &state,
            CoreEvent::StopCompleted {
                session_token: token(1),
            },
        );
        assert_eq!(next, state);
        assert_eq!(effect, None);
    }

    #[test]
    fn stale_session_events_cannot_advance_the_state_machine() {
        let current_token = token(10);
        let stale_token = token(9);
        let state = CoreState::Starting {
            generation: 3,
            session_token: current_token,

            started_at_epoch_ms: SESSION_START,
        };
        let (next, effect) = CoreStateMachine::step(
            &state,
            CoreEvent::ReadinessSuccess {
                session_token: stale_token,
                endpoint: "http://127.0.0.1:9090".to_string(),
            },
        );
        assert_eq!(next, state);
        assert_eq!(effect, None);
        assert!(CoreStateMachine::verify_invariants(&next));
    }

    #[test]
    fn active_state_requires_a_nonzero_session_token() {
        assert!(!CoreStateMachine::verify_invariants(&CoreState::Starting {
            generation: 1,
            session_token: SessionToken::ZERO,
            started_at_epoch_ms: SESSION_START,
        }));
    }

    #[test]
    fn the_session_start_travels_with_the_session_and_ends_with_it() {
        let session_token = token(11);
        let (starting, _) = CoreStateMachine::step(
            &CoreState::Idle { generation: 0 },
            CoreEvent::StartRequested {
                session_token,
                started_at_epoch_ms: SESSION_START,
            },
        );
        let (running, _) = CoreStateMachine::step(
            &starting,
            CoreEvent::ReadinessSuccess {
                session_token,
                endpoint: "http://127.0.0.1:8080".to_string(),
            },
        );
        let (reloading, _) =
            CoreStateMachine::step(&running, CoreEvent::ReloadRequested { session_token });
        let (running_again, _) =
            CoreStateMachine::step(&reloading, CoreEvent::ReloadSuccess { session_token });
        let (stopping, _) =
            CoreStateMachine::step(&running_again, CoreEvent::StopRequested { session_token });

        for state in [&starting, &running, &reloading, &running_again, &stopping] {
            let started_at = match state {
                CoreState::Starting {
                    started_at_epoch_ms,
                    ..
                }
                | CoreState::Running {
                    started_at_epoch_ms,
                    ..
                }
                | CoreState::Reloading {
                    started_at_epoch_ms,
                    ..
                }
                | CoreState::Stopping {
                    started_at_epoch_ms,
                    ..
                } => *started_at_epoch_ms,
                CoreState::Idle { .. } | CoreState::Failed { .. } => {
                    panic!("a session state must carry the session start")
                }
            };
            assert_eq!(started_at, SESSION_START);
        }

        let (idle, _) =
            CoreStateMachine::step(&stopping, CoreEvent::StopCompleted { session_token });
        assert_eq!(idle, CoreState::Idle { generation: 1 });
    }

    #[test]
    fn a_restart_stamps_a_new_session_start() {
        let (starting, _) = CoreStateMachine::step(
            &CoreState::Idle { generation: 0 },
            CoreEvent::StartRequested {
                session_token: token(1),
                started_at_epoch_ms: SESSION_START,
            },
        );
        let (failed, _) = CoreStateMachine::step(
            &starting,
            CoreEvent::StartFailed {
                session_token: token(1),
                error: "process exited".to_string(),
            },
        );
        let (restarted, _) = CoreStateMachine::step(
            &failed,
            CoreEvent::StartRequested {
                session_token: token(2),
                started_at_epoch_ms: SESSION_START + 60_000,
            },
        );
        assert_eq!(
            restarted,
            CoreState::Starting {
                generation: 2,
                session_token: token(2),
                started_at_epoch_ms: SESSION_START + 60_000,
            }
        );
    }
}
