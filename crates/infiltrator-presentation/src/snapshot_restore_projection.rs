//! Locale-aware restoration review projection over typed workbench phases.
use crate::snapshot_restore_workbench::{SnapshotRestorePhase, SnapshotRestoreWorkbench};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub struct RestorePresentation {
    pub status: String,
    pub details: String,
    pub content: String,
    pub confirm: bool,
    pub cancel: bool,
    pub retry: bool,
}

pub fn project_restore(model: &SnapshotRestoreWorkbench, locale: &str) -> RestorePresentation {
    let lang = Lang(locale);
    let status = if let Some(failure) = model.failure() {
        localize(
            locale,
            "snapshot_restore_failed",
            &[("reason", failure.message.clone())],
        )
    } else {
        let key = match model.phase() {
            SnapshotRestorePhase::Preparing { .. } => "snapshot_restore_pending",
            SnapshotRestorePhase::Restoring { .. } | SnapshotRestorePhase::Cancelling { .. } => {
                "snapshot_restore_pending"
            }
            SnapshotRestorePhase::Restored { .. } => "snapshot_restore_committed",
            SnapshotRestorePhase::Review { .. } => "snapshot_restore_ready",
            SnapshotRestorePhase::Closed
            | SnapshotRestorePhase::PrepareFailed { .. }
            | SnapshotRestorePhase::ReviewRejected { .. }
            | SnapshotRestorePhase::RestoreFailed { .. }
            | SnapshotRestorePhase::CancelFailed { .. } => "snapshot_restore_preparing",
        };
        lang.tr(key).into_owned()
    };
    let details = model
        .review()
        .map(|review| {
            localize(
                locale,
                "snapshot_restore_details",
                &[
                    ("profile", review.target.profile.clone()),
                    ("snapshot", review.target.snapshot_id.clone()),
                    (
                        "source",
                        review
                            .identity
                            .workspace_identity
                            .document
                            .revision
                            .as_str()
                            .to_owned(),
                    ),
                    ("hash", review.identity.snapshot_hash.clone()),
                ],
            )
        })
        .unwrap_or_default();
    let content = model
        .review()
        .map(|review| {
            format!(
                "{}\n{}\n\n{}\n{}",
                lang.tr("snapshot_restore_before"),
                review.before_yaml,
                lang.tr("snapshot_restore_after"),
                review.restored_yaml
            )
        })
        .unwrap_or_default();
    RestorePresentation {
        status,
        details,
        content,
        confirm: model.can_confirm(),
        cancel: model.can_cancel(),
        retry: model.can_retry(),
    }
}
