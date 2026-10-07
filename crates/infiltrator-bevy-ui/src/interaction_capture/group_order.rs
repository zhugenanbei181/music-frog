//! Open and move a full shared fixture through actual native order observers.
use super::geometry::CaptureGeometry;
use super::{InteractionCapture, ObservedInteraction, selected};
use crate::pages::proxies::ProxyGroupMoveUpButton;
use crate::pages::proxy_group_order::{
    GroupOrderAction, GroupOrderCard, GroupOrderList, GroupOrderRow, GroupOrderState,
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
use infiltrator_application::proxy_group_order_fixtures::{
    FIRST_GROUP, LAST_GROUP, MOVED_GROUP, observed_groups,
};
use infiltrator_application::proxy_projection::project_groups_snapshot;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_contract::parity::FeatureId;

#[derive(Resource, Default)]
struct OrderCapture {
    injected: bool,
}
pub fn install(app: &mut App) {
    app.init_resource::<OrderCapture>();
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
        let mut snapshot = surface.latest.0.clone();
        snapshot.revision += 1;
        let page = snapshot
            .pages
            .proxies
            .data
            .as_mut()
            .expect("order fixture page");
        let (groups, filter_alive) =
            project_groups_snapshot(&observed_groups(), &Default::default());
        page.groups = groups;
        page.filter_alive = filter_alive;
        commands.trigger(SurfaceSnapshotUpdated(snapshot));
        surface.capture.injected = true;
        return;
    }
    if !surface.editor.open
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
            || !state.open
            || !state.editor.can_apply()
            || state.editor.draft != [MOVED_GROUP, FIRST_GROUP, LAST_GROUP]
        {
            return None;
        }
        let (list_entity, children) = self.lists.iter().next()?;
        if children.len() != 3
            || !children
                .iter()
                .zip(&state.editor.draft)
                .all(|(entity, name)| self.rows.get(*entity).is_ok_and(|row| &row.0 == name))
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
