//! The synchronisation page of the Slint surface.
//!
//! The conflict resolver and the backup restore share one page.

use super::{Tone, button, header, row, row_action, text};
use crate::WorkbenchItem;
use crate::surface::action::workbench::{
    RESTORE_CANCEL, RESTORE_CONFIRM, SYNC_BACKUP, SYNC_CONFLICT_CANCEL, SYNC_KEEP_LOCAL, SYNC_NOW,
    SYNC_RESTORE, SYNC_TAKE_REMOTE,
};
use crate::surface::workbenches::Workbenches;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::sync::SyncStatus;
use infiltrator_presentation::sync_projection;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The synchronisation conflict and the backup page.
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
    let page = snapshot.pages.sync.data.as_ref();
    items.push(header(
        "sync-conflict",
        &catalog.text("sync_conflict_resolver_title"),
        &sync_projection::summary(
            page.map_or(SyncStatus::Unknown, |page| page.status),
            catalog,
        ),
        Tone::Neutral,
    ));
    let resolver = super::lock(&workbenches.conflict);
    let conflict = page.and_then(|page| page.conflict.as_ref());
    let shown = conflict.filter(|conflict| resolver.shows(&conflict.conflict_time));
    if let Some(page) = page {
        items.push(text(
            "sync-server",
            &format!(
                "{} · {}",
                sync_projection::server(&page.server_url, catalog),
                sync_projection::username(&page.username, catalog)
            ),
        ));
        items.push(text(
            "sync-last",
            &sync_projection::last_sync(page.last_sync.as_deref(), catalog),
        ));
        if let Some(conflict) = shown {
            items.push(text(
                "sync-conflict-detail",
                &sync_projection::conflict(
                    &conflict.remote_device,
                    &conflict.conflict_time,
                    conflict.conflicting_keys.len(),
                    catalog,
                ),
            ));
            for (key, local, remote) in &conflict.conflicting_keys {
                items.push(row(
                    &format!("sync-conflict/{}", key),
                    key,
                    &sync_projection::conflict_field(key, local, remote, catalog),
                    "",
                ));
            }
        }
        for item in &page.snapshots {
            items.push(row_action(
                &format!("sync-snapshot/{}", item.id),
                &item.id,
                &item.profile,
                &item.timestamp,
                &item.device,
                SYNC_RESTORE,
                &catalog.text("sync_restore_version_action"),
            ));
        }
        items.push(text(
            "sync-history",
            &sync_projection::history_notice(&page.history_status, catalog),
        ));
    }
    items.push(button(
        "sync-now",
        &catalog.text("sync_now"),
        SYNC_NOW,
        true,
    ));
    items.push(button(
        "sync-keep-local",
        &catalog.text("sync_conflict_keep_local"),
        SYNC_KEEP_LOCAL,
        shown.is_some(),
    ));
    items.push(button(
        "sync-take-remote",
        &catalog.text("sync_conflict_take_remote"),
        SYNC_TAKE_REMOTE,
        shown.is_some(),
    ));
    items.push(button(
        "sync-conflict-cancel",
        &catalog.text("common_confirm"),
        SYNC_CONFLICT_CANCEL,
        shown.is_some(),
    ));
    items.push(button(
        "sync-backup",
        &catalog.text("common_create_backup"),
        SYNC_BACKUP,
        true,
    ));
    // The restoration review belongs to the shared workbench, so the page
    // that opens it also shows how it is confirmed or dismissed.
    let restore = super::lock(&workbenches.restore);
    if let Some(failure) = restore.failure() {
        items.push(text("snapshot-restore-error", &failure.message));
    }
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
    (items, None, None, String::new(), String::new())
}
