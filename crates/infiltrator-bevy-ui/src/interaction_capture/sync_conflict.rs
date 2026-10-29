//! Scoped activation of the real sync conflict resolution card on the Sync page.
use super::geometry::CaptureGeometry;
use super::{CaptureObservationSet, InteractionCapture, ObservedInteraction, selected};
use crate::pages::sync::{
    ConflictCardContainer, ConflictingKey, KeepLocalConflictButton, LastSyncProjection,
    SyncConflictInfo, SyncProjection,
};
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, PostUpdate, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::sync::SyncStatus;

#[derive(Resource, Default)]
pub(super) struct SyncConflictCapture {
    pub(super) published: bool,
}

pub fn install(app: &mut App) {
    app.init_resource::<SyncConflictCapture>();
    app.add_systems(Update, activate);
    app.add_systems(PostUpdate, observe.in_set(CaptureObservationSet));
}

pub fn activate(
    feature: Res<'_, InteractionCapture>,
    route: Res<'_, ActiveRoute>,
    mut capture: ResMut<'_, SyncConflictCapture>,
    mut last_sync: ResMut<'_, LastSyncProjection>,
) {
    if !selected(
        &feature,
        &route,
        FeatureId::SyncConflictResolver,
        Route::Sync,
    ) {
        return;
    }
    if !capture.published {
        capture.published = true;
        let mut proj = last_sync.0.clone().unwrap_or_else(SyncProjection::demo);
        proj.status = SyncStatus::Conflict;
        proj.conflict = Some(SyncConflictInfo {
            remote_device: "MacBook Pro".to_string(),
            conflict_time: "2026-10-28 12:00".to_string(),
            conflicting_keys: vec![ConflictingKey {
                key: "mode".to_string(),
                local_value: "Rule".to_string(),
                remote_value: "Global".to_string(),
            }],
        });
        last_sync.0 = Some(proj);
    }
}

pub fn observe(
    mut feature: ResMut<'_, InteractionCapture>,
    route: Res<'_, ActiveRoute>,
    capture: Res<'_, SyncConflictCapture>,
    containers: Query<'_, '_, Entity, With<ConflictCardContainer>>,
    buttons: Query<'_, '_, Entity, With<KeepLocalConflictButton>>,
    mut geometry: CaptureGeometry<'_, '_>,
    mut observation: ResMut<'_, ObservedInteraction>,
) {
    if !selected(
        &feature,
        &route,
        FeatureId::SyncConflictResolver,
        Route::Sync,
    ) || !capture.published
    {
        return;
    }
    let bounds = (|| {
        let container = containers.iter().next()?;
        if !geometry.visible(container) {
            return None;
        }
        let button = buttons.iter().next()?;
        if !geometry.visible(button) {
            return None;
        }
        geometry.bounds(container, "sync conflict panel")
    })();
    observation.publish(&mut feature, bounds);
}
