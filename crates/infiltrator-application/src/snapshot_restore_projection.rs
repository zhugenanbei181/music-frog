//! One locale-aware review fold, preserving opaque source identities and complete YAML.
use crate::snapshot_restore_workbench::SnapshotRestoreWorkbench;
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
    let status = if let Some(failure) = &model.failure {
        localize(
            locale,
            "snapshot_restore_failed",
            &[("reason", failure.message.clone())],
        )
    } else {
        lang.tr(if model.busy() {
            "snapshot_restore_pending"
        } else if model.restored.is_some() {
            "snapshot_restore_committed"
        } else if model.review.is_some() {
            "snapshot_restore_ready"
        } else {
            "snapshot_restore_preparing"
        })
        .into_owned()
    };
    let details = model
        .review
        .as_ref()
        .map(|review| {
            localize(
                locale,
                "snapshot_restore_details",
                &[
                    ("profile", review.target.profile.clone()),
                    ("snapshot", review.target.snapshot_id.clone()),
                    ("source", review.identity.source.document_hash.clone()),
                    ("hash", review.identity.snapshot_hash.clone()),
                ],
            )
        })
        .unwrap_or_default();
    let content = model
        .review
        .as_ref()
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
        confirm: model.visible
            && !model.busy()
            && model.failure.is_none()
            && model.review.is_some()
            && model.restored.is_none(),
        cancel: model.visible && !model.busy(),
        retry: model.can_retry(),
    }
}
