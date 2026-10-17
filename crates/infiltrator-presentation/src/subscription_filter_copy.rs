//! One localized status fold for both native filter editors.
use crate::subscription_filter_editor::SubscriptionFilterEditor;
use crate::text::localize;
use infiltrator_domain::redact::redact_line;
use lurker_ui_presentation::copy::TextCatalog;

pub fn status(editor: &SubscriptionFilterEditor, catalog: &dyn TextCatalog) -> String {
    if editor.pending().is_some() {
        return localize(catalog, "filter_form_saving", &[]);
    }
    if let Some(failure) = editor.read_failure() {
        return localize(
            catalog,
            "filter_form_read_failed",
            &[("reason", redact_line(&failure.message, &[]))],
        );
    }
    if editor.stale() {
        return localize(
            catalog,
            "filter_form_source_changed",
            &[
                ("source", editor.source_profile().unwrap_or("—").into()),
                ("current", editor.latest_profile().unwrap_or("—").into()),
            ],
        );
    }
    if let Some(failure) = editor.failure() {
        return localize(
            catalog,
            "filter_form_failed",
            &[("reason", redact_line(&failure.message, &[]))],
        );
    }
    if editor.source_profile().is_none() {
        localize(catalog, "filter_form_unobserved", &[])
    } else if let Some(report) = editor.report().filter(|_| editor.applied()) {
        localize(
            catalog,
            "filter_form_report",
            &[
                ("passed", report.passed.to_string()),
                ("total", report.total_input.to_string()),
                ("renamed", report.renamed.to_string()),
                ("dedup", report.deduplicated.to_string()),
            ],
        )
    } else if editor.dirty() {
        localize(catalog, "filter_form_dirty", &[])
    } else {
        String::new()
    }
}
