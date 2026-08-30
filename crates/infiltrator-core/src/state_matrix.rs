use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreState {
    Idle,
    Starting { generation: u64 },
    Running { generation: u64, endpoint: String },
    Reloading { generation: u64 },
    Stopping,
    Failed { generation: u64, error: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreEvent {
    StartRequested,
    ReadinessSuccess(String),
    ReadinessTimeout,
    ProcessExitedUnexpectedly(String),
    ReloadRequested,
    ReloadSuccess,
    ReloadFailed(String),
    StopRequested,
    StopCompleted,
}

pub struct CoreStateMachine;

impl CoreStateMachine {
    /// Progresses the state machine given the current state and an event.
    /// Returns the new state and an optional warning/effect string.
    pub fn step(state: &CoreState, event: CoreEvent) -> (CoreState, Option<String>) {
        match (state, event) {
            // StartRequested
            (CoreState::Idle, CoreEvent::StartRequested) => {
                (CoreState::Starting { generation: 1 }, None)
            }
            (CoreState::Failed { generation, .. }, CoreEvent::StartRequested) => {
                (CoreState::Starting { generation: generation + 1 }, None)
            }

            // Readiness
            (CoreState::Starting { generation }, CoreEvent::ReadinessSuccess(ep)) => {
                (CoreState::Running { generation: *generation, endpoint: ep }, None)
            }
            (CoreState::Starting { generation }, CoreEvent::ReadinessTimeout) => {
                (CoreState::Failed {
                    generation: *generation,
                    error: "Readiness probe timed out".to_string(),
                }, None)
            }

            // ReloadRequested
            (CoreState::Running { generation, .. }, CoreEvent::ReloadRequested) => {
                (CoreState::Reloading { generation: *generation }, None)
            }

            // Reload outcomes
            (CoreState::Reloading { generation }, CoreEvent::ReloadSuccess) => {
                (CoreState::Running {
                    generation: *generation,
                    endpoint: "reloaded_endpoint".to_string(),
                }, None)
            }
            (CoreState::Reloading { generation }, CoreEvent::ReloadFailed(err)) => {
                (CoreState::Running {
                    generation: *generation,
                    endpoint: "retained_endpoint".to_string(),
                }, Some(err))
            }

            // Unexpected exit
            (CoreState::Running { generation, .. }, CoreEvent::ProcessExitedUnexpectedly(err)) => {
                (CoreState::Failed { generation: *generation, error: err }, None)
            }

            // Stop transitions
            (CoreState::Starting { .. }, CoreEvent::StopRequested) |
            (CoreState::Running { .. }, CoreEvent::StopRequested) |
            (CoreState::Reloading { .. }, CoreEvent::StopRequested) |
            (CoreState::Failed { .. }, CoreEvent::StopRequested) => {
                (CoreState::Stopping, None)
            }

            (CoreState::Stopping, CoreEvent::StopCompleted) => {
                (CoreState::Idle, None)
            }

            // Ignore unexpected transitions (no-op)
            (current_state, _) => (current_state.clone(), None),
        }
    }

    /// Verifies invariants of the state machine.
    /// Returns true if invariants hold, false otherwise.
    pub fn verify_invariants(state: &CoreState) -> bool {
        match state {
            CoreState::Idle | CoreState::Stopping => true,
            CoreState::Starting { generation } |
            CoreState::Running { generation, .. } |
            CoreState::Reloading { generation } |
            CoreState::Failed { generation, .. } => *generation > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_lifecycle_success() {
        let mut state = CoreState::Idle;
        assert!(CoreStateMachine::verify_invariants(&state));

        // Start
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::StartRequested);
        assert_eq!(new_state, CoreState::Starting { generation: 1 });
        assert_eq!(effect, None);
        state = new_state;
        assert!(CoreStateMachine::verify_invariants(&state));

        // Readiness success
        let ep = "http://127.0.0.1:8080".to_string();
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::ReadinessSuccess(ep.clone()));
        assert_eq!(new_state, CoreState::Running { generation: 1, endpoint: ep });
        assert_eq!(effect, None);
        state = new_state;

        // Reload requested
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::ReloadRequested);
        assert_eq!(new_state, CoreState::Reloading { generation: 1 });
        assert_eq!(effect, None);
        state = new_state;

        // Reload success
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::ReloadSuccess);
        assert_eq!(new_state, CoreState::Running { generation: 1, endpoint: "reloaded_endpoint".to_string() });
        assert_eq!(effect, None);
        state = new_state;

        // Stop requested
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::StopRequested);
        assert_eq!(new_state, CoreState::Stopping);
        assert_eq!(effect, None);
        state = new_state;

        // Stop completed
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::StopCompleted);
        assert_eq!(new_state, CoreState::Idle);
        assert_eq!(effect, None);
        assert!(CoreStateMachine::verify_invariants(&new_state));
    }

    #[test]
    fn test_failure_and_retry_generation() {
        let state = CoreState::Starting { generation: 1 };
        
        // Readiness timeout
        let (state, effect) = CoreStateMachine::step(&state, CoreEvent::ReadinessTimeout);
        assert_eq!(state, CoreState::Failed { generation: 1, error: "Readiness probe timed out".to_string() });
        assert_eq!(effect, None);

        // Restart
        let (state, effect) = CoreStateMachine::step(&state, CoreEvent::StartRequested);
        assert_eq!(state, CoreState::Starting { generation: 2 });
        assert_eq!(effect, None);
        assert!(CoreStateMachine::verify_invariants(&state));

        // Start success
        let (state, _) = CoreStateMachine::step(&state, CoreEvent::ReadinessSuccess("ep".to_string()));
        assert_eq!(state, CoreState::Running { generation: 2, endpoint: "ep".to_string() });

        // Process exit
        let (state, effect) = CoreStateMachine::step(&state, CoreEvent::ProcessExitedUnexpectedly("segfault".to_string()));
        assert_eq!(state, CoreState::Failed { generation: 2, error: "segfault".to_string() });
        assert_eq!(effect, None);

        // Restart again
        let (state, _) = CoreStateMachine::step(&state, CoreEvent::StartRequested);
        assert_eq!(state, CoreState::Starting { generation: 3 });
        assert!(CoreStateMachine::verify_invariants(&state));
    }

    #[test]
    fn test_reload_failure() {
        let state = CoreState::Reloading { generation: 5 };
        let (state, effect) = CoreStateMachine::step(&state, CoreEvent::ReloadFailed("config error".to_string()));
        assert_eq!(state, CoreState::Running { generation: 5, endpoint: "retained_endpoint".to_string() });
        assert_eq!(effect, Some("config error".to_string()));
    }

    #[test]
    fn test_unexpected_transition() {
        let state = CoreState::Idle;
        // Sending StopCompleted to Idle should be a no-op
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::StopCompleted);
        assert_eq!(new_state, CoreState::Idle);
        assert_eq!(effect, None);

        let state = CoreState::Running { generation: 2, endpoint: "ep".to_string() };
        // Sending StartRequested to Running should be a no-op
        let (new_state, effect) = CoreStateMachine::step(&state, CoreEvent::StartRequested);
        assert_eq!(new_state, CoreState::Running { generation: 2, endpoint: "ep".to_string() });
        assert_eq!(effect, None);
    }
}
