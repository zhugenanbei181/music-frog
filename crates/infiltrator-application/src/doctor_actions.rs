//! Correlated doctor command state shared by native adapters; telemetry does not complete writes.
use infiltrator_contract::doctor::DoctorAction;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingDoctorAction {
    pub token: u64,
    pub action: DoctorAction,
}
#[derive(Clone, Debug, Default)]
pub struct DoctorActionState {
    pub pending: Option<PendingDoctorAction>,
    pub failure: Option<Failure>,
    pub retry_action: Option<DoctorAction>,
    next_token: u64,
}
impl DoctorActionState {
    /// Permission recovery is manual: the user retries after following the visible host error.
    pub fn can_retry(&self) -> bool {
        self.pending.is_none()
            && self.retry_action.is_some()
            && self
                .failure
                .as_ref()
                .is_some_and(|failure| failure.retryable || failure.code == ErrorCode::Permission)
    }
    pub fn begin(&mut self, action: DoctorAction) -> Result<PendingDoctorAction, Failure> {
        if self.pending.is_some() {
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
        self.pending = Some(pending.clone());
        self.failure = None;
        self.retry_action = None;
        Ok(pending)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.token == token)
        else {
            return false;
        };
        match result {
            Ok(()) => {
                self.failure = None;
                self.retry_action = None;
            }
            Err(failure) => {
                self.failure = Some(failure);
                self.retry_action = Some(pending.action.clone());
            }
        }
        self.pending = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_and_stale_results_cannot_complete_or_replace_a_request_and_retry_keeps_its_identity() {
        let mut state = DoctorActionState::default();
        let first = state
            .begin(DoctorAction::RepairOne("config.integrity".into()))
            .unwrap();
        assert!(state.begin(DoctorAction::Bootstrap).is_err());
        assert!(!state.finish(first.token + 1, Ok(())));
        assert_eq!(state.pending.as_ref(), Some(&first));
        let failure = Failure::new(ErrorCode::Permission, "privilege required", true);
        assert!(state.finish(first.token, Err(failure.clone())));
        assert_eq!(state.failure, Some(failure));
        assert_eq!(state.retry_action, Some(first.action.clone()));
        let second = state.begin(state.retry_action.clone().unwrap()).unwrap();
        assert!(second.token > first.token);
        assert!(!state.finish(first.token, Ok(())));
        assert_eq!(state.pending.as_ref(), Some(&second));
        assert!(state.finish(second.token, Ok(())));
        assert!(state.pending.is_none());
        assert!(state.failure.is_none());
        assert!(state.retry_action.is_none());
    }
}
