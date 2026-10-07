//! Scoped native component access for sync systems.

use super::{
    ConflictCardContainer, ConflictSummaryText, RestoreSnapshotButton, SnapshotDeviceText,
    SnapshotSizeText, SyncLine,
};
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::prelude::Node;
use bevy::ui::widget::Text;

#[derive(QueryFilter)]
pub struct ApplySyncProjectionLinesFilter {
    with_sync_line: With<SyncLine>,
    without_conflict_summary_text: Without<ConflictSummaryText>,
    without_snapshot_device_text: Without<SnapshotDeviceText>,
    without_snapshot_size_text: Without<SnapshotSizeText>,
}

#[derive(QueryFilter)]
pub struct ApplySyncProjectionConflictTextsFilter {
    with_conflict_summary_text: With<ConflictSummaryText>,
    without_sync_line: Without<SyncLine>,
    without_snapshot_device_text: Without<SnapshotDeviceText>,
    without_snapshot_size_text: Without<SnapshotSizeText>,
}

#[derive(QueryFilter)]
pub struct ApplySyncProjectionSnapshotDevicesFilter {
    with_snapshot_device_text: With<SnapshotDeviceText>,
    without_sync_line: Without<SyncLine>,
    without_conflict_summary_text: Without<ConflictSummaryText>,
    without_snapshot_size_text: Without<SnapshotSizeText>,
}

#[derive(QueryFilter)]
pub struct ApplySyncProjectionSnapshotSizesFilter {
    with_snapshot_size_text: With<SnapshotSizeText>,
    without_sync_line: Without<SyncLine>,
    without_conflict_summary_text: Without<ConflictSummaryText>,
    without_snapshot_device_text: Without<SnapshotDeviceText>,
}

#[derive(SystemParam)]
pub struct SyncProjectionTargets<'w, 's> {
    pub(super) lines:
        Query<'w, 's, (&'static mut Text, &'static SyncLine), ApplySyncProjectionLinesFilter>,
    pub(super) conflict_texts: Query<
        'w,
        's,
        (&'static mut Text, &'static ConflictSummaryText),
        ApplySyncProjectionConflictTextsFilter,
    >,
    pub(super) conflict_containers: Query<'w, 's, &'static mut Node, With<ConflictCardContainer>>,
    pub(super) snapshot_devices: Query<
        'w,
        's,
        (&'static mut Text, &'static SnapshotDeviceText),
        ApplySyncProjectionSnapshotDevicesFilter,
    >,
    pub(super) snapshot_sizes: Query<
        'w,
        's,
        (&'static mut Text, &'static SnapshotSizeText),
        ApplySyncProjectionSnapshotSizesFilter,
    >,
    pub(super) restore_buttons: Query<'w, 's, &'static mut RestoreSnapshotButton>,
}
