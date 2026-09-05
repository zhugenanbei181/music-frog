use serde::{Deserialize, Serialize};

use infiltrator_contract::session::SessionToken;

/// The lifecycle state of the mihomo application domain.
///
/// The state machine stores only business facts. Process handles, futures,
/// timers, and controller clients belong to an adapter/application layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreState {
    Idle {
        generation: u64,
    },
    Starting {
        generation: u64,
        session_token: SessionToken,
    },
    Running {
        generation: u64,
        session_token: SessionToken,
        endpoint: String,
    },
    Reloading {
        generation: u64,
        session_token: SessionToken,
        endpoint: String,
    },
    Stopping {
        generation: u64,
        session_token: SessionToken,
    },
    Failed {
        generation: u64,
        session_token: SessionToken,
        error: String,
    },
}

/// Domain events that drive [`CoreStateMachine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreEvent {
    StartRequested {
        session_token: SessionToken,
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
        endpoint: String,
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
            (CoreState::Idle { generation }, CoreEvent::StartRequested { session_token }) => (
                CoreState::Starting {
                    generation: generation.saturating_add(1).max(1),
                    session_token,
                },
                None,
            ),
            (CoreState::Failed { generation, .. }, CoreEvent::StartRequested { session_token }) => {
                (
                    CoreState::Starting {
                        generation: generation.saturating_add(1).max(1),
                        session_token,
                    },
                    None,
                )
            }
            (
                CoreState::Starting {
                    generation,
                    session_token,
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
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
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
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
                },
                CoreEvent::ReadinessTimeout {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Failed {
                    generation: *generation,
                    session_token: *session_token,
                    error: "Readiness probe timed out".to_string(),
                },
                None,
            ),
            (
                CoreState::Running {
                    generation,
                    session_token,
                    endpoint,
                },
                CoreEvent::ReloadRequested {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Reloading {
                    generation: *generation,
                    session_token: *session_token,
                    endpoint: endpoint.clone(),
                },
                None,
            ),
            (
                CoreState::Reloading {
                    generation,
                    session_token,
                    ..
                },
                CoreEvent::ReloadSuccess {
                    session_token: event_token,
                    endpoint,
                },
            ) if session_token == &event_token => (
                CoreState::Running {
                    generation: *generation,
                    session_token: *session_token,
                    endpoint,
                },
                None,
            ),
            (
                CoreState::Reloading {
                    generation,
                    session_token,
                    endpoint,
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
                },
                Some(error),
            ),
            (
                CoreState::Running {
                    generation,
                    session_token,
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
                },
                None,
            ),
            (
                CoreState::Starting {
                    generation,
                    session_token,
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
                | CoreState::Failed {
                    generation,
                    session_token,
                    ..
                },
                CoreEvent::StopRequested {
                    session_token: event_token,
                },
            ) if session_token == &event_token => (
                CoreState::Stopping {
                    generation: *generation,
                    session_token: *session_token,
                },
                None,
            ),
            (
                CoreState::Stopping {
                    generation,
                    session_token,
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

    fn token(value: u128) -> SessionToken {
        SessionToken::new(value)
    }

    #[test]
    fn full_lifecycle_success() {
        let mut state = CoreState::Idle { generation: 0 };
        assert!(CoreStateMachine::verify_invariants(&state));

        let session_token = token(1);
        (state, _) = CoreStateMachine::step(&state, CoreEvent::StartRequested { session_token });
        assert_eq!(
            state,
            CoreState::Starting {
                generation: 1,
                session_token,
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
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::ReloadRequested { session_token });
        assert_eq!(
            state,
            CoreState::Reloading {
                generation: 1,
                session_token,
                endpoint: "http://127.0.0.1:8080".to_string(),
            }
        );

        (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::ReloadSuccess {
                session_token,
                endpoint: "http://127.0.0.1:8081".to_string(),
            },
        );
        assert_eq!(
            state,
            CoreState::Running {
                generation: 1,
                session_token,
                endpoint: "http://127.0.0.1:8081".to_string(),
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::StopRequested { session_token });
        assert_eq!(
            state,
            CoreState::Stopping {
                generation: 1,
                session_token,
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
            },
        );
        assert_eq!(
            state,
            CoreState::Starting {
                generation: 2,
                session_token: new_token,
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
            }
        );

        let (state, _) = CoreStateMachine::step(&state, CoreEvent::StopRequested { session_token });
        assert_eq!(
            state,
            CoreState::Stopping {
                generation: 7,
                session_token,
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
        }));
    }
}
