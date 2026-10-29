//! The editor family page of the Slint surface.
//!
//! The script sandbox, the reviewed script export, the configuration
//! difference and the snapshot restoration share one page.

use super::{Tone, button, header, lock, row, row_action, text};
use crate::WorkbenchItem;
use crate::surface::action::workbench::{
    DIFF_CLOSE, DIFF_LOAD, RESTORE_CANCEL, RESTORE_CONFIRM, RESTORE_PREPARE, SCRIPT_CLEAR,
    SCRIPT_EXPORT_CANCEL, SCRIPT_EXPORT_PREPARE, SCRIPT_EXPORT_SAVE, SCRIPT_PRESET, SCRIPT_RETRY,
    SCRIPT_RUN,
};
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::script_engine::ScriptEngine;
use infiltrator_presentation::script_console_projection;
use infiltrator_presentation::script_export_projection;
use infiltrator_presentation::snapshot_presentation::{diff_summary, history_summary};
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The editor family page: the script sandbox, the export review, the
/// configuration difference and the snapshot restoration.
pub(super) fn page(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> (
    Vec<WorkbenchItem>,
    Option<String>,
    Option<String>,
    String,
    String,
) {
    let mut items = Vec::new();
    let script = lock(&workbenches.script);
    let sandbox = script.snapshot();
    items.push(header(
        "script-sandbox",
        &catalog.text("script_sandbox_title"),
        &catalog.text(if script.busy() {
            "script_workbench_running"
        } else {
            "script_workbench_unobserved"
        }),
        Tone::Neutral,
    ));
    if let Some(report) = sandbox {
        let presentation = script_console_projection::project_report(report, catalog);
        items.push(text(
            "script-sandbox-report",
            &format!(
                "{} · {} · {}",
                presentation.engine, presentation.capabilities, presentation.breaker
            ),
        ));
        items.push(text("script-sandbox-output", &presentation.output));
        for directive in presentation.directives.iter().take(32) {
            items.push(row(
                &format!("script-sandbox/{}", directive),
                directive,
                "",
                "",
            ));
        }
    }
    if let Some(failure) = script.error_detail() {
        items.push(text("script-sandbox-error", failure));
    }
    items.push(button(
        "script-run",
        &catalog.text("script_sandbox_run"),
        SCRIPT_RUN,
        !script.busy(),
    ));
    items.push(button(
        "script-clear",
        &catalog.text("common_clear"),
        SCRIPT_CLEAR,
        !script.busy(),
    ));
    items.push(button(
        "script-retry",
        &catalog.text("script_workbench_retry"),
        SCRIPT_RETRY,
        script.can_retry(),
    ));
    for preset in ScriptEngine::builtin_presets() {
        let label = preset_label(preset.id, catalog);
        items.push(row_action(
            &format!("script-preset/{}", preset.id),
            preset.id,
            &label,
            preset.id,
            &catalog.text("script_sandbox_presets"),
            SCRIPT_PRESET,
            &label,
        ));
    }
    let script_draft = script.script_code().to_owned();

    let export = script_export_projection::project_script_export(&script, catalog);
    items.push(header(
        "script-export",
        &catalog.text("script_export_title"),
        &export.status,
        Tone::Neutral,
    ));
    items.push(text("script-export-detail", &export.details));
    if !export.content.is_empty() {
        items.push(text("script-export-content", &export.content));
    }
    items.push(button(
        "script-export-prepare",
        &catalog.text("script_export_review_instructions"),
        SCRIPT_EXPORT_PREPARE,
        !script.is_export_visible(),
    ));
    items.push(button(
        "script-export-save",
        &catalog.text("common_save_apply"),
        SCRIPT_EXPORT_SAVE,
        export.confirm,
    ));
    items.push(button(
        "script-export-cancel",
        &catalog.text("dns_cache_cancel"),
        SCRIPT_EXPORT_CANCEL,
        export.close,
    ));

    let difference = lock(&workbenches.difference);
    let published = snapshot.yaml_ast_diff.as_ref();
    let shown = published.filter(|diff| difference.shows(&diff.source_id));
    items.push(header(
        "config-diff",
        &catalog.text("snapshot_diff_title"),
        &match shown {
            Some(diff) => diff.source_id.clone(),
            None => catalog.text("snapshot_diff_unobserved").into_owned(),
        },
        Tone::Neutral,
    ));
    if let Some(diff) = shown {
        items.push(text(
            "config-diff-summary",
            &diff_summary(Some(diff), catalog),
        ));
        for line in diff.unified_lines.iter().take(64) {
            items.push(row(
                &format!("config-diff/{}", line.content),
                &line.content,
                "",
                "",
            ));
        }
    }
    items.push(button(
        "config-diff-load",
        &catalog.text("snapshot_diff_refresh"),
        DIFF_LOAD,
        true,
    ));
    items.push(button(
        "config-diff-close",
        &catalog.text("dns_cache_cancel"),
        DIFF_CLOSE,
        shown.is_some(),
    ));

    let history = snapshot
        .pages
        .profiles
        .data
        .as_ref()
        .and_then(|page| page.snapshot_history.as_ref());
    let restore = lock(&workbenches.restore);
    items.push(header(
        "snapshot-restore",
        &catalog.text("snapshot_restore_title"),
        &history_summary(history, history.map(|h| h.keep_limit).unwrap_or(5), catalog),
        Tone::Neutral,
    ));
    if let Some(history) = history {
        for entry in history.entries.iter().take(32) {
            items.push(row(
                &format!("snapshot-restore/{}", entry.id),
                &entry.file_name,
                &entry.id,
                &entry.sha256,
            ));
        }
    }
    if let Some(failure) = restore.failure() {
        items.push(text("snapshot-restore-error", &failure.message));
    }
    items.push(button(
        "snapshot-restore-prepare",
        &catalog.text("snapshot_restore_ready"),
        RESTORE_PREPARE,
        history.is_some_and(|history| !history.entries.is_empty()),
    ));
    items.push(button(
        "snapshot-restore-confirm",
        &catalog.text("common_save_apply"),
        RESTORE_CONFIRM,
        restore.can_confirm(),
    ));
    items.push(button(
        "snapshot-restore-cancel",
        &catalog.text("snapshot_restore_cancel"),
        RESTORE_CANCEL,
        restore.can_cancel(),
    ));

    (
        items,
        Some(script_draft),
        None,
        "script-code".to_owned(),
        String::new(),
    )
}

/// The localized label of one built-in script preset.
///
/// The preset catalogue belongs to the shared domain; the label of every
/// preset belongs to the shared message catalogue.
fn preset_label(preset: &str, catalog: &Lang<'_>) -> String {
    let key = match preset {
        "remove-ads" => "script_preset_remove_ads",
        "auto-country-groups" => "script_preset_country",
        "streaming-groups" => "script_preset_streaming",
        "direct-china" => "script_preset_direct_china",
        _ => "script_sandbox_presets",
    };
    catalog.text(key).into_owned()
}
