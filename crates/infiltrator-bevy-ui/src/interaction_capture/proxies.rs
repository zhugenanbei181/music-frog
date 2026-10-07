//! Fold captures execute both native transitions, then scroll the real expanded node into view.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::proxies::{
    GroupNodesContainer, ProxiesProjectionUpdated, ProxiesScrollArea, ProxyGroupFoldButton,
    ProxyNodeButton,
};
use crate::pages::proxies_identity::ProxyGroupIdentity;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, proxies_projection};
use bevy::app::App;
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::project_proxy_groups;
use infiltrator_contract::parity::FeatureId;
use std::sync::Arc;

#[derive(Resource, Default)]
struct FoldCapture {
    preferences: ProxyPreferencesApplication,
    group: Option<String>,
    stage: u8,
}

pub fn install(app: &mut App) {
    let capture = FoldCapture::default();
    app.insert_resource(CommandSinkHandle(Arc::new(CaptureCommandSink::new(
        CommandApplication::new().with_proxy_preferences(capture.preferences.clone()),
        CaptureCapability::GroupExpansion,
    ))));
    app.insert_resource(capture);
}

#[derive(SystemParam)]
pub struct FoldSurface<'w, 's> {
    capture: ResMut<'w, FoldCapture>,
    route: Res<'w, ActiveRoute>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    groups: Query<'w, 's, (&'static ProxyGroupIdentity, &'static Node), With<GroupNodesContainer>>,
    folds: Query<'w, 's, (Entity, &'static ProxyGroupFoldButton)>,
    nodes: Query<'w, 's, (Entity, &'static ProxyNodeButton)>,
    scroll: Query<'w, 's, Entity, With<ProxiesScrollArea>>,
}
impl FoldSurface<'_, '_> {
    fn visible(&self, name: &str) -> bool {
        self.groups
            .iter()
            .any(|(identity, node)| identity.0 == name && node.display == Display::Flex)
    }
    fn expanded_node(&self) -> Option<Entity> {
        let name = self.capture.group.as_deref()?;
        if self.capture.stage != 2
            || !self
                .capture
                .preferences
                .is_group_expanded(name)
                .expect("capture preferences")
            || !self.visible(name)
        {
            return None;
        }
        self.nodes
            .iter()
            .find(|(_, node)| node.group_name == name)
            .map(|(entity, _)| entity)
    }
}
pub fn activate(
    mut surface: FoldSurface,
    feature: Res<InteractionCapture>,
    geometry: CaptureGeometry,
    mut commands: Commands,
) {
    if !selected(
        &feature,
        &surface.route,
        FeatureId::ProxiesGroupExpanded,
        Route::Proxies,
    ) {
        return;
    }
    // Replay the actual preference result only after the native observer ran.
    let preferences = surface
        .capture
        .preferences
        .preferences()
        .expect("capture preferences");
    let mut snapshot = surface.latest.0.clone();
    if let Some(page) = snapshot.pages.proxies.data.as_mut() {
        page.groups = project_proxy_groups(page.groups.clone(), &preferences);
        let wanted = proxies_projection(&snapshot);
        commands.trigger(ProxiesProjectionUpdated(wanted));
    }
    if surface.capture.stage == 2 {
        if let (Some(node), Some(scroll)) = (surface.expanded_node(), surface.scroll.iter().next())
            && let (Some(bounds), Some(viewport)) = (geometry.rect(node), geometry.rect(scroll))
        {
            request_scroll(&mut commands, scroll, bounds, viewport);
        }
        return;
    }
    let Some((entity, group)) = surface
        .folds
        .iter()
        .find(|(_, button)| {
            surface
                .capture
                .group
                .as_ref()
                .is_none_or(|name| name == &button.group_name)
        })
        .map(|(entity, button)| (entity, button.group_name.clone()))
    else {
        return;
    };
    if surface.capture.stage == 1 && surface.visible(&group) {
        return;
    }
    surface.capture.group = Some(group.clone());
    commands.trigger(Activate { entity });
    surface.capture.stage += 1;
}
pub fn observe(
    surface: FoldSurface,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    let bounds = surface
        .expanded_node()
        .and_then(|node| geometry.bounds(node, "expanded proxy node"));
    observation.publish(&mut state, bounds);
}
