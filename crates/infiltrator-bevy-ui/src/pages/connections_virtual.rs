//! BANDROID-010: the Connections flat row list wired to the shared recycler
//! engine (the same [`VirtualListState`] / [`VirtualEntityPool`] the Proxies
//! page consumes, see [`super::proxies_virtual`]).
//!
//! The business collection is never truncated: [`ConnectionsProjection`] keeps
//! every connection and the recycler only bounds the mounted row roots. The
//! window is derived from the page scroll area's real offset and the sorted
//! projection order, so a 10,000-connection projection mounts the same bounded
//! slot set as a five-row one; scrolling slides the window instead of moving
//! every row.
//!
//! The large-list path is selected by [`CONNECTIONS_VIRTUAL_THRESHOLD`]; below
//! it the page keeps its full-mount vocabulary (and its stable-identity
//! reconcile contract). Search, grouping and sort stay shared reductions:
//! mounted rows still carry their projection-index markers, so in-place
//! restamps keep working while the recycler owns the row topology.

use crate::pages::connections::{
    ConnectionsProjection, ConnectionsScrollArea, LastConnectionsProjection,
};
use crate::pages::connections_row::connection_row_scene;
use crate::pages::connections_view::ConnectionsViewState;
use crate::pages::virtual_scroll::virtual_window_offset_px;
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::UiGlobalTransform;
use bevy::ui::prelude::{ComputedNode, FlexDirection, Node, Overflow, ScrollPosition, percent, px};
use infiltrator_bevy_widgets::list::VirtualListState;
use infiltrator_bevy_widgets::list::scroll_core::{SlotRebindAction, VirtualEntityPool};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_domain::connection_view;
use infiltrator_domain::connection_view::{ConnectionSortKey, ConnectionView};

/// Total connection count at or below which the page keeps its full-mount path.
/// Above it the recycler owns row mounting.
pub const CONNECTIONS_VIRTUAL_THRESHOLD: usize = 100;

/// Estimated height of one flat connection row (card padding + content).
pub const CONNECTION_ROW_HEIGHT_PX: f32 = 112.0;

/// Extra rows mounted above and below the viewport.
pub const CONNECTIONS_VIRTUAL_OVERSCAN: usize = 3;

/// Declared viewport height before layout reports a measured one.
pub const CONNECTIONS_VIRTUAL_FALLBACK_VIEWPORT_PX: f32 = 720.0;

/// Marker on the large-list virtual row container (the scroll content). The
/// container is also a [`ConnRowsContainer`], so the shared search/grouping
/// display reductions keep addressing it.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ConnectionsVirtualNodes;

/// Marker on a recycled row-slot root. The slot entity is stable across scroll;
/// only its children are rebuilt when it rebinds to a different row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsVirtualSlot {
    /// Slot index in the pool, `0..capacity`.
    pub slot_idx: usize,
}

/// Marker on a virtual spacer node. `top` distinguishes the leading spacer
/// (rows above the window) from the trailing one (rows below it).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsVirtualSpacer {
    /// Whether this is the leading spacer.
    pub top: bool,
}

/// Recycler state for the Connections flat row list.
#[derive(Resource)]
pub struct ConnectionsVirtualState {
    /// Window/scroll engine state over the sorted row stream.
    pub list: VirtualListState,
    /// Slot recycler over the spawned slot entities.
    pub pool: VirtualEntityPool,
    /// Slot entities in pool order.
    pub slots: Vec<Entity>,
    /// Leading spacer entity.
    pub top_spacer: Option<Entity>,
    /// Trailing spacer entity.
    pub bottom_spacer: Option<Entity>,
    /// Projection indices in the current sorted render order.
    pub order: Vec<usize>,
    /// Sort key `order` was derived from.
    pub sort: ConnectionSortKey,
    /// Signature of the mounted structure (sort key + render order).
    pub signature: u64,
    /// Current slot pool capacity.
    pub capacity: usize,
}

