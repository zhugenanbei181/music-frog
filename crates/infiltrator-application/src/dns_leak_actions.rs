//! Native adapters correlate terminal DNS probe feedback without completing writes on snapshots.
use infiltrator_contract::error::{ErrorCode, Failure};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DnsLeakActionPhase {
    #[default]
    Idle,
    Pending {
        token: u64,
    },
    Failed {
        failure: Failure,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DnsLeakFinish {
    Stale,
    Applied,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DnsLeakActionState {
    pub phase: DnsLeakActionPhase,
    next_token: u64,
}
impl DnsLeakActionState {
    pub fn begin(&mut self) -> Result<u64, Failure> {
        if matches!(&self.phase, DnsLeakActionPhase::Pending { .. }) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cross-source probe is already pending",
                true,
            ));
        }
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(ErrorCode::InvalidState, "DNS probe token exhausted", false)
        })?;
        self.phase = DnsLeakActionPhase::Pending {
            token: self.next_token,
        };
        Ok(self.next_token)
    }
    pub fn begin_retry(&mut self) -> Result<u64, Failure> {
        match &self.phase {
            DnsLeakActionPhase::Failed { failure }
                if failure.retryable || failure.code == ErrorCode::Permission =>
            {
                self.begin()
            }
            DnsLeakActionPhase::Idle
            | DnsLeakActionPhase::Pending { .. }
            | DnsLeakActionPhase::Failed { .. } => Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS probe has no retryable failure",
                false,
            )),
        }
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> DnsLeakFinish {
        let DnsLeakActionPhase::Pending { token: expected } = self.phase else {
            return DnsLeakFinish::Stale;
        };
        if expected != token {
            return DnsLeakFinish::Stale;
        }
        self.phase = match result {
            Ok(()) => DnsLeakActionPhase::Idle,
            Err(failure) => DnsLeakActionPhase::Failed { failure },
        };
        DnsLeakFinish::Applied
    }
    pub fn can_retry(&self) -> bool {
        matches!(
            &self.phase,
            DnsLeakActionPhase::Failed { failure }
                if failure.retryable || failure.code == ErrorCode::Permission
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_feedback_cannot_complete_new_probe_and_permission_recovery_is_explicit() {
        let mut state = DnsLeakActionState::default();
        assert!(state.begin_retry().is_err());
        let first = state.begin().unwrap();
        assert!(state.begin().is_err());
        assert_eq!(state.finish(first + 1, Ok(())), DnsLeakFinish::Stale);
        assert_eq!(state.phase, DnsLeakActionPhase::Pending { token: first });
        assert_eq!(
            state.finish(
                first,
                Err(Failure::new(
                    ErrorCode::Permission,
                    "allow echo probe",
                    false
                ))
            ),
            DnsLeakFinish::Applied
        );
        assert!(state.can_retry());
        let second = state.begin_retry().unwrap();
        assert_eq!(state.phase, DnsLeakActionPhase::Pending { token: second });
        assert_eq!(state.finish(first, Ok(())), DnsLeakFinish::Stale);
        assert_eq!(state.phase, DnsLeakActionPhase::Pending { token: second });
        assert_eq!(state.finish(second, Ok(())), DnsLeakFinish::Applied);
        assert_eq!(state.phase, DnsLeakActionPhase::Idle);
    }
}
