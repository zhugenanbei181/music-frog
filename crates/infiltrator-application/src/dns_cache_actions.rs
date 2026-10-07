//! Shared visible confirmation and exact terminal correlation; readers never finish a command.
use infiltrator_contract::dns_cache::{DnsCacheOperationId, DnsCacheSnapshot};
use infiltrator_contract::error::{ErrorCode, Failure};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);
pub fn allocate_operation() -> Result<DnsCacheOperationId, Failure> {
    NEXT_OPERATION
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map(DnsCacheOperationId)
        .map_err(|_| {
            Failure::new(
                ErrorCode::InvalidState,
                "DNS cache identity exhausted",
                false,
            )
        })
}
#[derive(Clone, Debug)]
pub struct DnsCacheActions {
    pub open: bool,
    pub confirmed: bool,
    pub pending: Option<u64>,
    pub failure: Option<Failure>,
    pub snapshot: DnsCacheSnapshot,
    pub requested: Option<DnsCacheOperationId>,
}
impl Default for DnsCacheActions {
    fn default() -> Self {
        Self {
            open: false,
            confirmed: false,
            pending: None,
            failure: None,
            snapshot: DnsCacheSnapshot::unavailable(),
            requested: None,
        }
    }
}
impl DnsCacheActions {
    pub fn show(&mut self) {
        if self.pending.is_none() {
            self.open = true;
            self.confirmed = false;
            self.requested = None;
            self.failure = None;
        }
    }
    pub fn cancel(&mut self) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.open = false;
        self.confirmed = false;
        self.failure = None;
        true
    }
    pub fn confirm(&mut self) -> Result<u64, Failure> {
        if self.confirmed {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cache confirmation was already consumed",
                false,
            ));
        }
        self.begin()
    }
    pub fn retry(&mut self) -> Result<u64, Failure> {
        if !self.can_retry() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cache has no retryable failure",
                false,
            ));
        }
        self.begin()
    }
    fn begin(&mut self) -> Result<u64, Failure> {
        if !self.open || self.pending.is_some() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cache flush requires an open confirmation",
                false,
            ));
        }
        let operation = allocate_operation()?;
        self.requested = Some(operation);
        self.pending = Some(operation.0);
        self.confirmed = true;
        self.failure = None;
        Ok(operation.0)
    }
    pub fn finish(&mut self, token: u64, result: Result<(), Failure>) -> bool {
        if self.pending != Some(token) {
            return false;
        }
        self.pending = None;
        self.failure = result.err();
        true
    }
    pub fn observe(&mut self, snapshot: &DnsCacheSnapshot) {
        if snapshot.revision >= self.snapshot.revision
            && (!self.confirmed
                || self.requested == snapshot.operation_id
                || self.requested == snapshot.report_id)
        {
            self.snapshot = snapshot.clone();
        }
    }
    pub fn can_retry(&self) -> bool {
        self.open
            && self.confirmed
            && self.pending.is_none()
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
    fn cancellation_closed_confirmation_duplicate_submit_reader_replay_and_wrong_terminal_are_inert()
     {
        let mut state = DnsCacheActions::default();
        assert!(state.confirm().is_err());
        state.show();
        assert!(state.cancel());
        assert!(state.confirm().is_err());
        state.show();
        let token = state.confirm().unwrap();
        assert!(state.confirm().is_err());
        assert!(!state.cancel());
        state.observe(&DnsCacheSnapshot {
            revision: 2,
            report_id: Some(DnsCacheOperationId(token)),
            ..Default::default()
        });
        assert_eq!(state.pending, Some(token));
        assert!(!state.finish(token + 1, Ok(())));
        assert!(state.finish(
            token,
            Err(Failure::new(
                ErrorCode::Permission,
                "grant cache access",
                false
            ))
        ));
        assert!(state.can_retry());
        state.observe(&DnsCacheSnapshot::default());
        assert_eq!(state.snapshot.revision, 2);
        assert!(state.failure.is_some());
        let next = state.retry().unwrap();
        assert!(!state.finish(token, Ok(())));
        assert!(state.finish(next, Ok(())));
        assert!(state.failure.is_none());
        assert!(state.cancel());
    }
}