impl Default for ConnectionsVirtualState {
    fn default() -> Self {
        Self {
            list: VirtualListState::default(),
            pool: VirtualEntityPool::new(Vec::new()),
            slots: Vec::new(),
            top_spacer: None,
            bottom_spacer: None,
            order: Vec::new(),
            sort: ConnectionSortKey::default(),
            signature: 0,
            capacity: 0,
        }
    }
}

/// The shared sort reduction applied to projection indices. Mirrors
/// [`connection_view::sort_connections`] (same key ranking and stable
/// id tie-break) but returns the permutation so the recycler can window over
/// the original collection without cloning it.
pub fn sorted_connection_order(
    projection: &ConnectionsProjection,
    key: ConnectionSortKey,
) -> Vec<usize> {
    let rows = &projection.connections;
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&left, &right| {
        let a = &rows[left];
        let b = &rows[right];
        let ordering = match key {
            ConnectionSortKey::DownloadDesc => {
                b.view_download_total().cmp(&a.view_download_total())
            }
            ConnectionSortKey::UploadDesc => b.view_upload_total().cmp(&a.view_upload_total()),
            ConnectionSortKey::DownloadRateDesc => b
                .view_download_rate_bps()
                .total_cmp(&a.view_download_rate_bps()),
            ConnectionSortKey::UploadRateDesc => b
                .view_upload_rate_bps()
                .total_cmp(&a.view_upload_rate_bps()),
            ConnectionSortKey::LatestDesc => b.view_start().cmp(a.view_start()),
            ConnectionSortKey::HostAsc => {
                connection_view::connection_host(a).cmp(connection_view::connection_host(b))
            }
        };
        ordering.then_with(|| a.view_id().cmp(b.view_id()))
    });
    order
}

/// Stable signature of the sorted order so order/count changes trigger a
/// window rebuild. Text-only restamps do not rebuild; the page's in-place
/// observers refresh the mounted rows.
fn order_signature(order: &[usize], sort: ConnectionSortKey) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    sort.hash(&mut hasher);
    order.hash(&mut hasher);
    hasher.finish()
}

/// One recycled slot root: a fixed-height clipped column whose children are
/// rebuilt when the slot rebinds.
fn slot_scene(slot_idx: usize) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                height: px(0.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
            }
            ConnectionsVirtualSlot { slot_idx }
    }
}

/// One flat spacer node of exactly `height` logical pixels.
fn spacer_scene() -> impl Scene {
    bsn! {
            Node {
                width: percent(100),
                height: px(0.0),
                flex_shrink: 0.0,
            }
    }
}

/// Spawn the leading spacer, the slot pool and the trailing spacer in order.
fn spawn_slots(
    commands: &mut Commands<'_, '_>,
    state: &mut ConnectionsVirtualState,
    container: Entity,
    capacity: usize,
) {
    commands.entity(container).despawn_children();
    let top = commands
        .spawn_scene(spacer_scene())
        .insert((ConnectionsVirtualSpacer { top: true }, ChildOf(container)))
        .id();
    let mut slots = Vec::with_capacity(capacity);
    for slot_idx in 0..capacity {
        let entity = commands
            .spawn_scene(slot_scene(slot_idx))
            .insert(ChildOf(container))
            .id();
        slots.push(entity);
    }
    let bottom = commands
        .spawn_scene(spacer_scene())
        .insert((ConnectionsVirtualSpacer { top: false }, ChildOf(container)))
        .id();
    state.top_spacer = Some(top);
    state.bottom_spacer = Some(bottom);
    state.slots = slots;
    state.capacity = capacity;
    state.pool = VirtualEntityPool::new(state.slots.clone());
}

/// Scroll-area read shape: the scroll position plus the optional measured node
/// and global transform the window offset derives from.
type ConnectionsScrollQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ScrollPosition,
        Option<&'static ComputedNode>,
        Option<&'static UiGlobalTransform>,
    ),
    With<ConnectionsScrollArea>,
>;

