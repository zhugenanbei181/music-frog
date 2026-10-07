//! Shared editor presentation from neutral facts, with opaque values interpolated once.
use infiltrator_contract::apply_transaction::{ApplyTransactionSnapshot, ApplyTransactionStage};
use infiltrator_contract::editor_viewport::EditorViewport;
use infiltrator_contract::profile_document::SyntaxDiagnosticSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_domain::yaml_edit::format::FormatSkipReason;
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn format_note(reason: FormatSkipReason, locale: &str) -> String {
    Lang(locale)
        .tr(match reason {
            FormatSkipReason::AnchorsPresent => "yaml_format_skipped_anchors",
            FormatSkipReason::RootSequence => "yaml_format_skipped_root_sequence",
            FormatSkipReason::MergeKey => "yaml_format_skipped_merge_key",
            FormatSkipReason::UnclassifiedTopLevelLine => "yaml_format_skipped_unclassified",
            FormatSkipReason::NothingToOrder => "editor_format_nothing_to_order",
            FormatSkipReason::EmptyDocument => "editor_format_empty_document",
        })
        .into_owned()
}
pub fn protection_label(protection: ProfileWriteProtection, locale: &str) -> String {
    Lang(locale)
        .tr(match protection {
            ProfileWriteProtection::Editable => "editor_protection_local_label",
            ProfileWriteProtection::RemoteSubscription => "editor_protection_remote_label",
        })
        .into_owned()
}
pub fn protection_hint(protection: ProfileWriteProtection, locale: &str) -> String {
    Lang(locale)
        .tr(match protection {
            ProfileWriteProtection::Editable => "editor_protection_local_hint",
            ProfileWriteProtection::RemoteSubscription => "editor_protection_remote_hint",
        })
        .into_owned()
}
pub fn protection_banner(
    protection: ProfileWriteProtection,
    notice: Option<&str>,
    locale: &str,
) -> String {
    let mut parts = vec![
        protection_label(protection, locale),
        protection_hint(protection, locale),
    ];
    if let Some(notice) = notice {
        parts.push(notice.to_owned());
    }
    parts.join(" · ")
}
pub fn protection_action(
    protection: ProfileWriteProtection,
    unlocked: bool,
    locale: &str,
) -> String {
    Lang(locale)
        .tr(if !protection.is_protected() {
            "editor_protection_local_action"
        } else if unlocked {
            "editor_protection_restore_action"
        } else {
            "editor_protection_explicit_unlock"
        })
        .into_owned()
}
pub fn transaction(record: &ApplyTransactionSnapshot, locale: &str) -> String {
    let key = match record.stage {
        ApplyTransactionStage::Idle => "editor_apply_idle",
        ApplyTransactionStage::Committed if record.method.is_some() => {
            "editor_apply_committed_method"
        }
        ApplyTransactionStage::Committed => "editor_apply_committed",
        ApplyTransactionStage::RolledBack => "editor_apply_rolled_back",
        ApplyTransactionStage::RollbackFailed => "editor_apply_rollback_failed",
    };
    localize(
        locale,
        key,
        &[
            ("profile", record.profile.clone()),
            ("detail", record.detail.clone()),
            ("method", record.method.clone().unwrap_or_default()),
            (
                "rollback",
                record
                    .rollback_error
                    .clone()
                    .unwrap_or_else(|| Lang(locale).tr("editor_cause_unknown").into_owned()),
            ),
        ],
    )
}
pub fn viewport_range(viewport: &EditorViewport, locale: &str) -> String {
    localize(
        locale,
        "editor_window_range",
        &[
            ("first", (viewport.first_line() + 1).to_string()),
            ("last", (viewport.last_line() + 1).to_string()),
            ("total", viewport.total_lines().to_string()),
        ],
    )
}
pub struct EditorStatus<'a> {
    pub dirty: bool,
    pub line_count: usize,
    pub cursor_line: usize,
    pub cursor_column: usize,
    pub viewport: &'a EditorViewport,
    pub focused: bool,
    pub transaction: Option<&'a ApplyTransactionSnapshot>,
    pub notice: Option<&'a str>,
}
pub fn status(facts: &EditorStatus<'_>, locale: &str) -> String {
    let lang = Lang(locale);
    let mut parts = vec![
        lang.tr(if facts.dirty {
            "editor_unsaved"
        } else {
            "editor_matches_observation"
        })
        .into_owned(),
        localize(
            locale,
            "editor_cursor_readout",
            &[
                ("lines", facts.line_count.to_string()),
                ("line", facts.cursor_line.to_string()),
                ("column", facts.cursor_column.to_string()),
            ],
        ),
    ];
    if !facts.viewport.covers_document() {
        parts.push(viewport_range(facts.viewport, locale));
    }
    if facts.focused {
        parts.push(lang.tr("editor_keyboard_active").into_owned());
    }
    if let Some(record) = facts.transaction {
        parts.push(transaction(record, locale));
    }
    if let Some(notice) = facts.notice {
        parts.push(notice.to_owned());
    }
    parts.join(" · ")
}
pub fn diagnostic(value: Option<&SyntaxDiagnosticSnapshot>, locale: &str) -> (String, bool) {
    match value {
        Some(value) => (
            localize(
                locale,
                "editor_syntax_diagnostic",
                &[
                    ("line", value.line.to_string()),
                    ("column", value.column.to_string()),
                    ("reason", value.message.clone()),
                ],
            ),
            true,
        ),
        None => (Lang(locale).tr("editor_syntax_valid").into_owned(), false),
    }
}
pub fn syntax_label(has_error: bool, locale: &str) -> String {
    Lang(locale)
        .tr(if has_error {
            "editor_syntax_failed_label"
        } else {
            "editor_syntax_valid_label"
        })
        .into_owned()
}
pub fn title(lines: usize, locale: &str) -> String {
    localize(
        locale,
        "editor_document_title",
        &[("lines", lines.to_string())],
    )
}
pub fn hidden_lines(lines: usize, before: bool, locale: &str) -> String {
    localize(
        locale,
        if before {
            "editor_hidden_before"
        } else {
            "editor_hidden_after"
        },
        &[("lines", lines.to_string())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_causes_and_transaction_identities_remain_opaque_in_both_locales() {
        let record = ApplyTransactionSnapshot::rollback_failed(
            "profile {detail}/中文🙂",
            "cause {rollback}",
            "restore {profile}",
        );
        for locale in ["zh-CN", "en-US"] {
            let copy = transaction(&record, locale);
            assert!(copy.contains("profile {detail}/中文🙂"));
            assert!(copy.contains("cause {rollback}"));
            assert!(copy.contains("restore {profile}"));
        }
        assert!(transaction(&record, "en-US").contains("Rollback failed"));
        let diagnostic = SyntaxDiagnosticSnapshot {
            line: 2,
            column: 3,
            message: "reason {line}/🙂".into(),
        };
        assert_eq!(
            self::diagnostic(Some(&diagnostic), "en-US"),
            ("Line 2 : Col 3 · reason {line}/🙂".into(), true)
        );
    }
    #[test]
    fn viewport_and_protection_display_the_same_facts_with_native_locale() {
        let viewport = EditorViewport::new(1000, 80, 40);
        let copy = viewport_range(&viewport, "en-US");
        assert_eq!(copy, "Lines 41–120 / Total 1000");
        assert!(
            protection_label(ProfileWriteProtection::RemoteSubscription, "en-US")
                .contains("read-only")
        );
        assert_eq!(
            protection_action(ProfileWriteProtection::Editable, false, "en-US"),
            "Local profile: save directly"
        );
    }
}
