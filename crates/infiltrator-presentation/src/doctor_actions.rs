//! Correlated doctor command state shared by native adapters; telemetry does not complete writes.
use infiltrator_contract::doctor::DoctorAction;
use lurker_core_contract::error::{ErrorCode, Failure};

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
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(
                ErrorCode::InvalidState,
                "diagnostic action token exhausted",
                false,
            )
        })?;
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
#[path = "doctor_actions_tests.rs"]
mod tests;
