//! Shared snapshot copy keeps missing observations distinct and preserves opaque identities.
use infiltrator_contract::snapshot_history::{
    SnapshotEntry, SnapshotHistorySnapshot, SnapshotPruneSource,
};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn prune_source(source: SnapshotPruneSource, locale: &str) -> String {
    Lang(locale)
        .tr(match source {
            SnapshotPruneSource::Apply => "snapshot_prune_source_apply",
            SnapshotPruneSource::Manual => "snapshot_prune_source_manual",
        })
        .into_owned()
}
pub fn history_summary(
    history: Option<&SnapshotHistorySnapshot>,
    keep: usize,
    locale: &str,
) -> String {
    let Some(history) = history else {
        return localize(
            locale,
            "snapshot_history_unobserved",
            &[("keep", keep.to_string())],
        );
    };
    let mut summary = localize(
        locale,
        "snapshot_history_summary",
        &[
            ("profile", history.profile.clone()),
            ("count", history.entries.len().to_string()),
            ("keep", history.keep_limit.to_string()),
            ("pending", history.pending_prune.to_string()),
        ],
    );
    if history.duplicate_entries > 0 {
        summary.push_str(&localize(
            locale,
            "snapshot_history_duplicates",
            &[("count", history.duplicate_entries.to_string())],
        ));
    }
    if let Some(report) = history.last_prune {
        summary.push_str(&localize(
            locale,
            "snapshot_history_pruned",
            &[
                ("source", prune_source(report.source, locale)),
                ("count", report.removed.to_string()),
            ],
        ));
    }
    summary
}
pub fn entry_label(entry: &SnapshotEntry, locale: &str) -> String {
    let lang = Lang(locale);
    localize(
        locale,
        "snapshot_history_entry",
        &[
            ("stamp", entry.stamp_label()),
            ("hash", entry.short_hash().into()),
            (
                "newest",
                if entry.is_newest {
                    lang.tr("snapshot_history_newest").into_owned()
                } else {
                    String::new()
                },
            ),
            (
                "action",
                lang.tr(if entry.is_duplicate {
                    "snapshot_history_duplicate"
                } else {
                    "snapshot_diff_open"
                })
                .into_owned(),
            ),
        ],
    )
}
pub fn diff_summary(diff: Option<&YamlAstDiffSnapshot>, locale: &str) -> String {
    let Some(diff) = diff else {
        return Lang(locale).tr("snapshot_diff_unobserved").into_owned();
    };
    if !diff.has_differences() {
        return Lang(locale).tr("snapshot_diff_identical").into_owned();
    }
    localize(
        locale,
        "snapshot_diff_summary",
        &[
            ("source", diff.source_id.clone()),
            ("target", diff.target_id.clone()),
            ("changes", diff.change_summary()),
            (
                "fidelity",
                Lang(locale)
                    .tr(if diff.fidelity_preserved {
                        "snapshot_fidelity_preserved"
                    } else {
                        "snapshot_fidelity_degraded"
                    })
                    .into_owned(),
            ),
            ("grade", diff.fidelity_grade.as_str().into()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn localized_snapshot_copy_distinguishes_unknown_from_empty_and_preserves_opaque_user_identity()
    {
        let history = SnapshotHistorySnapshot::empty("用户 {count} profile");
        let unknown = history_summary(None, 20, "en-US");
        let empty = history_summary(Some(&history), 20, "en-US");
        assert!(unknown.contains("not observed"));
        assert!(empty.contains("用户 {count} profile · 0 snapshots"));
        assert!(!empty.contains("not observed"));
        assert_eq!(
            prune_source(SnapshotPruneSource::Apply, "en-US"),
            "Automatic prune after apply"
        );
        assert_eq!(
            prune_source(SnapshotPruneSource::Manual, "zh-CN"),
            "手动修剪"
        );
        let mut diff = YamlAstDiffSnapshot {
            source_id: "旧 {target}".into(),
            target_id: "新 {source}".into(),
            ..Default::default()
        };
        assert_eq!(
            diff_summary(None, "en-US"),
            "Snapshot difference not observed; refresh to compute it"
        );
        assert_eq!(
            diff_summary(Some(&diff), "en-US"),
            "Snapshot and current profile have identical contents"
        );
        diff.stats.additions = 1;
        let en = diff_summary(Some(&diff), "en-US");
        let zh = diff_summary(Some(&diff), "zh-CN");
        assert!(en.contains("旧 {target} → 新 {source}"));
        assert!(zh.contains("旧 {target} → 新 {source}"));
        assert!(en.contains("+1"));
        assert!(zh.contains("+1"));
    }
}
