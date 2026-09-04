use serde::{Deserialize, Serialize};

/// The lifecycle state of the mihomo application domain.
///
/// The state machine stores only business facts. Process handles, futures,
/// timers, and controller clients belong to an adapter/application layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreState {
    Idle,
    Starting { generation: u64 },
    Running { generation: u64, endpoint: String },
    Reloading { generation: u64, endpoint: String },
    Stopping,
    Failed { generation: u64, error: String },
}

/// Domain events that drive [`CoreStateMachine`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreEvent {
    StartRequested,
    ReadinessSuccess(String),
    ReadinessTimeout,
    ProcessExitedUnexpectedly(String),
    ReloadRequested,
    ReloadSuccess(String),
    ReloadFailed(String),
    StopRequested,
    StopCompleted,
}

/// Pure reducer for the mihomo lifecycle state.
pub struct CoreStateMachine;

impl CoreStateMachine {
    /// Progresses the state machine given the current state and an event.
    /// Returns the new state and an optional warning/effect string.
    pub fn step(state: &CoreState, event: CoreEvent) -> (CoreState, Option<String>) {
        match (state, event) {
            (CoreState::Idle, CoreEvent::StartRequested) => {
                (CoreState::Starting { generation: 1 }, None)
            }
            (CoreState::Failed { generation, .. }, CoreEvent::StartRequested) => (
                CoreState::Starting {
                    generation: generation + 1,
                },
                None,
            ),
            (CoreState::Starting { generation }, CoreEvent::ReadinessSuccess(endpoint)) => (
                CoreState::Running {
                    generation: *generation,
                    endpoint,
                },
                None,
            ),
            (CoreState::Starting { generation }, CoreEvent::ReadinessTimeout) => (
                CoreState::Failed {
                    generation: *generation,
                    error: "Readiness probe timed out".to_string(),
                },
                None,
            ),
            (
                CoreState::Running {
                    generation,
                    endpoint,
                },
                CoreEvent::ReloadRequested,
            ) => (
                CoreState::Reloading {
                    generation: *generation,
                    endpoint: endpoint.clone(),
                },
                None,
            ),
            (CoreState::Reloading { generation, .. }, CoreEvent::ReloadSuccess(endpoint)) => (
                CoreState::Running {
                    generation: *generation,
                    endpoint,
                },
                None,
            ),
            (
                CoreState::Reloading {
                    generation,
                    endpoint,
                },
                CoreEvent::ReloadFailed(error),
            ) => (
                CoreState::Running {
                    generation: *generation,
                    endpoint: endpoint.clone(),
                },
                Some(error),
            ),
            (
                CoreState::Running { generation, .. },
                CoreEvent::ProcessExitedUnexpectedly(error),
            ) => (
                CoreState::Failed {
                    generation: *generation,
                    error,
                },
                None,
            ),
            (CoreState::Starting { .. }, CoreEvent::StopRequested)
            | (CoreState::Running { .. }, CoreEvent::StopRequested)
            | (CoreState::Reloading { .. }, CoreEvent::StopRequested)
            | (CoreState::Failed { .. }, CoreEvent::StopRequested) => (CoreState::Stopping, None),
            (CoreState::Stopping, CoreEvent::StopCompleted) => (CoreState::Idle, None),
            (current_state, _) => (current_state.clone(), None),
        }
    }

    /// Verifies the invariant that every non-idle generation is non-zero.
    pub fn verify_invariants(state: &CoreState) -> bool {
        match state {
            CoreState::Idle | CoreState::Stopping => true,
            CoreState::Starting { generation }
            | CoreState::Running { generation, .. }
            | CoreState::Reloading { generation, .. }
            | CoreState::Failed { generation, .. } => *generation > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_lifecycle_success() {
        let mut state = CoreState::Idle;
        assert!(CoreStateMachine::verify_invariants(&state));

        (state, _) = CoreStateMachine::step(&state, CoreEvent::StartRequested);
        assert_eq!(state, CoreState::Starting { generation: 1 });

        (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::ReadinessSuccess("http://127.0.0.1:8080".to_string()),
        );
        assert_eq!(
            state,
            CoreState::Running {
                generation: 1,
                endpoint: "http://127.0.0.1:8080".to_string(),
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::ReloadRequested);
        assert_eq!(
            state,
            CoreState::Reloading {
                generation: 1,
                endpoint: "http://127.0.0.1:8080".to_string(),
            }
        );

        (state, _) = CoreStateMachine::step(
            &state,
            CoreEvent::ReloadSuccess("http://127.0.0.1:8081".to_string()),
        );
        assert_eq!(
            state,
            CoreState::Running {
                generation: 1,
                endpoint: "http://127.0.0.1:8081".to_string(),
            }
        );

        (state, _) = CoreStateMachine::step(&state, CoreEvent::StopRequested);
        assert_eq!(state, CoreState::Stopping);
        (state, _) = CoreStateMachine::step(&state, CoreEvent::StopCompleted);
        assert_eq!(state, CoreState::Idle);
        assert!(CoreStateMachine::verify_invariants(&state));
    }

    #[test]
    fn failed_generation_can_restart() {
        let state = CoreState::Starting { generation: 1 };
        let (state, _) = CoreStateMachine::step(&state, CoreEvent::ReadinessTimeout);
        assert!(matches!(state, CoreState::Failed { generation: 1, .. }));

        let (state, _) = CoreStateMachine::step(&state, CoreEvent::StartRequested);
        assert_eq!(state, CoreState::Starting { generation: 2 });
        assert!(CoreStateMachine::verify_invariants(&state));
    }

    #[test]
    fn reload_failure_retains_running_state_and_reports_warning() {
        let state = CoreState::Reloading {
            generation: 5,
            endpoint: "http://127.0.0.1:8080".to_string(),
        };
        let (state, warning) = CoreStateMachine::step(
            &state,
            CoreEvent::ReloadFailed("invalid config".to_string()),
        );
        assert_eq!(
            state,
            CoreState::Running {
                generation: 5,
                endpoint: "http://127.0.0.1:8080".to_string(),
            }
        );
        assert_eq!(warning.as_deref(), Some("invalid config"));
    }

    #[test]
    fn unexpected_events_are_noops() {
        let state = CoreState::Idle;
        let (next, effect) = CoreStateMachine::step(&state, CoreEvent::StopCompleted);
        assert_eq!(next, state);
        assert_eq!(effect, None);
    }
}
