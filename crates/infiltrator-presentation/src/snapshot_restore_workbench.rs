//! Typed snapshot restoration phases; SnapshotApplication retains all I/O ownership.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::snapshot_restore::{
    SnapshotRestoreIdentity, SnapshotRestoreReceipt, SnapshotRestoreReview, SnapshotRestoreTarget,
};
use infiltrator_domain::profile_source::hash_document_bytes;
use lurker_core_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestorePending {
    pub operation: u64,
    pub intent: CommandIntent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreCancelContext {
    Reviewed(SnapshotRestoreReview),
    Rejected {
        target: SnapshotRestoreTarget,
        identity: SnapshotRestoreIdentity,
    },
}

impl RestoreCancelContext {
    fn target(&self) -> &SnapshotRestoreTarget {
        match self {
            Self::Reviewed(review) => &review.target,
            Self::Rejected { target, .. } => target,
        }
    }

    fn identity(&self) -> &SnapshotRestoreIdentity {
        match self {
            Self::Reviewed(review) => &review.identity,
            Self::Rejected { identity, .. } => identity,
        }
    }

    fn review(&self) -> Option<&SnapshotRestoreReview> {
        match self {
            Self::Reviewed(review) => Some(review),
            Self::Rejected { .. } => None,
        }
    }
}

/// Each stage contains only the identity and evidence valid at that stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotRestorePhase {
    Closed,
    Preparing {
        target: SnapshotRestoreTarget,
        pending: RestorePending,
    },
    Review {
        review: SnapshotRestoreReview,
    },
    Restoring {
        review: SnapshotRestoreReview,
        pending: RestorePending,
    },
    Restored {
        review: SnapshotRestoreReview,
        receipt: SnapshotRestoreReceipt,
    },
    Cancelling {
        context: RestoreCancelContext,
        pending: RestorePending,
    },
    PrepareFailed {
        target: SnapshotRestoreTarget,
        failure: Failure,
    },
    ReviewRejected {
        target: SnapshotRestoreTarget,
        identity: SnapshotRestoreIdentity,
        failure: Failure,
    },
    RestoreFailed {
        review: SnapshotRestoreReview,
        failure: Failure,
    },
    CancelFailed {
        context: RestoreCancelContext,
        failure: Failure,
    },
}

