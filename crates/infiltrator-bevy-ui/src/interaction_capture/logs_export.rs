//! Activate the real native export launcher and confirmation, then require its visible failure panel.
use super::geometry::CaptureGeometry;
use super::logs::LogsCapture;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::logs::ExportLogsButton;
use crate::pages::logs_export::{
    LogsExportAction, LogsExportCard, LogsExportLine, LogsExportState,
};
use crate::route::{ActiveRoute, Route};
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::parity::FeatureId;
use std::time::Duration;

#[derive(SystemParam)]
pub struct ExportControls<'w, 's> {
    capture: ResMut<'w, LogsCapture>,
    route: Res<'w, ActiveRoute>,
    state: Res<'w, LogsExportState>,
    launchers: Query<'w, 's, Entity, With<ExportLogsButton>>,
    actions: Query<'w, 's, (Entity, &'static LogsExportAction), Without<ModalScrim>>,
    cards: Query<'w, 's, Entity, With<LogsExportCard>>,
    lines: Query<'w, 's, (Entity, &'static LogsExportLine)>,
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
}
pub fn activate(
    mut controls: ExportControls,
    feature: Res<InteractionCapture>,
    mut geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &controls.route,
        FeatureId::LogsRedactedExport,
        Route::Logs,
    ) {
        return;
    }
    let target = match controls.capture.stage {
        1 if !controls.state.model.open => controls.launchers.single().ok(),
        2 if controls.state.model.pending.is_none() && controls.state.model.summary.is_some() => {
            controls
                .actions
                .iter()
                .find(|(_, action)| matches!(action, LogsExportAction::Confirm))
                .map(|(entity, _)| entity)
        }
        _ => None,
    };
    let (Some(entity), Ok(window)) = (target, controls.windows.single()) else {
        return;
    };
    let Some(bounds) = geometry.bounds(entity, "logs-export-native-action") else {
        return;
    };
    let pointer = || {
        Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::None {
                    width: window.width() as u32,
                    height: window.height() as u32,
                },
                position: Vec2::new(bounds[0] + bounds[2] / 2.0, bounds[1] + bounds[3] / 2.0),
            },
        )
    };
    commands.trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    commands.trigger(PointerClick {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
        duration: Duration::from_millis(40),
    });
    controls.capture.stage += 1;
}
pub fn observe(
    controls: ExportControls,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if !selected(
        &feature,
        &controls.route,
        FeatureId::LogsRedactedExport,
        Route::Logs,
    ) {
        return;
    }
    let model = &controls.state.model;
    if !model.open
        || model.pending.is_some()
        || model.receipt.is_some()
        || !model
            .summary
            .as_ref()
            .is_some_and(|summary| summary.records == 3)
        || !model
            .failure
            .as_ref()
            .is_some_and(|failure| failure.code == ErrorCode::Permission)
    {
        return;
    }
    let bounds = (|| {
        for role in [LogsExportLine::Status, LogsExportLine::Details] {
            let (entity, _) = controls.lines.iter().find(|(_, marker)| {
                matches!(
                    (marker, role),
                    (LogsExportLine::Status, LogsExportLine::Status)
                        | (LogsExportLine::Details, LogsExportLine::Details)
                )
            })?;
            geometry.bounds(entity, "logs-export-review-copy")?;
        }
        for role in [LogsExportAction::Retry, LogsExportAction::Cancel] {
            let (entity, _) = controls.actions.iter().find(|(_, marker)| {
                matches!(
                    (marker, role),
                    (LogsExportAction::Retry, LogsExportAction::Retry)
                        | (LogsExportAction::Cancel, LogsExportAction::Cancel)
                )
            })?;
            geometry.bounds(entity, "logs-export-recovery-action")?;
        }
        geometry.bounds(controls.cards.single().ok()?, "logs-export-modal")
    })();
    observed.publish(&mut feature, bounds);
}
