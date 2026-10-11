//! Correlated doctor command state shared by native adapters; telemetry does not complete writes.
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDoctorAction {
    pub token: u64,
    pub action: DoctorAction,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DoctorActionPhase {
    #[default]
    Idle,
    Pending(PendingDoctorAction),
    Failed {
        failure: Failure,
        retry_action: DoctorAction,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoctorActionFinish {
    Stale,
    Applied,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DoctorActionState {
    pub phase: DoctorActionPhase,
    next_token: u64,
}
impl DoctorActionState {
    /// Permission recovery is manual: the user retries after following the visible host error.
    pub fn can_retry(&self) -> bool {
        matches!(
            &self.phase,
            DoctorActionPhase::Failed { failure, .. }
                if failure.retryable || failure.code == ErrorCode::Permission
        )
    }
    pub fn begin(&mut self, action: DoctorAction) -> Result<PendingDoctorAction, Failure> {
        if matches!(&self.phase, DoctorActionPhase::Pending(_)) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "a diagnostic operation is already pending",
                true,
            ));
        }
        self.next_token += 1;
        let pending = PendingDoctorAction {
            token: self.next_token,
            action,
        };
        self.phase = DoctorActionPhase::Pending(pending.clone());
        Ok(pending)
    }
    pub fn begin_retry(&mut self) -> Result<PendingDoctorAction, Failure> {
        let action = match &self.phase {
            DoctorActionPhase::Failed {
                failure,
                retry_action,
            } if failure.retryable || failure.code == ErrorCode::Permission => retry_action.clone(),
            DoctorActionPhase::Idle
            | DoctorActionPhase::Pending(_)
            | DoctorActionPhase::Failed { .. } => {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "diagnostic action has no retryable failure",
                    false,
                ));
            }
        };
        self.begin(action)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> DoctorActionFinish {
        let pending = match &self.phase {
            DoctorActionPhase::Pending(pending) if pending.token == token => pending.clone(),
            DoctorActionPhase::Idle
            | DoctorActionPhase::Pending(_)
            | DoctorActionPhase::Failed { .. } => return DoctorActionFinish::Stale,
        };
        self.phase = match result {
            Ok(()) => DoctorActionPhase::Idle,
            Err(failure) => DoctorActionPhase::Failed {
                failure,
                retry_action: pending.action,
            },
        };
        DoctorActionFinish::Applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_and_stale_results_cannot_complete_or_replace_a_request_and_retry_keeps_its_identity() {
        let mut state = DoctorActionState::default();
        assert!(state.begin_retry().is_err());
        let first = state
            .begin(DoctorAction::RepairOne("config.integrity".into()))
            .unwrap();
        assert!(state.begin(DoctorAction::Bootstrap).is_err());
        assert_eq!(
            state.finish(first.token + 1, Ok(())),
            DoctorActionFinish::Stale
        );
        assert_eq!(state.phase, DoctorActionPhase::Pending(first.clone()));
        let failure = Failure::new(ErrorCode::Permission, "privilege required", true);
        assert_eq!(
            state.finish(first.token, Err(failure.clone())),
            DoctorActionFinish::Applied
        );
        assert_eq!(
            state.phase,
            DoctorActionPhase::Failed {
                failure,
                retry_action: first.action.clone(),
            }
        );
        let second = state.begin_retry().unwrap();
        assert!(second.token > first.token);
        assert_eq!(state.finish(first.token, Ok(())), DoctorActionFinish::Stale);
        assert_eq!(state.phase, DoctorActionPhase::Pending(second.clone()));
        assert_eq!(
            state.finish(second.token, Ok(())),
            DoctorActionFinish::Applied
        );
        assert_eq!(state.phase, DoctorActionPhase::Idle);
    }
}
