//! Native adapters correlate terminal DNS probe feedback without completing writes on snapshots.
use infiltrator_contract::error::{ErrorCode, Failure};
#[derive(Clone, Debug, Default)]
pub struct DnsLeakActionState {
    pub pending: Option<u64>,
    pub failure: Option<Failure>,
    next_token: u64,
}
impl DnsLeakActionState {
    pub fn begin(&mut self) -> Result<u64, Failure> {
        if self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cross-source probe is already pending",
                true,
            ));
        }
        self.next_token = self.next_token.checked_add(1).ok_or_else(|| {
            Failure::new(ErrorCode::InvalidState, "DNS probe token exhausted", false)
        })?;
        self.pending = Some(self.next_token);
        self.failure = None;
        Ok(self.next_token)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        if self.pending != Some(token) {
            return false;
        }
        self.failure = result.err();
        self.pending = None;
        true
    }
    pub fn can_retry(&self) -> bool {
        self.pending.is_none()
            && self
                .failure
                .as_ref()
                .is_some_and(|failure| failure.retryable || failure.code == ErrorCode::Permission)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_feedback_cannot_complete_new_probe_and_permission_recovery_is_explicit() {
        let mut state = DnsLeakActionState::default();
        let first = state.begin().unwrap();
        assert!(state.begin().is_err());
        assert!(!state.finish(first + 1, Ok(())));
        assert_eq!(state.pending, Some(first));
        assert!(state.finish(
            first,
            Err(Failure::new(
                ErrorCode::Permission,
                "allow echo probe",
                false
            ))
        ));
        assert!(state.can_retry());
        let second = state.begin().unwrap();
        assert!(!state.finish(first, Ok(())));
        assert_eq!(state.pending, Some(second));
        assert!(state.finish(second, Ok(())));
        assert!(state.failure.is_none());
    }
}
