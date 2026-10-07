//! Route mounting consumes only the accepted render cache and the state needed for one subtree.
use super::{
    ActiveRoute, RouteChanged, RouteHistory, SurfaceSourceHandle, page_scene,
    trigger_page_projection_events,
};
use crate::app::ContentSlot;
use crate::history::TrafficHistory;
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::CommandsSceneExt;
use infiltrator_bevy_widgets::palette::UiPalette;

#[derive(SystemParam)]
pub(crate) struct RouteMount<'w, 's> {
    slots: Query<'w, 's, Entity, With<ContentSlot>>,
    active: Option<Res<'w, ActiveRoute>>,
    history: Option<ResMut<'w, RouteHistory>>,
    palette: Res<'w, UiPalette>,
    source: Res<'w, SurfaceSourceHandle>,
    history_ring: Res<'w, TrafficHistory>,
    latest: Option<Res<'w, LatestSurfaceSnapshot>>,
}

pub(crate) fn sync_route(trigger: On<RouteChanged>, mut mount: RouteMount, mut commands: Commands) {
    let route = trigger.0;
    if mount.active.is_some_and(|mounted| mounted.0 == Some(route)) {
        return;
    }
    let Ok(slot) = mount.slots.single() else {
        return;
    };
    if let Some(ref mut history) = mount.history {
        history.push(route);
    }
    let snapshot = mount.latest.as_ref().map_or_else(
        || mount.source.0.surface_snapshot(),
        |latest| latest.0.clone(),
    );
    let scene = page_scene(route, &snapshot, &mount.history_ring, &mount.palette);
    commands.entity(slot).despawn_children();
    commands.spawn_scene(scene).insert(ChildOf(slot));
    // Spawn is committed before plugin-installed projection observers run.
    trigger_page_projection_events(&snapshot, &mut commands);
    commands.insert_resource(ActiveRoute(Some(route)));
}
