//! BANDROID-009: the Proxies node list wired to the shared recycler engine.
//!
//! The engine in `infiltrator_bevy_widgets::list` owns the window math
//! ([`VirtualListState`], dynamic row heights, overscan) and the slot recycler
//! ([`VirtualEntityPool`]). This module maps the shared Proxies projection onto
//! a flat row stream (group header rows + wrapped node grid rows), derives the
//! mounted window from the viewport's real scroll offset, and mounts only the
//! window plus top/bottom spacer nodes.
//!
//! The business collection is never truncated: the projection stays whole and
//! only the mounted row roots are bounded. The large-list path is selected by
//! [`PROXIES_VIRTUAL_THRESHOLD`]; below it the page keeps its full-mount
//! vocabulary (and its stable-identity reconcile contract).

use crate::pages::proxies::{LastProxiesProjection, ProxiesProjection, ProxiesScrollArea};
use crate::pages::proxies_card::{group_header_scene, proxy_node_scene_sized};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    ComputedNode, FlexDirection, FlexWrap, Node, Overflow, ScrollPosition, Val, percent, px,
};
use infiltrator_bevy_widgets::fluid_grid::FluidCardGrid;
use infiltrator_bevy_widgets::list::VirtualListState;
use infiltrator_bevy_widgets::list::scroll_core::{SlotRebindAction, VirtualEntityPool};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::theme::{Breakpoint, space};

/// Total visible node count at or below which the page keeps its full-mount
/// path. Above it the recycler owns node mounting.
pub const PROXIES_VIRTUAL_THRESHOLD: usize = 100;

/// Estimated height of one group header row.
pub const PROXY_GROUP_ROW_HEIGHT_PX: f32 = 64.0;

/// Estimated height of one wrapped node grid row (card min-height + row gap).
pub const PROXY_NODE_ROW_HEIGHT_PX: f32 = 66.0;

/// Extra grid rows mounted above and below the viewport.
pub const PROXIES_VIRTUAL_OVERSCAN: usize = 2;

/// Declared viewport height before layout reports a measured one.
pub const PROXIES_VIRTUAL_FALLBACK_VIEWPORT_PX: f32 = 720.0;

/// One virtual row: either a group header card or a wrapped grid row of nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxiesVirtualRow {
    /// A single group header card.
    Group { group_idx: usize },
    /// A grid row holding up to `columns` nodes of one group.
    Nodes {
        group_idx: usize,
        node_indices: Vec<usize>,
    },
}

impl ProxiesVirtualRow {
    /// Estimated row height in logical pixels.
    pub fn height_px(&self) -> f32 {
        match self {
            Self::Group { .. } => PROXY_GROUP_ROW_HEIGHT_PX,
            Self::Nodes { .. } => PROXY_NODE_ROW_HEIGHT_PX,
        }
    }
}

/// The shared tier column operator for the proxies node grid.
pub fn proxies_virtual_columns(compact_view: bool, context: Option<&ResponsiveContext>) -> usize {
    if compact_view {
        return 1;
    }
    match context
        .map(|c| c.breakpoint)
        .unwrap_or(Breakpoint::Expanded)
    {
        Breakpoint::Compact => 1,
        Breakpoint::Medium => 2,
        Breakpoint::Expanded => 3,
        Breakpoint::Ultra => 4,
    }
}

/// Flatten the whole projection into visible rows: one group header per group
/// followed by its wrapped node grid rows when expanded. Search/sort are already
/// baked into the projection order; collapse is honoured by the `expanded` flag.
pub fn flatten_proxy_rows(
    projection: &ProxiesProjection,
    columns: usize,
) -> Vec<ProxiesVirtualRow> {
    let columns = columns.max(1);
    let mut rows = Vec::new();
    for (group_idx, group) in projection.groups.iter().enumerate() {
        rows.push(ProxiesVirtualRow::Group { group_idx });
        if !group.expanded || group.proxies.is_empty() {
            continue;
        }
        let mut start = 0;
        while start < group.proxies.len() {
            let end = (start + columns).min(group.proxies.len());
            rows.push(ProxiesVirtualRow::Nodes {
                group_idx,
                node_indices: (start..end).collect(),
            });
            start = end;
        }
    }
    rows
}

