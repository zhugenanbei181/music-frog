//! Scoped native component access for profiles diff history systems.

use super::{SnapshotHistoryBody, SnapshotHistoryEntryButton};
use crate::pages::profiles_diff::RollbackSnapshotLabel;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;

#[derive(SystemParam)]
pub struct SnapshotHistorySelectionControls<'w, 's> {
    pub(super) buttons: Query<'w, 's, &'static SnapshotHistoryEntryButton>,
    pub(super) labels:
        Query<'w, 's, (&'static mut Text, &'static mut LocalizedText), With<RollbackSnapshotLabel>>,
    pub(super) history_bodies: Query<'w, 's, Entity, With<SnapshotHistoryBody>>,
}

/// Native history rendering consumes one resolved appearance context.
#[derive(SystemParam)]
pub struct HistoryAppearance<'w> {
    pub(super) palette: Res<'w, UiPalette>,
    pub(super) locale: Res<'w, UiLocale>,
}
