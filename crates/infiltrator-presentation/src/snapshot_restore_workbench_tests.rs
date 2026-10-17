use crate::snapshot_restore_workbench::{
    RestoreCancelContext, SnapshotRestorePhase, SnapshotRestoreWorkbench,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentIdentity, ProfileDocumentRevision, ProfileDocumentSourceIdentity,
    ProfileDocumentSourceKind, ProfileFormatId, ProfileId, ProfileSchemaVersion,
    ProfileSidecarRevision, ProfileSourceId, ProfileWorkspaceIdentity,
};
use infiltrator_contract::snapshot_restore::SnapshotRestoreCancelled;
use infiltrator_contract::snapshot_restore::{
    SnapshotRestoreIdentity, SnapshotRestoreReceipt, SnapshotRestoreReview, SnapshotRestoreTarget,
};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::profile_source::hash_document_bytes;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;

fn review(
    target: SnapshotRestoreTarget,
    restored_yaml: &str,
) -> (SnapshotRestoreIdentity, SnapshotRestoreReview) {
    let source = "mode: rule\n";
    let identity = SnapshotRestoreIdentity {
        owner: 1,
        sequence: 1,
        snapshot_hash: hash_document_bytes(restored_yaml),
        workspace_identity: ProfileWorkspaceIdentity::new(
            ProfileDocumentIdentity::new(
                ProfileId::new(target.profile.clone()).unwrap(),
                EngineId::new("mihomo").unwrap(),
                ProfileDocumentSourceIdentity::new(
                    ProfileDocumentSourceKind::Local,
                    ProfileSourceId::new("local-main").unwrap(),
                ),
                ProfileDocumentRevision::new(hash_document_bytes(source)).unwrap(),
                ProfileFormatId::new("mihomo-yaml").unwrap(),
                ProfileSchemaVersion::new(1).unwrap(),
            ),
            ProfileSidecarRevision::Absent,
        ),
    };
    let review = SnapshotRestoreReview {
        identity: identity.clone(),
        target: target.clone(),
        before_yaml: source.into(),
        restored_yaml: restored_yaml.into(),
        difference: YamlAstDiffSnapshot {
            target_id: target.profile.clone(),
            source_path: Some(target.snapshot_id.clone()),
            ..Default::default()
        },
    };
    (identity, review)
}

fn restored_receipt(review: &SnapshotRestoreReview) -> SnapshotRestoreReceipt {
    let mut committed = review.identity.workspace_identity.clone();
    committed.document.revision =
        ProfileDocumentRevision::new(review.identity.snapshot_hash.clone()).unwrap();
    SnapshotRestoreReceipt {
        identity: review.identity.clone(),
        committed,
    }
}

#[test]
fn rejected_review_cannot_be_confirmed_and_its_exact_identity_can_be_cancelled() {
    let target = SnapshotRestoreTarget {
        profile: "main".into(),
        snapshot_id: "/history/one".into(),
    };
    let (identity, mut review) = review(target.clone(), "mode: direct\n");
    review.restored_yaml = "corrupted response".into();
    let mut model = SnapshotRestoreWorkbench::default();
    let prepare = model.open(target).unwrap();
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::Preparing { .. }
    ));
    model.finish(
        prepare.operation,
        Ok(CommandOutput::SnapshotRestorePrepared(Box::new(review))),
    );
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::ReviewRejected { .. }
    ));
    assert_eq!(model.failure().unwrap().code, ErrorCode::InvalidState);
    assert!(!model.can_confirm());
    let cancel = model.cancel().unwrap();
    assert_eq!(
        cancel.intent,
        CommandIntent::CancelSnapshotRestore {
            identity: identity.clone()
        }
    );
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::Cancelling {
            context: RestoreCancelContext::Rejected { .. },
            ..
        }
    ));
    model.finish(
        cancel.operation,
        Ok(CommandOutput::SnapshotRestoreCancelled(
            SnapshotRestoreCancelled { identity },
        )),
    );
    assert!(matches!(model.phase(), SnapshotRestorePhase::Closed));
}

#[test]
fn restore_failure_retries_with_a_new_operation_and_only_a_fresh_matching_receipt_completes() {
    let target = SnapshotRestoreTarget {
        profile: "main".into(),
        snapshot_id: "/history/one".into(),
    };
    let (identity, review) = review(target, "mode: direct\n");
    let mut model = SnapshotRestoreWorkbench::default();
    let prepare = model.open(review.target.clone()).unwrap();
    model.finish(
        prepare.operation,
        Ok(CommandOutput::SnapshotRestorePrepared(Box::new(
            review.clone(),
        ))),
    );
    assert!(matches!(model.phase(), SnapshotRestorePhase::Review { .. }));

    let restore = model.confirm().unwrap();
    model.finish(
        restore.operation + 1,
        Ok(CommandOutput::SnapshotRestored(restored_receipt(&review))),
    );
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::Restoring { .. }
    ));
    assert!(model.restored().is_none());
    model.finish(
        restore.operation,
        Err(Failure::new(
            ErrorCode::Permission,
            "profile write denied",
            false,
        )),
    );
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::RestoreFailed { .. }
    ));
    assert!(model.can_retry());

    let retry = model.retry().unwrap();
    assert_ne!(retry.operation, restore.operation);
    assert_eq!(
        retry.intent,
        CommandIntent::ConfirmSnapshotRestore {
            identity: identity.clone()
        }
    );
    let receipt = restored_receipt(&review);
    model.finish(
        retry.operation,
        Ok(CommandOutput::SnapshotRestored(receipt.clone())),
    );
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::Restored { .. }
    ));
    assert_eq!(model.restored(), Some(&receipt));
    assert!(model.cancel().is_none());
    assert!(matches!(model.phase(), SnapshotRestorePhase::Closed));
}

#[test]
fn mismatched_terminal_receipt_cannot_fabricate_a_restore_success() {
    let target = SnapshotRestoreTarget {
        profile: "main".into(),
        snapshot_id: "/history/one".into(),
    };
    let (_, review) = review(target, "mode: direct\n");
    let mut model = SnapshotRestoreWorkbench::default();
    let prepare = model.open(review.target.clone()).unwrap();
    model.finish(
        prepare.operation,
        Ok(CommandOutput::SnapshotRestorePrepared(Box::new(
            review.clone(),
        ))),
    );
    let restore = model.confirm().unwrap();
    let mut stale = restored_receipt(&review);
    stale.identity.sequence += 1;
    model.finish(
        restore.operation,
        Ok(CommandOutput::SnapshotRestored(stale)),
    );
    assert!(matches!(
        model.phase(),
        SnapshotRestorePhase::RestoreFailed { .. }
    ));
    assert_eq!(model.failure().unwrap().code, ErrorCode::InvalidState);
    assert!(model.restored().is_none());
}