/// Cohesive recycler surface for [`sync_connections_virtual_window`]: the
/// projection/state inputs plus the three mounted-tree queries, bundled so the
/// system keeps a small, typed parameter list (BEVY-ECS-009).
#[derive(SystemParam)]
pub(crate) struct ConnectionsVirtualSurface<'w, 's> {
    state: ResMut<'w, ConnectionsVirtualState>,
    last: Option<Res<'w, LastConnectionsProjection>>,
    view: Option<Res<'w, ConnectionsViewState>>,
    palette: Res<'w, UiPalette>,
    scrolls: ConnectionsScrollQuery<'w, 's>,
    rows_globals: Query<'w, 's, &'static UiGlobalTransform, With<ConnectionsVirtualNodes>>,
    containers: Query<'w, 's, Entity, With<ConnectionsVirtualNodes>>,
    slot_nodes: Query<
        'w,
        's,
        (
            &'static ConnectionsVirtualSlot,
            &'static mut Node,
            Option<&'static Children>,
        ),
        Without<ConnectionsVirtualSpacer>,
    >,
    spacers: Query<
        'w,
        's,
        (&'static ConnectionsVirtualSpacer, &'static mut Node),
        Without<ConnectionsVirtualSlot>,
    >,
    row_nodes: Query<'w, 's, &'static ComputedNode>,
}

