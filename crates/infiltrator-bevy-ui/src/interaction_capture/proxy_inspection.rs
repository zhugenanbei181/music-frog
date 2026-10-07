//! Inject shared demo observations, then activate the real native information control.
use super::{InteractionCapture, selected};
use crate::pages::proxies::NodeDetailButton;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
use infiltrator_application::proxy_inspection_fixtures::{
    INSPECTION_GROUP, INSPECTION_NODE, observed_proxy,
};
use infiltrator_application::proxy_inspection_projection::project_proxy_inspection;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::proxies::ProxyGroupClassification;
use infiltrator_contract::surface_snapshot::{ProxyGroupSnapshot, ProxyNodeSnapshot};

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
        let page = snapshot
            .pages
            .proxies
            .data
            .as_mut()
            .expect("inspection fixture page");
        page.groups = vec![ProxyGroupSnapshot {
            name: INSPECTION_GROUP.into(),
            group_type: "Selector".into(),
            classification: Some(ProxyGroupClassification::Selector),
            current: INSPECTION_NODE.into(),
            expanded: true,
            proxies: vec![ProxyNodeSnapshot {
                name: INSPECTION_NODE.into(),
                node_type: "Shadowsocks".into(),
                delay_ms: Some(42),
                alive: Some(true),
                selected: true,
                favorite: false,
                features: vec!["UDP".into()],
            }],
        }];
        page.node_details = vec![project_proxy_inspection(INSPECTION_NODE, &observed_proxy())];
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
