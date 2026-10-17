//! Open and move a full shared fixture through actual native order observers.
use super::geometry::CaptureGeometry;
use super::host::{CaptureCapability, CaptureCommandSink};
use super::outbound_fixture::{FIRST_GROUP, LAST_GROUP, MOVED_GROUP, set_capture_inventory};
use super::outbound_preferences::OutboundPreferenceCaptureOwner;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::command::CommandSinkHandle;
use crate::pages::proxies::ProxyGroupMoveUpButton;
use crate::pages::proxy_group_order::{
    GroupOrderAction, GroupOrderCard, GroupOrderDialogState, GroupOrderList, GroupOrderRow,
    GroupOrderState,
};
use crate::route::{ActiveRoute, Route};
use crate::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface_snapshot::PageData;
use lurker_core_contract::outbound::inventory::SelectionAvailability;
use std::sync::Arc;

#[derive(Resource)]
pub(super) struct OrderCapture {
    injected: bool,
    preferences: Option<OutboundPreferenceCaptureOwner>,
}
pub fn install(app: &mut App) {
    app.insert_resource(OrderCapture {
        injected: false,
        preferences: None,
    });
}
pub(super) fn initialize(
    latest: Res<LatestSurfaceSnapshot>,
    mut capture: ResMut<OrderCapture>,
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
            CaptureCapability::GroupOrder,
            owner.clone(),
        ),
    )));
    capture.preferences = Some(owner);
}
#[derive(SystemParam)]
pub struct OrderSurface<'w, 's> {
    route: Res<'w, ActiveRoute>,
    capture: ResMut<'w, OrderCapture>,
    latest: Res<'w, LatestSurfaceSnapshot>,
    editor: Res<'w, GroupOrderState>,
    moves: Query<'w, 's, (Entity, &'static ProxyGroupMoveUpButton)>,
    lists: Query<'w, 's, (Entity, &'static Children), With<GroupOrderList>>,
    rows: Query<'w, 's, &'static GroupOrderRow>,
    cards: Query<'w, 's, Entity, With<GroupOrderCard>>,
    actions: Query<'w, 's, (Entity, &'static GroupOrderAction, &'static ButtonDisabled)>,
}
pub fn activate(mut surface: OrderSurface, state: Res<InteractionCapture>, mut commands: Commands) {
    if !selected(
        &state,
        &surface.route,
        FeatureId::ProxiesGroupReorder,
        Route::Proxies,
    ) {
        return;
    }
    if !surface.capture.injected {
        let Some(preferences) = &surface.capture.preferences else {
            return;
        };
        let mut snapshot = surface.latest.0.clone();
        snapshot.revision += 1;
        set_capture_inventory(
            &mut snapshot,
            &[
                (
                    FIRST_GROUP,
                    &["Inspection node"],
                    SelectionAvailability::Selectable,
                ),
                (
                    MOVED_GROUP,
                    &["Inspection node"],
                    SelectionAvailability::Selectable,
                ),
                (
                    LAST_GROUP,
                    &["Inspection node"],
                    SelectionAvailability::Selectable,
                ),
            ],
        );
        snapshot.outbound_ui_preferences = PageData::ready(preferences.snapshot());
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
        surface.capture.injected = true;
        return;
    }
    if surface.editor.dialog != GroupOrderDialogState::Editing
        && let Some((entity, _)) = surface
            .moves
            .iter()
            .find(|(_, button)| button.group_name == MOVED_GROUP)
    {
        commands.trigger(Activate { entity });
    }
}
impl OrderSurface<'_, '_> {
    fn bounds(&self, geometry: &mut CaptureGeometry) -> Option<[f32; 4]> {
        let state = &self.editor;
        if !self.capture.injected
            || state.dialog != GroupOrderDialogState::Editing
            || !state.editor.can_apply()
            || !state.editor.draft().iter().map(|id| id.value()).eq([
                MOVED_GROUP,
                FIRST_GROUP,
                LAST_GROUP,
            ])
        {
            return None;
        }
        let (list_entity, children) = self.lists.iter().next()?;
        if children.len() != 3
            || !children
                .iter()
                .zip(state.editor.draft())
                .all(|(entity, id)| self.rows.get(*entity).is_ok_and(|row| &row.id == id))
        {
            return None;
        }
        let card = geometry.bounds(self.cards.iter().next()?, "group order")?;
        let list = geometry.bounds(list_entity, "group order list")?;
        if !inside(card, list) {
            return None;
        }
        for entity in children.iter() {
            if !inside(card, geometry.bounds(*entity, "group-order-row")?) {
                return None;
            }
        }
        let (apply, _, _) = self.actions.iter().find(|(_, action, disabled)| {
            matches!(action, GroupOrderAction::Apply) && !disabled.0
        })?;
        if !inside(card, geometry.bounds(apply, "group-order-apply")?) {
            return None;
        }
        Some(card)
    }
}
fn inside(card: [f32; 4], region: [f32; 4]) -> bool {
    region[0] >= card[0]
        && region[1] >= card[1]
        && region[0] + region[2] <= card[0] + card[2]
        && region[1] + region[3] <= card[1] + card[3]
}
pub fn observe(
    surface: OrderSurface,
    mut geometry: CaptureGeometry,
    mut state: ResMut<InteractionCapture>,
    mut observation: ResMut<ObservedInteraction>,
) {
    observation.publish(&mut state, surface.bounds(&mut geometry));
}