/// Mount exactly the shared recycler window of the sorted connection list into
/// the virtual container. Inert unless the large-list container is mounted.
pub(crate) fn sync_connections_virtual_window(
    mut commands: Commands,
    surface: ConnectionsVirtualSurface,
) {
    let ConnectionsVirtualSurface {
        mut state,
        last,
        view,
        palette,
        scrolls,
        rows_globals,
        containers,
        mut slot_nodes,
        mut spacers,
        row_nodes,
    } = surface;
    let Ok(container) = containers.single() else {
        // The full-mount path owns the page; the recycler is not mounted.
        return;
    };
    let Some(projection) = last.as_ref().and_then(|last| last.0.clone()) else {
        return;
    };
    let sort = view.as_ref().map(|view| view.sort).unwrap_or_default();

    // Recompute the render permutation only when the projection or the shared
    // sort key actually changed, so a live rate tick keeps its window.
    let projection_changed = last.as_ref().is_some_and(|last| last.is_changed());
    if projection_changed || state.sort != sort || state.order.len() != projection.connections.len()
    {
        state.order = sorted_connection_order(&projection, sort);
        state.sort = sort;
    }

    let (offset, viewport) = scrolls.iter().next().map_or(
        (0.0, CONNECTIONS_VIRTUAL_FALLBACK_VIEWPORT_PX),
        |(position, computed, scroll_global)| {
            let height = computed
                .map(|node| node.size().y * node.inverse_scale_factor())
                .filter(|height| *height > 0.0)
                .unwrap_or(CONNECTIONS_VIRTUAL_FALLBACK_VIEWPORT_PX);
            let offset = virtual_window_offset_px(
                position,
                scroll_global,
                rows_globals.iter().next(),
                computed,
            );
            (offset, height)
        },
    );

    let signature = order_signature(&state.order, state.sort);
    let structure_changed = signature != state.signature;

    if structure_changed {
        state.signature = signature;
        state.list =
            VirtualListState::new_dynamic(state.order.len(), CONNECTION_ROW_HEIGHT_PX, viewport)
                .with_overscan(CONNECTIONS_VIRTUAL_OVERSCAN);
        for index in 0..state.order.len() {
            state
                .list
                .update_measured_row_height(index, CONNECTION_ROW_HEIGHT_PX);
        }
    } else {
        state.list.set_viewport_height(viewport);
    }
    state.list.set_scroll_offset(offset);

    let window = state.list.window();

    // Every row shares the estimated height, so the pool capacity is bounded by
    // the viewport, never by the collection.
    let capacity = ((viewport / CONNECTION_ROW_HEIGHT_PX).ceil() as usize)
        .saturating_add(CONNECTIONS_VIRTUAL_OVERSCAN * 2)
        .saturating_add(2)
        .max(8);
    if structure_changed || capacity != state.capacity || state.slots.is_empty() {
        spawn_slots(&mut commands, &mut state, container, capacity);
    }

    let actions = state.pool.sync_window(window.start, window.end, |_| 0.0);
    let rebind_happened = !actions.is_empty();
    for action in actions {
        match action {
            SlotRebindAction::Rebind {
                entity, item_idx, ..
            } => {
                commands.entity(entity).despawn_children();
                if let Some(source_index) = state.order.get(item_idx).copied()
                    && let Some(conn) = projection.connections.get(source_index)
                {
                    let scene = connection_row_scene(source_index, conn, &palette);
                    commands.spawn_scene(scene).insert(ChildOf(entity));
                }
            }
            SlotRebindAction::Deactivate { entity, .. } => {
                commands.entity(entity).despawn_children();
            }
        }
    }

    // Slot heights track their bound row so the mounted window and the scroll
    // extent agree. Inactive slots collapse to zero height.
    for (slot, mut node, children) in &mut slot_nodes {
        let active = slot.slot_idx < window.visible_count;
        let item_idx = window.start + slot.slot_idx;
        let height = if active {
            state
                .list
                .item_height(item_idx)
                .unwrap_or(CONNECTION_ROW_HEIGHT_PX)
        } else {
            0.0
        };
        let target = px(height);
        if node.height != target {
            node.height = target;
        }
        // Dynamic height: feed the mounted row's measured layout height back
        // into the index so multi-hop cards never overlap. Skip the frame that
        // rebound slots (the old child would measure against the new index).
        if active
            && !rebind_happened
            && let Some(child) = children.and_then(|children| children.iter().next())
            && let Ok(computed) = row_nodes.get(*child)
        {
            let measured = computed.size().y * computed.inverse_scale_factor();
            if measured > 0.0 {
                state.list.update_measured_row_height(item_idx, measured);
            }
        }
    }
    for (spacer, mut node) in &mut spacers {
        let target = px(if spacer.top {
            window.top_spacer_px
        } else {
            window.bottom_spacer_px
        });
        if node.height != target {
            node.height = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::world::World;
    use infiltrator_contract::connection::ConnectionStreamPhase;
    use infiltrator_contract::surface_snapshot::ConnectionSnapshot;

    fn connection(index: usize) -> ConnectionSnapshot {
        ConnectionSnapshot {
            start: format!("2026-01-01T00:00:{:02}Z", index % 60),
            id: format!("c-{index}"),
            destination_host: format!("host-{index}.example"),
            host: format!("host-{index}.example:443"),
            process: format!("proc-{index}"),
            rule: "DIRECT".to_owned(),
            rule_payload: String::new(),
            chain: "DIRECT".to_owned(),
            chains: vec!["DIRECT".to_owned()],
            network: "tcp".to_owned(),
            source_ip: "192.168.1.5".to_owned(),
            source_port: "50000".to_owned(),
            destination_ip: "1.1.1.1".to_owned(),
            destination_port: "443".to_owned(),
            rate_observed: true,
            upload_bps: index as f64,
            download_bps: (index * 2) as f64,
            upload_total: index as u64,
            download_total: (index * 10) as u64,
            destination_geo_ip: None,
            destination_ip_asn: String::new(),
        }
    }

    fn projection_with(count: usize) -> ConnectionsProjection {
        ConnectionsProjection {
            total_connections: count,
            total_upload_bytes: 0,
            total_download_bytes: 0,
            stream_phase: ConnectionStreamPhase::Live,
            connections: (0..count).map(connection).collect(),
        }
    }

    #[test]
    fn sorted_order_mirrors_the_shared_sort_reduction() {
        let projection = projection_with(5);
        // Default (download-desc) ranks c-4 first, c-0 last.
        let order = sorted_connection_order(&projection, ConnectionSortKey::DownloadDesc);
        assert_eq!(order, vec![4, 3, 2, 1, 0]);
        // Host-ascending is the identity order for `host-{i}` names.
        let host_order = sorted_connection_order(&projection, ConnectionSortKey::HostAsc);
        assert_eq!(host_order, vec![0, 1, 2, 3, 4]);
        // Instantaneous upload rate keeps the same monotone ranking.
        let rate_order = sorted_connection_order(&projection, ConnectionSortKey::UploadRateDesc);
        assert_eq!(rate_order, vec![4, 3, 2, 1, 0]);
    }

    #[test]
    fn large_connection_list_mounts_a_bounded_window_not_the_full_list() {
        let projection = projection_with(10_000);
        let order = sorted_connection_order(&projection, ConnectionSortKey::DownloadDesc);
        assert_eq!(order.len(), 10_000);

        let viewport = CONNECTIONS_VIRTUAL_FALLBACK_VIEWPORT_PX;
        let mut list =
            VirtualListState::new_dynamic(order.len(), CONNECTION_ROW_HEIGHT_PX, viewport)
                .with_overscan(CONNECTIONS_VIRTUAL_OVERSCAN);
        for index in 0..order.len() {
            list.update_measured_row_height(index, CONNECTION_ROW_HEIGHT_PX);
        }
        let bound = (viewport / CONNECTION_ROW_HEIGHT_PX).ceil() as usize
            + CONNECTIONS_VIRTUAL_OVERSCAN * 2;
        let window = list.window();
        assert!(
            window.visible_count <= bound,
            "mounted window {} must be bounded by {bound}",
            window.visible_count
        );
        assert!(
            window.visible_count < order.len(),
            "a 10k-connection list must not mount every row"
        );
    }

    #[test]
    fn scrolling_slides_the_window_and_the_tail_is_reachable() {
        let projection = projection_with(10_000);
        let order = sorted_connection_order(&projection, ConnectionSortKey::DownloadDesc);
        let viewport = CONNECTIONS_VIRTUAL_FALLBACK_VIEWPORT_PX;
        let mut list =
            VirtualListState::new_dynamic(order.len(), CONNECTION_ROW_HEIGHT_PX, viewport)
                .with_overscan(CONNECTIONS_VIRTUAL_OVERSCAN);
        for index in 0..order.len() {
            list.update_measured_row_height(index, CONNECTION_ROW_HEIGHT_PX);
        }

        let top = list.window();
        assert_eq!(top.start, 0);
        assert!(top.bottom_spacer_px > 0.0);

        list.scroll_to_bottom();
        let bottom = list.window();
        assert_eq!(bottom.end, order.len());
        assert!(bottom.top_spacer_px > 0.0);
        assert!(bottom.bottom_spacer_px <= f32::EPSILON);
        assert!(
            bottom.start > top.start,
            "the window must slide as the offset advances"
        );
        // The last sorted row is the smallest cumulative download (index 0).
        assert_eq!(order[bottom.end - 1], 0);
    }

    #[test]
    fn rebinding_preserves_stable_slot_identity() {
        // The engine's pool recycles pre-spawned slots; a rebind never replaces
        // the slot entity itself, only its children.
        let world = World::new();
        // Allocate bare entity ids without spawning a UI tree: the recycler
        // only needs stable identities, never queryable entities.
        let slots: Vec<Entity> = (0..16).map(|_| world.entity_allocator().alloc()).collect();
        let mut pool = VirtualEntityPool::new(slots.clone());

        let first = pool.sync_window(0, 10, |_| 0.0);
        assert_eq!(pool.active_count(), 10);
        assert_eq!(first.len(), 10);

        let second = pool.sync_window(5, 15, |_| 0.0);
        for action in second {
            if let SlotRebindAction::Rebind { entity, .. } = action {
                assert!(
                    slots.contains(&entity),
                    "rebound slot {entity:?} must be one of the stable pool entities"
                );
            }
        }
        assert_eq!(pool.active_count(), 10);
    }
}