/// Stable signature of the visible projection so structure changes (order,
/// names, collapse, column count) trigger a window rebuild. Selection, favorite
/// and latency changes do not rebuild; the page restamps those in place.
fn row_signature(projection: &ProxiesProjection, columns: usize) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    columns.hash(&mut hasher);
    for group in &projection.groups {
        group.name.hash(&mut hasher);
        group.expanded.hash(&mut hasher);
        for node in &group.proxies {
            node.name.hash(&mut hasher);
        }
    }
    hasher.finish()
}

/// Marker on the large-list virtual node container (the scroll content).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxiesVirtualNodes;

/// Marker on a recycled row-slot root. The slot entity is stable across scroll;
/// only its children are rebuilt when it rebinds to a different row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxiesVirtualSlot {
    /// Slot index in the pool, `0..capacity`.
    pub slot_idx: usize,
}

/// Marker on a virtual spacer node. `top` distinguishes the leading spacer
/// (rows above the window) from the trailing one (rows below it).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxiesVirtualSpacer {
    /// Whether this is the leading spacer.
    pub top: bool,
}

/// Recycler state for the Proxies node list.
#[derive(Resource)]
pub struct ProxiesVirtualState {
    /// Window/scroll engine state over the flattened row stream.
    pub list: VirtualListState,
    /// Slot recycler over the spawned slot entities.
    pub pool: VirtualEntityPool,
    /// Slot entities in pool order.
    pub slots: Vec<Entity>,
    /// Leading spacer entity.
    pub top_spacer: Option<Entity>,
    /// Trailing spacer entity.
    pub bottom_spacer: Option<Entity>,
    /// Current flattened rows.
    pub rows: Vec<ProxiesVirtualRow>,
    /// Current grid column count.
    pub columns: usize,
    /// Signature of the mounted structure.
    pub signature: u64,
    /// Current slot pool capacity.
    pub capacity: usize,
}

impl Default for ProxiesVirtualState {
    fn default() -> Self {
        Self {
            list: VirtualListState::default(),
            pool: VirtualEntityPool::new(Vec::new()),
            slots: Vec::new(),
            top_spacer: None,
            bottom_spacer: None,
            rows: Vec::new(),
            columns: 3,
            signature: 0,
            capacity: 0,
        }
    }
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
            ProxiesVirtualSlot { slot_idx }
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

/// Build the scene for one flattened row.
fn row_scene(
    row: &ProxiesVirtualRow,
    projection: &ProxiesProjection,
    palette: &UiPalette,
    columns: usize,
) -> Box<dyn Scene> {
    match row {
        ProxiesVirtualRow::Group { group_idx } => {
            let Some(group) = projection.groups.get(*group_idx) else {
                return Box::new(bsn! { Node { width: percent(100) } });
            };
            Box::new(group_header_scene(*group_idx, group, palette))
        }
        ProxiesVirtualRow::Nodes {
            group_idx,
            node_indices,
        } => {
            let Some(group) = projection.groups.get(*group_idx) else {
                return Box::new(bsn! { Node { width: percent(100) } });
            };
            let width = Val::Percent(FluidCardGrid::wrapped_item_percent(columns));
            let cards: Vec<Box<dyn Scene>> = node_indices
                .iter()
                .filter_map(|node_idx| {
                    group.proxies.get(*node_idx).map(|node| {
                        Box::new(proxy_node_scene_sized(
                            *group_idx,
                            *node_idx,
                            &group.name,
                            node,
                            width,
                            palette,
                        )) as Box<dyn Scene>
                    })
                })
                .collect();
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: px(space::S8),
                        row_gap: px(space::S8),
                    }
                    Children [ { cards } ]
            })
        }
    }
}

/// Spawn the leading spacer, the slot pool and the trailing spacer in order.
fn spawn_slots(
    commands: &mut Commands<'_, '_>,
    state: &mut ProxiesVirtualState,
    container: Entity,
    capacity: usize,
) {
    commands.entity(container).despawn_children();
    let top = commands
        .spawn_scene(spacer_scene())
        .insert((ProxiesVirtualSpacer { top: true }, ChildOf(container)))
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
        .insert((ProxiesVirtualSpacer { top: false }, ChildOf(container)))
        .id();
    state.top_spacer = Some(top);
    state.bottom_spacer = Some(bottom);
    state.slots = slots;
    state.capacity = capacity;
    state.pool = VirtualEntityPool::new(state.slots.clone());
}

