//! Inject shared demo observations, then activate the real native information control.
use super::outbound_fixture::{INSPECTION_NODE, set_inspection_observations};
use super::{InteractionCapture, selected};
use crate::pages::proxies::NodeDetailButton;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
use infiltrator_contract::parity::FeatureId;

#[derive(Resource, Default)]
struct InspectionCapture {
    injected: bool,
}
pub fn install(app: &mut App) {
    app.init_resource::<InspectionCapture>();
}
#[derive(SystemParam)]
pub struct InspectionControls<'w, 's> {
    capture: ResMut<'w, InspectionCapture>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    route: Res<'w, ActiveRoute>,
    buttons: Query<'w, 's, (Entity, &'static NodeDetailButton)>,
}
pub fn activate(
    mut controls: InspectionControls,
    mut feature: ResMut<InteractionCapture>,
    mut commands: Commands,
) {
    if feature.activated
        || !selected(
            &feature,
            &controls.route,
            FeatureId::ProxiesNodeDetailDrawer,
            Route::Proxies,
        )
    {
        return;
    }
    if !controls.capture.injected {
        let mut snapshot = controls.latest.0.clone();
        snapshot.revision += 1;
        set_inspection_observations(&mut snapshot);
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
        controls.capture.injected = true;
        return;
    }
    if let Some((entity, _)) = controls
        .buttons
        .iter()
        .find(|(_, button)| button.node_name == INSPECTION_NODE)
    {
        commands.trigger(Activate { entity });
        feature.activated = true;
    }
}