impl Default for SnapshotRestorePhase {
    fn default() -> Self {
        Self::Closed
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SnapshotRestoreWorkbench {
    phase: SnapshotRestorePhase,
    sequence: u64,
}

impl SnapshotRestoreWorkbench {
    pub fn phase(&self) -> &SnapshotRestorePhase {
        &self.phase
    }

    pub fn is_visible(&self) -> bool {
        !matches!(self.phase, SnapshotRestorePhase::Closed)
    }

    pub fn is_busy(&self) -> bool {
        self.pending().is_some()
    }

    pub fn target(&self) -> Option<&SnapshotRestoreTarget> {
        match &self.phase {
            SnapshotRestorePhase::Closed => None,
            SnapshotRestorePhase::Preparing { target, .. }
            | SnapshotRestorePhase::PrepareFailed { target, .. }
            | SnapshotRestorePhase::ReviewRejected { target, .. } => Some(target),
            SnapshotRestorePhase::Review { review }
            | SnapshotRestorePhase::Restoring { review, .. }
            | SnapshotRestorePhase::Restored { review, .. }
            | SnapshotRestorePhase::RestoreFailed { review, .. } => Some(&review.target),
            SnapshotRestorePhase::Cancelling { context, .. }
            | SnapshotRestorePhase::CancelFailed { context, .. } => Some(context.target()),
        }
    }

    pub fn review(&self) -> Option<&SnapshotRestoreReview> {
        match &self.phase {
            SnapshotRestorePhase::Review { review }
            | SnapshotRestorePhase::Restoring { review, .. }
            | SnapshotRestorePhase::Restored { review, .. }
            | SnapshotRestorePhase::RestoreFailed { review, .. } => Some(review),
            SnapshotRestorePhase::Cancelling { context, .. }
            | SnapshotRestorePhase::CancelFailed { context, .. } => context.review(),
            SnapshotRestorePhase::Closed
            | SnapshotRestorePhase::Preparing { .. }
            | SnapshotRestorePhase::PrepareFailed { .. }
            | SnapshotRestorePhase::ReviewRejected { .. } => None,
        }
    }

    pub fn restored(&self) -> Option<&SnapshotRestoreReceipt> {
        match &self.phase {
            SnapshotRestorePhase::Restored { receipt, .. } => Some(receipt),
            _ => None,
        }
    }

    pub fn failure(&self) -> Option<&Failure> {
        match &self.phase {
            SnapshotRestorePhase::PrepareFailed { failure, .. }
            | SnapshotRestorePhase::ReviewRejected { failure, .. }
            | SnapshotRestorePhase::RestoreFailed { failure, .. }
            | SnapshotRestorePhase::CancelFailed { failure, .. } => Some(failure),
            _ => None,
        }
    }

    pub fn pending(&self) -> Option<&RestorePending> {
        match &self.phase {
            SnapshotRestorePhase::Preparing { pending, .. }
            | SnapshotRestorePhase::Restoring { pending, .. }
            | SnapshotRestorePhase::Cancelling { pending, .. } => Some(pending),
            _ => None,
        }
    }

    pub fn can_confirm(&self) -> bool {
        matches!(self.phase, SnapshotRestorePhase::Review { .. })
    }

    pub fn can_cancel(&self) -> bool {
        self.is_visible() && !self.is_busy()
    }

    pub fn open(&mut self, target: SnapshotRestoreTarget) -> Option<RestorePending> {
        if !matches!(self.phase, SnapshotRestorePhase::Closed) {
            return None;
        }
        self.begin_prepare(target)
    }

    pub fn confirm(&mut self) -> Option<RestorePending> {
        let SnapshotRestorePhase::Review { review } = self.phase.clone() else {
            return None;
        };
        let pending = self.begin(CommandIntent::ConfirmSnapshotRestore {
            identity: review.identity.clone(),
        })?;
        self.phase = SnapshotRestorePhase::Restoring {
            review,
            pending: pending.clone(),
        };
        Some(pending)
    }

    pub fn cancel(&mut self) -> Option<RestorePending> {
        match self.phase.clone() {
            SnapshotRestorePhase::Closed
            | SnapshotRestorePhase::Preparing { .. }
            | SnapshotRestorePhase::Restoring { .. }
            | SnapshotRestorePhase::Cancelling { .. } => None,
            SnapshotRestorePhase::Restored { .. } => {
                self.phase = SnapshotRestorePhase::Closed;
                None
            }
            SnapshotRestorePhase::PrepareFailed { .. } => {
                self.phase = SnapshotRestorePhase::Closed;
                None
            }
            SnapshotRestorePhase::ReviewRejected {
                target, identity, ..
            } => self.begin_cancel(RestoreCancelContext::Rejected { target, identity }),
            SnapshotRestorePhase::Review { review }
            | SnapshotRestorePhase::RestoreFailed { review, .. } => {
                self.begin_cancel(RestoreCancelContext::Reviewed(review))
            }
            SnapshotRestorePhase::CancelFailed { context, .. } => self.begin_cancel(context),
        }
    }

    pub fn can_retry(&self) -> bool {
        let failure = match &self.phase {
            SnapshotRestorePhase::PrepareFailed { failure, .. }
            | SnapshotRestorePhase::ReviewRejected { failure, .. }
            | SnapshotRestorePhase::RestoreFailed { failure, .. }
            | SnapshotRestorePhase::CancelFailed { failure, .. } => failure,
            _ => return false,
        };
        matches!(
            failure.code,
            ErrorCode::Permission | ErrorCode::Network | ErrorCode::Storage | ErrorCode::NotReady
        ) && (failure.retryable || failure.code == ErrorCode::Permission)
    }

    pub fn retry(&mut self) -> Option<RestorePending> {
        if !self.can_retry() {
            return None;
        }
        match self.phase.clone() {
            SnapshotRestorePhase::PrepareFailed { target, .. } => self.begin_prepare(target),
            SnapshotRestorePhase::RestoreFailed { review, .. } => self.confirm_review(review),
            SnapshotRestorePhase::CancelFailed { context, .. } => self.begin_cancel(context),
            SnapshotRestorePhase::ReviewRejected { .. } => None,
            _ => None,
        }
    }

    pub fn finish(&mut self, operation: u64, result: Result<CommandOutput, Failure>) {
        let (pending, pending_phase) = match self.phase.clone() {
            SnapshotRestorePhase::Preparing { target, pending } => {
                (pending, PendingRestore::Prepare(target))
            }
            SnapshotRestorePhase::Restoring { review, pending } => {
                (pending, PendingRestore::Restore(review))
            }
            SnapshotRestorePhase::Cancelling { context, pending } => {
                (pending, PendingRestore::Cancel(context))
            }
            _ => return,
        };
        if pending.operation != operation {
            return;
        }
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            Ok(output)
        });
        self.phase = match (pending_phase, result) {
            (
                PendingRestore::Prepare(target),
                Ok(CommandOutput::SnapshotRestorePrepared(review)),
            ) => {
                let review = *review;
                if hash_document_bytes(&review.before_yaml)
                    != review
                        .identity
                        .workspace_identity
                        .document
                        .revision
                        .as_str()
                    || hash_document_bytes(&review.restored_yaml) != review.identity.snapshot_hash
                {
                    self.phase = SnapshotRestorePhase::ReviewRejected {
                        target,
                        identity: review.identity,
                        failure: Failure::new(
                            ErrorCode::InvalidState,
                            "Snapshot review bytes do not match their identities",
                            false,
                        ),
                    };
                    return;
                }
                SnapshotRestorePhase::Review { review }
            }
            (PendingRestore::Prepare(target), Err(failure)) => {
                SnapshotRestorePhase::PrepareFailed { target, failure }
            }
            (PendingRestore::Restore(review), Ok(CommandOutput::SnapshotRestored(receipt))) => {
                match receipt.validate(&review.identity) {
                    Ok(()) => SnapshotRestorePhase::Restored { review, receipt },
                    Err(failure) => SnapshotRestorePhase::RestoreFailed { review, failure },
                }
            }
            (PendingRestore::Restore(review), Err(failure)) => {
                SnapshotRestorePhase::RestoreFailed { review, failure }
            }
            (PendingRestore::Cancel(_), Ok(CommandOutput::SnapshotRestoreCancelled(_))) => {
                SnapshotRestorePhase::Closed
            }
            (PendingRestore::Cancel(context), Err(failure)) => {
                SnapshotRestorePhase::CancelFailed { context, failure }
            }
            (_, Ok(_)) => unreachable!("typed output validation rejected unrelated output"),
        };
    }

