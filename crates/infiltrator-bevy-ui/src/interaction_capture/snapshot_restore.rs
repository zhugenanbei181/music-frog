//! The capture drives a real restoration request and verifies native card and action geometry.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::{CaptureObservationSet, InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::snapshot_restore::{OpenSnapshotRestore, RestoreState};
use crate::pages::snapshot_restore_scene::{RestoreCard, RestoreControl};
use crate::pages::snapshot_restore_view::replay;
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, PostUpdate, Update};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_composition::snapshot_restore_fixture::SnapshotRestoreFixture;
use infiltrator_contract::parity::FeatureId;
use std::mem::discriminant;
use std::sync::Arc;
#[derive(Resource)]
struct RestoreFixture {
    fixture: SnapshotRestoreFixture,
    launched: bool,
}
pub fn install(app: &mut App) {
    let fixture = SnapshotRestoreFixture::new();
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_snapshots(fixture.application.clone()),
        CaptureCapability::SnapshotRestore,
    ))));
    app.insert_resource(RestoreFixture {
        fixture,
        launched: false,
    });
    app.add_systems(Update, activate).add_systems(
        PostUpdate,
        observe.in_set(CaptureObservationSet).after(replay),
    );
}
fn activate(
    feature: Res<InteractionCapture>,
    route: Res<ActiveRoute>,
    mut fixture: ResMut<RestoreFixture>,
    mut commands: Commands,
) {
    if selected(
        &feature,
        &route,
        FeatureId::ProfilesSnapshotRestoreConfirm,
        Route::Profiles,
    ) && !fixture.launched
    {
        fixture.launched = true;
        commands.trigger(OpenSnapshotRestore(fixture.fixture.target.clone()));
    }
}
fn observe(
    state: Res<RestoreState>,
    fixture: Res<RestoreFixture>,
    cards: Query<Entity, With<RestoreCard>>,
    controls: Query<(Entity, &RestoreControl, &ButtonDisabled), Without<ModalScrim>>,
    mut geometry: CaptureGeometry,
    mut feature: ResMut<InteractionCapture>,
    mut observed: ResMut<ObservedInteraction>,
) {
    if !state.model.visible
        || state.model.busy()
        || state.model.failure.is_some()
        || state.model.review.is_none()
        || fixture.fixture.writes() != 0
    {
        return;
    }
    let Some(card) = cards
        .iter()
        .next()
        .and_then(|entity| geometry.bounds(entity, "snapshot restore review"))
    else {
        return;
    };
    for role in [RestoreControl::Confirm, RestoreControl::Cancel] {
        let Some((entity, _, disabled)) = controls
            .iter()
            .find(|(_, control, _)| discriminant(*control) == discriminant(&role))
        else {
            return;
        };
        let Some(bounds) = geometry.bounds(entity, "snapshot restore action") else {
            return;
        };
        if disabled.0
            || bounds[0] < card[0]
            || bounds[1] < card[1]
            || bounds[0] + bounds[2] > card[0] + card[2]
            || bounds[1] + bounds[3] > card[1] + card[3]
        {
            return;
        }
    }
    observed.publish(&mut feature, Some(card));
}
