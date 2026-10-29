//! The business panels of the logs workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;
use std::sync::PoisonError;

/// The business panels the logs page carries.
pub(super) fn panels(
    snapshot: &SurfaceSnapshot,
    catalog: &Lang<'_>,
    workbenches: &Workbenches,
) -> Vec<Panel> {
    let page = snapshot.pages.logs.data.as_ref();
    let entries = page.map(|page| page.total_entries).unwrap_or(0);
    let level = page
        .and_then(|page| page.active_level.clone())
        .unwrap_or_else(|| "all".to_owned());
    let stream = page
        .map(|page| format!("{:?}", page.stream))
        .unwrap_or_default();
    let rows = page
        .map(|page| {
            page.entries
                .iter()
                .map(|entry| format!("{} · {}", entry.tag, entry.message))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let failure = super::page_failure(&snapshot.pages.logs.status);
    let total_bytes: usize = page
        .map(|page| {
            page.entries
                .iter()
                .map(|e| e.message.len() + e.tag.len())
                .sum()
        })
        .unwrap_or(0);
    let export_details = catalog
        .text("logs_export_details")
        .replace("{records}", &entries.to_string())
        .replace("{bytes}", &format_bytes(total_bytes as u64));
    let mut export = Panel::new(
        BusinessId::LogsRedactedExport,
        catalog.text("logs_export_title"),
        format!("{entries}"),
        vec![
            export_details,
            catalog.text("logs_export_review").into_owned(),
            level.clone(),
        ],
    );
    export.facts.extend(rows.clone());

    let follow = workbenches
        .log_follow
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let follow_label = catalog.text(follow.label_key()).into_owned();
    let following = follow.should_follow();
    drop(follow);
    let buffer_summary = catalog
        .text("logs_buffer_summary")
        .replace("{count}", &entries.to_string());
    let mut lock = Panel::new(
        BusinessId::LogsScrollLock,
        catalog.text("logs_scroll_lock"),
        stream,
        vec![follow_label, buffer_summary, format!("{following}")],
    );
    let empty = super::page_empty(&snapshot.pages.logs.status);
    export.failure = failure.clone();
    export.empty = empty;
    lock.facts.extend(rows);
    lock.failure = failure.clone();
    lock.empty = empty;
    vec![export, lock]
}