    fn confirm_review(&mut self, review: SnapshotRestoreReview) -> Option<RestorePending> {
        let pending = self.begin(CommandIntent::ConfirmSnapshotRestore {
            identity: review.identity.clone(),
        })?;
        self.phase = SnapshotRestorePhase::Restoring {
            review,
            pending: pending.clone(),
        };
        Some(pending)
    }

    fn begin_prepare(&mut self, target: SnapshotRestoreTarget) -> Option<RestorePending> {
        let pending = self.begin(CommandIntent::PrepareSnapshotRestore {
            target: target.clone(),
        })?;
        self.phase = SnapshotRestorePhase::Preparing {
            target,
            pending: pending.clone(),
        };
        Some(pending)
    }

    fn begin_cancel(&mut self, context: RestoreCancelContext) -> Option<RestorePending> {
        let pending = self.begin(CommandIntent::CancelSnapshotRestore {
            identity: context.identity().clone(),
        })?;
        self.phase = SnapshotRestorePhase::Cancelling {
            context,
            pending: pending.clone(),
        };
        Some(pending)
    }

    fn begin(&mut self, intent: CommandIntent) -> Option<RestorePending> {
        self.sequence = self.sequence.checked_add(1)?;
        Some(RestorePending {
            operation: self.sequence,
            intent,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingRestore {
    Prepare(SnapshotRestoreTarget),
    Restore(SnapshotRestoreReview),
    Cancel(RestoreCancelContext),
}
