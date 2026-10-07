//! Locale replay updates mounted snapshot labels without replacing controls or selection.
use crate::pages::profiles::LastProfilesProjection;
use crate::pages::profiles_diff::{SnapshotDiffSummaryText, SnapshotDiffViewState};
use crate::pages::profiles_diff_history::{SnapshotEntryLabel, SnapshotHistorySummaryText};
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::snapshot_presentation;
use infiltrator_bevy_widgets::localization::UiLocale;
#[derive(QueryData)]
#[query_data(mutable)]
pub struct CopyTarget {
    text: &'static mut Text,
    diff: Has<SnapshotDiffSummaryText>,
    entry: Option<&'static SnapshotEntryLabel>,
}
#[derive(QueryFilter)]
pub struct CopyFilter {
    kinds: Or<(
        With<SnapshotDiffSummaryText>,
        With<SnapshotHistorySummaryText>,
        With<SnapshotEntryLabel>,
    )>,
}
pub fn replay(
    locale: Res<UiLocale>,
    last: Res<LastProfilesProjection>,
    view: Res<SnapshotDiffViewState>,
    mut targets: Query<CopyTarget, CopyFilter>,
) {
    let projection = last.0.as_ref();
    for mut target in &mut targets {
        let copy = if let Some(entry) = target.entry {
            snapshot_presentation::entry_label(&entry.0, locale.code())
        } else if target.diff {
            snapshot_presentation::diff_summary(
                projection.and_then(|p| p.yaml_ast_diff.as_ref()),
                locale.code(),
            )
        } else {
            snapshot_presentation::history_summary(
                projection.and_then(|p| p.snapshot_history.as_ref()),
                view.prune_keep,
                locale.code(),
            )
        };
        if target.text.0 != copy {
            target.text.0 = copy;
        }
    }
}
