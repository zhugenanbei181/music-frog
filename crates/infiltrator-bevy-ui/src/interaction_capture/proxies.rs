//! Fold captures execute both native transitions, then scroll the real expanded node into view.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::outbound_preferences::OutboundPreferenceCaptureOwner;
use super::scroll::request_scroll;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::proxies::{
    GroupNodesContainer, ProxiesScrollArea, ProxyGroupFoldButton, ProxyNodeButton,
};
use crate::pages::proxies_identity::ProxyGroupIdentity;
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::ecs::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface_snapshot::PageData;
use std::sync::Arc;

#[derive(Resource)]
pub(super) struct FoldCapture {
    preferences: Option<OutboundPreferenceCaptureOwner>,
    group: Option<OutboundId>,
    stage: u8,
}

pub fn install(app: &mut App) {
    let capture = FoldCapture {
        preferences: None,
        group: None,
        stage: 0,
    };
    app.insert_resource(capture);
}
pub(super) fn initialize(
    latest: Res<LatestSurfaceSnapshot>,
    mut capture: ResMut<FoldCapture>,
    mut commands: Commands,
) {
    if capture.preferences.is_some() {
        return;
    }
    let Some(preferences) = latest.0.outbound_ui_preferences.data.clone() else {
        return;
    };
    let owner = OutboundPreferenceCaptureOwner::new(preferences);
    commands.insert_resource(CommandSinkHandle(Arc::new(
        CaptureCommandSink::with_outbound_preferences(
            CommandApplication::new(),
            CaptureCapability::GroupExpansion,
            owner.clone(),
        ),
    )));
    capture.preferences = Some(owner);
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
    fn visible(&self, collection_id: &OutboundId) -> bool {
        self.groups.iter().any(|(identity, node)| {
            identity.0 == collection_id.value() && node.display == Display::Flex
        })
    }
    fn expanded_node(&self) -> Option<Entity> {
        let collection_id = self.capture.group.as_ref()?;
        let preferences = self.capture.preferences.as_ref()?;
        if self.capture.stage != 2
            || preferences
                .snapshot()
                .collapsed_group_ids
                .contains(collection_id)
            || !self.visible(collection_id)
        {
            return None;
        }
        self.nodes
            .iter()
            .find(|(_, node)| node.collection_id.as_ref() == Some(collection_id))
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
    let Some(owner) = &surface.capture.preferences else {
        return;
    };
    let mut snapshot = surface.latest.0.clone();
    let preferences = owner.snapshot();
    if snapshot.outbound_ui_preferences.data.as_ref() != Some(&preferences) {
        snapshot.revision = snapshot.revision.saturating_add(1);
        snapshot.outbound_ui_preferences = PageData::ready(preferences);
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
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
                .is_none_or(|id| button.collection_id.as_ref() == Some(id))
        })
        .map(|(entity, button)| (entity, button.collection_id.clone()))
    else {
        return;
    };
    let Some(group) = group else { return };
    if surface.capture.stage == 1 && surface.visible(&group) {
        return;
    }
    surface.capture.group = Some(group);
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
