//! One restoration transaction state machine for both native products.
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot_restore::{
    SnapshotRestoreIdentity, SnapshotRestoreReceipt, SnapshotRestoreReview, SnapshotRestoreTarget,
};
use infiltrator_domain::profile_source::hash_document_bytes;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestorePending {
    pub operation: u64,
    pub intent: CommandIntent,
}
#[derive(Clone, Debug, Default)]
pub struct SnapshotRestoreWorkbench {
    pub visible: bool,
    pub target: Option<SnapshotRestoreTarget>,
    pub review: Option<SnapshotRestoreReview>,
    pub restored: Option<SnapshotRestoreReceipt>,
    pub failure: Option<Failure>,
    pub pending: Option<RestorePending>,
    retry: Option<CommandIntent>,
    rejected_review: Option<SnapshotRestoreIdentity>,
    sequence: u64,
}
impl SnapshotRestoreWorkbench {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn open(&mut self, target: SnapshotRestoreTarget) -> Option<RestorePending> {
        if self.busy() || self.visible {
            return None;
        }
        self.visible = true;
        self.target = Some(target.clone());
        self.review = None;
        self.restored = None;
        self.begin(CommandIntent::PrepareSnapshotRestore { target })
    }
    pub fn confirm(&mut self) -> Option<RestorePending> {
        if !self.visible || self.failure.is_some() || self.restored.is_some() {
            return None;
        }
        let identity = self.review.as_ref()?.identity.clone();
        self.begin(CommandIntent::ConfirmSnapshotRestore { identity })
    }
    pub fn cancel(&mut self) -> Option<RestorePending> {
        if !self.visible || self.busy() {
            return None;
        }
        let identity = self
            .review
            .as_ref()
            .map(|review| review.identity.clone())
            .or_else(|| self.rejected_review.clone());
        let Some(identity) = identity else {
            self.close();
            return None;
        };
        if self.restored.is_some() {
            self.close();
            return None;
        }
        self.begin(CommandIntent::CancelSnapshotRestore { identity })
    }
    fn close(&mut self) {
        self.visible = false;
        self.target = None;
        self.review = None;
        self.failure = None;
        self.retry = None;
        self.rejected_review = None;
    }
    pub fn can_retry(&self) -> bool {
        self.visible
            && !self.busy()
            && self.retry.is_some()
            && self.failure.as_ref().is_some_and(|failure| {
                matches!(
                    failure.code,
                    ErrorCode::Permission
                        | ErrorCode::Network
                        | ErrorCode::Storage
                        | ErrorCode::NotReady
                ) && (failure.retryable || failure.code == ErrorCode::Permission)
            })
    }
    pub fn retry(&mut self) -> Option<RestorePending> {
        if !self.can_retry() {
            return None;
        }
        self.begin(self.retry.clone()?)
    }
    fn begin(&mut self, intent: CommandIntent) -> Option<RestorePending> {
        if self.busy() {
            return None;
        }
        self.sequence = self.sequence.checked_add(1)?;
        let pending = RestorePending {
            operation: self.sequence,
            intent,
        };
        self.pending = Some(pending.clone());
        self.failure = None;
        self.retry = None;
        Some(pending)
    }
    pub fn finish(&mut self, operation: u64, result: Result<CommandOutput, Failure>) {
        let Some(pending) = self
            .pending
            .as_ref()
            .filter(|pending| pending.operation == operation)
            .cloned()
        else {
            return;
        };
        self.pending = None;
        let result = result.and_then(|output| {
            output.validate_for(&pending.intent)?;
            Ok(output)
        });
        match result {
            Ok(CommandOutput::SnapshotRestorePrepared(review)) => {
                if hash_document_bytes(&review.before_yaml) != review.identity.source.document_hash
                    || hash_document_bytes(&review.restored_yaml) != review.identity.snapshot_hash
                {
                    self.rejected_review = Some(review.identity.clone());
                    self.failure = Some(Failure::new(
                        ErrorCode::InvalidState,
                        "Snapshot review bytes do not match their identities",
                        false,
                    ));
                } else {
                    self.review = Some(*review);
                }
            }
            Ok(CommandOutput::SnapshotRestored(receipt)) => self.restored = Some(receipt),
            Ok(CommandOutput::SnapshotRestoreCancelled(_)) => self.close(),
            Ok(_) => {
                self.failure = Some(Failure::new(
                    ErrorCode::InvalidState,
                    "Unexpected snapshot restoration result",
                    false,
                ))
            }
            Err(failure) => {
                self.retry = Some(pending.intent);
                self.failure = Some(failure);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::profile_source::ProfileSourceIdentity;
    use infiltrator_contract::snapshot_restore::SnapshotRestoreCancelled;
    use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
    #[test]
    fn corrupted_review_bytes_cannot_be_confirmed_but_the_correlated_identity_can_be_cancelled() {
        let target = SnapshotRestoreTarget {
            profile: "main".into(),
            snapshot_id: "/history/one".into(),
        };
        let source = "mode: rule\n";
        let identity = SnapshotRestoreIdentity {
            owner: 1,
            sequence: 1,
            snapshot_hash: hash_document_bytes(source),
            source: ProfileSourceIdentity {
                profile: target.profile.clone(),
                document_hash: hash_document_bytes(source),
                options_hash: None,
            },
        };
        let review = SnapshotRestoreReview {
            identity: identity.clone(),
            target: target.clone(),
            before_yaml: source.into(),
            restored_yaml: "corrupted response".into(),
            difference: YamlAstDiffSnapshot {
                target_id: target.profile.clone(),
                source_path: Some(target.snapshot_id.clone()),
                ..Default::default()
            },
        };
        let mut model = SnapshotRestoreWorkbench::default();
        let pending = model.open(target).unwrap();
        model.finish(
            pending.operation,
            Ok(CommandOutput::SnapshotRestorePrepared(Box::new(review))),
        );
        assert_eq!(
            model.failure.as_ref().unwrap().code,
            ErrorCode::InvalidState
        );
        assert!(model.confirm().is_none());
        let cancel = model
            .cancel()
            .expect("the admitted identity must be retired even when display bytes are corrupt");
        assert_eq!(
            cancel.intent,
            CommandIntent::CancelSnapshotRestore {
                identity: identity.clone()
            }
        );
        model.finish(
            cancel.operation,
            Ok(CommandOutput::SnapshotRestoreCancelled(
                SnapshotRestoreCancelled { identity },
            )),
        );
        assert!(!model.visible);
    }
}
