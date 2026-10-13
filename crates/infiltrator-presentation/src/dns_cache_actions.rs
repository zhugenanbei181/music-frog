//! Cache confirmation and correlated command/readback lifecycle.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheReadback {
    Waiting,
    Observed,
}

/// Mutually exclusive cache-modal phases. A readback may arrive before the
/// command receipt, so that ordering is represented explicitly in Submitting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DnsCachePhase {
    Closed,
    Confirming,
    Submitting {
        operation: DnsCacheOperationId,
        readback: CacheReadback,
    },
    AwaitingReadback {
        operation: DnsCacheOperationId,
    },
    Observed {
        operation: DnsCacheOperationId,
    },
    Failed {
        operation: DnsCacheOperationId,
        failure: Failure,
    },
}

#[derive(Clone, Debug)]
pub struct DnsCacheActions {
    phase: DnsCachePhase,
    pub snapshot: DnsCacheSnapshot,
}

impl Default for DnsCacheActions {
    fn default() -> Self {
        Self {
            phase: DnsCachePhase::Closed,
            snapshot: DnsCacheSnapshot::unavailable(),
        }
    }
}

impl DnsCacheActions {
    pub fn phase(&self) -> &DnsCachePhase {
        &self.phase
    }

    pub fn is_open(&self) -> bool {
        !matches!(&self.phase, DnsCachePhase::Closed)
    }

    pub fn is_confirmed(&self) -> bool {
        matches!(
            &self.phase,
            DnsCachePhase::Submitting { .. }
                | DnsCachePhase::AwaitingReadback { .. }
                | DnsCachePhase::Observed { .. }
                | DnsCachePhase::Failed { .. }
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(&self.phase, DnsCachePhase::Submitting { .. })
    }

    pub fn requested_operation(&self) -> Option<DnsCacheOperationId> {
        match &self.phase {
            DnsCachePhase::Submitting { operation, .. }
            | DnsCachePhase::AwaitingReadback { operation }
            | DnsCachePhase::Observed { operation }
            | DnsCachePhase::Failed { operation, .. } => Some(*operation),
            DnsCachePhase::Closed | DnsCachePhase::Confirming => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            DnsCachePhase::Failed { failure, .. } => Some(failure),
            _ => None,
        }
    }

    pub fn show(&mut self) {
        if !self.is_pending() {
            self.phase = DnsCachePhase::Confirming;
        }
    }

    pub fn cancel(&mut self) -> bool {
        if self.is_pending() {
            return false;
        }
        self.phase = DnsCachePhase::Closed;
        true
    }

    pub fn confirm(&mut self) -> Result<DnsCacheOperationId, Failure> {
        if !matches!(&self.phase, DnsCachePhase::Confirming) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cache flush requires an open confirmation",
                false,
            ));
        }
        self.begin()
    }

    pub fn retry(&mut self) -> Result<DnsCacheOperationId, Failure> {
        if !self.can_retry() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cache has no retryable failure",
                false,
            ));
        }
        self.begin()
    }

    fn begin(&mut self) -> Result<DnsCacheOperationId, Failure> {
        if !self.is_open() || self.is_pending() {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "DNS cache flush requires an open confirmation",
                false,
            ));
        }
        let operation = allocate_operation()?;
        self.phase = DnsCachePhase::Submitting {
            operation,
            readback: CacheReadback::Waiting,
        };
        Ok(operation)
    }

    pub fn finish(&mut self, operation: DnsCacheOperationId, result: Result<(), Failure>) -> bool {
        let DnsCachePhase::Submitting {
            operation: expected,
            readback,
        } = &self.phase
        else {
            return false;
        };
        if operation != *expected {
            return false;
        }
        let readback = *readback;
        self.phase = match result {
            Err(failure) => DnsCachePhase::Failed { operation, failure },
            Ok(()) if readback == CacheReadback::Observed => DnsCachePhase::Observed { operation },
            Ok(()) if self.snapshot.report_id == Some(operation) => {
                DnsCachePhase::Observed { operation }
            }
            Ok(()) => DnsCachePhase::AwaitingReadback { operation },
        };
        true
    }

    pub fn observe(&mut self, snapshot: &DnsCacheSnapshot) {
        let correlation = self.requested_operation();
        if snapshot.revision < self.snapshot.revision
            || correlation.is_some_and(|operation| {
                snapshot.operation_id != Some(operation) && snapshot.report_id != Some(operation)
            })
        {
            return;
        }
        self.snapshot.clone_from(snapshot);
        self.phase = match self.phase.clone() {
            DnsCachePhase::Submitting {
                operation,
                readback: _,
            } if snapshot.operation_id == Some(operation)
                || snapshot.report_id == Some(operation) =>
            {
                DnsCachePhase::Submitting {
                    operation,
                    readback: if snapshot.report_id == Some(operation) {
                        CacheReadback::Observed
                    } else {
                        CacheReadback::Waiting
                    },
                }
            }
            DnsCachePhase::AwaitingReadback { operation }
                if snapshot.report_id == Some(operation) =>
            {
                DnsCachePhase::Observed { operation }
            }
            phase => phase,
        };
    }

    pub fn can_retry(&self) -> bool {
        matches!(
            &self.phase,
            DnsCachePhase::Failed { failure, .. }
                if failure.retryable || failure.code == ErrorCode::Permission
        )
    }
}