/// Cohesive recycler surface for [`sync_proxies_virtual_window`]: the
/// projection inputs plus the mounted-tree queries, bundled so the system keeps
/// a small, typed parameter list (BEVY-ECS-009).
#[derive(SystemParam)]
pub(crate) struct ProxiesVirtualSurface<'w, 's> {
    state: ResMut<'w, ProxiesVirtualState>,
    last: Option<Res<'w, LastProxiesProjection>>,
    palette: Res<'w, UiPalette>,
    responsive: Option<Res<'w, ResponsiveContext>>,
    scrolls: Query<
        'w,
        's,
        (&'static ScrollPosition, Option<&'static ComputedNode>),
        With<ProxiesScrollArea>,
    >,
    containers: Query<'w, 's, Entity, With<ProxiesVirtualNodes>>,
    slot_nodes: Query<
        'w,
        's,
        (&'static ProxiesVirtualSlot, &'static mut Node),
        Without<ProxiesVirtualSpacer>,
    >,
    spacers: Query<
        'w,
        's,
        (&'static ProxiesVirtualSpacer, &'static mut Node),
        Without<ProxiesVirtualSlot>,
    >,
}

/// Mount exactly the shared recycler window of the flattened node list into the
/// virtual container. Inert unless the large-list container is mounted.
pub(crate) fn sync_proxies_virtual_window(mut commands: Commands, surface: ProxiesVirtualSurface) {
    let ProxiesVirtualSurface {
        mut state,
        last,
        palette,
        responsive,
        scrolls,
        containers,
        mut slot_nodes,
        mut spacers,
    } = surface;
    let Ok(container) = containers.single() else {
        // The full-mount path owns the page; the recycler is not mounted.
        return;
    };
    let Some(projection) = last.as_ref().and_then(|last| last.0.clone()) else {
        return;
    };

    let (offset, viewport) = scrolls.iter().next().map_or(
        (0.0, PROXIES_VIRTUAL_FALLBACK_VIEWPORT_PX),
        |(position, computed)| {
            let height = computed
                .map(|node| node.size().y * node.inverse_scale_factor())
                .filter(|height| *height > 0.0)
                .unwrap_or(PROXIES_VIRTUAL_FALLBACK_VIEWPORT_PX);
            (position.0.y.max(0.0), height)
        },
    );

    let columns = proxies_virtual_columns(projection.compact_view, responsive.as_deref());
    let rows = flatten_proxy_rows(&projection, columns);
    let signature = row_signature(&projection, columns);
    let structure_changed = signature != state.signature;

    if structure_changed {
        state.rows = rows;
        state.columns = columns;
        state.signature = signature;
        state.list =
            VirtualListState::new_dynamic(state.rows.len(), PROXY_NODE_ROW_HEIGHT_PX, viewport)
                .with_overscan(PROXIES_VIRTUAL_OVERSCAN);
        for index in 0..state.rows.len() {
            let height = state.rows[index].height_px();
            state.list.update_measured_row_height(index, height);
        }
    } else {
        state.list.set_viewport_height(viewport);
    }
    state.list.set_scroll_offset(offset);

    let window = state.list.window();

    // The shortest row is a group header; that bounds how many rows can ever be
    // visible, so the pool capacity is viewport-bounded, never list-bounded.
    let capacity = ((viewport / PROXY_GROUP_ROW_HEIGHT_PX).ceil() as usize)
        .saturating_add(PROXIES_VIRTUAL_OVERSCAN * 2)
        .saturating_add(2)
        .max(8);
    if structure_changed || capacity != state.capacity || state.slots.is_empty() {
        spawn_slots(&mut commands, &mut state, container, capacity);
    }

    let actions = state.pool.sync_window(window.start, window.end, |_| 0.0);
    for action in actions {
        match action {
            SlotRebindAction::Rebind {
                entity, item_idx, ..
            } => {
                commands.entity(entity).despawn_children();
                if let Some(row) = state.rows.get(item_idx) {
                    let scene = row_scene(row, &projection, &palette, state.columns);
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
    for (slot, mut node) in &mut slot_nodes {
        let height = if slot.slot_idx < window.visible_count {
            state
                .rows
                .get(window.start + slot.slot_idx)
                .map(ProxiesVirtualRow::height_px)
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let target = px(height);
        if node.height != target {
            node.height = target;
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
    use crate::pages::proxies::{ProxyGroup, ProxyNode};
    use bevy::ecs::world::World;
    use infiltrator_contract::proxies::ProxyGroupClassification;

    fn projection_with(count: usize) -> ProxiesProjection {
        ProxiesProjection {
            name_runs: Default::default(),
            search_query: String::new(),
            testing: false,
            filter_alive: false,
            compact_view: false,
            active_exit: "node-0".to_owned(),
            custom_node: Default::default(),
            groups: vec![ProxyGroup {
                name: "BIG".to_owned(),
                group_type: "Selector".to_owned(),
                classification: ProxyGroupClassification::Selector,
                current: "node-0".to_owned(),
                expanded: true,
                proxies: (0..count)
                    .map(|index| ProxyNode {
                        name: format!("node-{index}"),
                        node_type: "Shadowsocks".to_owned(),
                        delay_ms: Some(40),
                        selected: index == 0,
                        favorite: false,
                        features: vec![],
                    })
                    .collect(),
            }],
        }
    }

    #[test]
    fn large_node_list_mounts_a_bounded_window_not_the_full_list() {
        let projection = projection_with(2000);
        let columns = 3;
        let rows = flatten_proxy_rows(&projection, columns);
        // 1 group header + ceil(2000 / 3) node rows.
        assert_eq!(rows.len(), 1 + 2000_usize.div_ceil(columns));

        let viewport = PROXIES_VIRTUAL_FALLBACK_VIEWPORT_PX;
        let mut list =
            VirtualListState::new_dynamic(rows.len(), PROXY_NODE_ROW_HEIGHT_PX, viewport)
                .with_overscan(PROXIES_VIRTUAL_OVERSCAN);
        for (index, row) in rows.iter().enumerate() {
            list.update_measured_row_height(index, row.height_px());
        }
        let bound =
            (viewport / PROXY_GROUP_ROW_HEIGHT_PX).ceil() as usize + PROXIES_VIRTUAL_OVERSCAN * 2;
        let window = list.window();
        assert!(
            window.visible_count <= bound,
            "mounted window {} must be bounded by {bound}",
            window.visible_count
        );
        assert!(
            window.visible_count < rows.len(),
            "a 2000-node list must not mount every row"
        );
    }

    #[test]
    fn scrolling_slides_the_window_and_the_tail_is_reachable() {
        let projection = projection_with(2000);
        let rows = flatten_proxy_rows(&projection, 3);
        let viewport = PROXIES_VIRTUAL_FALLBACK_VIEWPORT_PX;
        let mut list =
            VirtualListState::new_dynamic(rows.len(), PROXY_NODE_ROW_HEIGHT_PX, viewport)
                .with_overscan(PROXIES_VIRTUAL_OVERSCAN);
        for (index, row) in rows.iter().enumerate() {
            list.update_measured_row_height(index, row.height_px());
        }

        let top = list.window();
        assert_eq!(top.start, 0);
        assert!(top.bottom_spacer_px > 0.0);

        list.scroll_to_bottom();
        let bottom = list.window();
        assert_eq!(bottom.end, rows.len());
        assert!(bottom.top_spacer_px > 0.0);
        assert!(bottom.bottom_spacer_px <= f32::EPSILON);
        assert!(
            bottom.start > top.start,
            "the window must slide as the offset advances"
        );
    }

    #[test]
    fn rebinding_preserves_stable_slot_identity() {
        // The engine's pool recycles pre-spawned slots; a rebind never replaces
        // the slot entity itself, only its children. Stale queued clicks on the
        // removed child cannot fire because the child is despawned.
        let world = World::new();
        // Allocate bare entity ids without spawning a UI tree: the recycler
        // only needs stable identities, never queryable entities.
        let slots: Vec<Entity> = (0..16).map(|_| world.entity_allocator().alloc()).collect();
        let mut pool = VirtualEntityPool::new(slots.clone());

        let first = pool.sync_window(0, 10, |_| 0.0);
        assert_eq!(pool.active_count(), 10);
        let rebound_entities: Vec<Entity> = first
            .iter()
            .filter_map(|action| match action {
                SlotRebindAction::Rebind { entity, .. } => Some(*entity),
                SlotRebindAction::Deactivate { .. } => None,
            })
            .collect();
        assert_eq!(rebound_entities.len(), 10);

        // Slide the window: the same slot entities are reused, never respawned.
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
