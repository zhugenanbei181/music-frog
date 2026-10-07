//! Scoped native component access for profiles diff systems.

use super::super::profiles_diff_history::{SnapshotHistoryBody, SnapshotHistorySummaryText};
use super::{SnapshotDiffBody, SnapshotDiffModeButton, SnapshotDiffSummaryText};
use crate::pages::profiles::ProfileProtectionText;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Query, SystemParam};
use bevy::ui::prelude::BackgroundColor;
use bevy::ui::widget::Text;

#[derive(QueryFilter)]
pub struct SyncSnapshotDiffSummaryFilter {
    with_snapshot_diff_summary_text: With<SnapshotDiffSummaryText>,
    without_profile_protection_text: Without<ProfileProtectionText>,
    without_snapshot_history_summary_text: Without<SnapshotHistorySummaryText>,
}

#[derive(QueryFilter)]
pub struct SyncSnapshotDiffHistorySummariesFilter {
    with_snapshot_history_summary_text: With<SnapshotHistorySummaryText>,
    without_snapshot_diff_summary_text: Without<SnapshotDiffSummaryText>,
    without_profile_protection_text: Without<ProfileProtectionText>,
}

#[derive(SystemParam)]
pub struct SnapshotDiffTargets<'w, 's> {
    pub(super) summary: Query<'w, 's, &'static mut Text, SyncSnapshotDiffSummaryFilter>,
    pub(super) mode_buttons: Query<
        'w,
        's,
        (
            &'static SnapshotDiffModeButton,
            &'static mut BackgroundColor,
        ),
    >,
    pub(super) bodies: Query<'w, 's, Entity, With<SnapshotDiffBody>>,
    pub(super) history_summaries:
        Query<'w, 's, &'static mut Text, SyncSnapshotDiffHistorySummariesFilter>,
    pub(super) history_bodies: Query<'w, 's, Entity, With<SnapshotHistoryBody>>,
}

#[derive(SystemParam)]
pub struct SnapshotDiffModeControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, &'static SnapshotDiffModeButton>,
    pub(super) bodies: Query<'w, 's, Entity, With<SnapshotDiffBody>>,
    pub(super) mode_buttons: Query<
        'w,
        's,
        (
            &'static SnapshotDiffModeButton,
            &'static mut BackgroundColor,
        ),
    >,
}
