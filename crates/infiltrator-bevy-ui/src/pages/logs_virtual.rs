//! BANDROID-010: the Logs row list wired to the shared recycler engine (the
//! same [`VirtualListState`] / shared recycle rule the Proxies and
//! Connections pages consume).
//!
//! The mounted row topology is always bounded: the shared recycler bounds the
//! visible window of the page's in-memory ring ([`crate::pages::logs_ring`],
//! BEVY-020), and the projection the page reads is that same bounded window.
//! Log rows wrap, so the shared `zhuge-ui-core::virtual_list::VariableExtentIndex`
//! receives each mounted row's measured extent and keeps the scroll geometry honest.
//!
//! The large-list path is selected by [`LOGS_VIRTUAL_THRESHOLD`]; below it the
//! page keeps its full-mount vocabulary. Stable [`LogRowIdentity`] markers,
//! search/highlight restamps and the shared scroll-lock/follow policy all keep
//! working because only the row topology is bounded, never the projection.

use crate::pages::logs::{LastLogsProjection, LogEntry, LogsPageRoot};
use crate::pages::logs_rows::log_row_scene;
use crate::pages::virtual_scroll::virtual_window_offset_px;
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
use infiltrator_bevy_widgets::palette::UiPalette;
use zhuge_ui_core::virtual_list::max_band_rows;
use zhuge_ui_kit_bevy::control::{SlotAction, VirtualScrollSlot, sync_virtual_slots};

/// Total entry count at or below which the page keeps its full-mount path.
/// Above it the recycler owns row mounting.
pub const LOGS_VIRTUAL_THRESHOLD: usize = 200;

/// Estimated height of one log row before layout reports a measured one.
///
/// The console row is a single mono line with a hairline separator, so the
/// estimate is one text line tall; the variable-extent index replaces it with
/// the measured height as soon as layout reports one.
pub const LOG_ROW_HEIGHT_PX: f32 = 30.0;

/// Extra rows mounted above and below the viewport.
pub const LOGS_VIRTUAL_OVERSCAN: usize = 4;

/// Declared viewport height before layout reports a measured one.
pub const LOGS_VIRTUAL_FALLBACK_VIEWPORT_PX: f32 = 720.0;

/// Marker on the large-list virtual row container (the scroll content). The
/// container also keeps its `LogRowsContainer` marker, so the shared search
/// display reductions keep addressing it.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LogsVirtualNodes;

/// Marker on a recycled row-slot root. The slot entity is stable across scroll;
/// only its children are rebuilt when it rebinds to a different row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogsVirtualSlot {
    /// Slot index in the pool, `0..capacity`.
    pub slot_idx: usize,
}

/// Marker on a virtual spacer node. `top` distinguishes the leading spacer
/// (rows above the window) from the trailing one (rows below it).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogsVirtualSpacer {
    /// Whether this is the leading spacer.
    pub top: bool,
}

/// Recycler state for the Logs row list.
#[derive(Resource, Default)]
pub struct LogsVirtualState {
    /// Window/scroll engine state over the log entry stream.
    pub list: VirtualListState,
    /// Slot recycler over the spawned slot entities.
    /// The recycled slot states, in pool order. The recycle rule itself is
    /// owned by `zhuge-ui-kit-bevy::control::sync_virtual_slots`; this array is
    /// the per-slot truth that rule advances.
    pub slot_states: Vec<VirtualScrollSlot>,
    /// Slot entities in pool order.
    pub slots: Vec<Entity>,
    /// Leading spacer entity.
    pub top_spacer: Option<Entity>,
    /// Trailing spacer entity.
    pub bottom_spacer: Option<Entity>,
    /// Signature of the mounted structure (entry count + identities).
    pub signature: u64,
    /// Current slot pool capacity.
    pub capacity: usize,
}

/// Stable signature of the entry stream so growth/shrink/reorder trigger a
/// window rebuild. Text-only restamps do not rebuild; the page's in-place
/// observers refresh the mounted rows.
fn entry_signature(entries: &[LogEntry]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    entries.len().hash(&mut hasher);
    for entry in entries {
        entry.id.hash(&mut hasher);
    }
    hasher.finish()
}

/// One recycled slot root: a clipped column whose children are rebuilt when the
/// slot rebinds. The child row keeps its natural height (`flex_shrink: 0`) so
/// the shared variable-extent index can measure wrapped messages.
fn slot_scene(slot_idx: usize) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                height: px(0.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
            }
            LogsVirtualSlot { slot_idx }
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
    state: &mut LogsVirtualState,
    container: Entity,
    capacity: usize,
) {
    commands.entity(container).despawn_children();
    let top = commands
        .spawn_scene(spacer_scene())
        .insert((LogsVirtualSpacer { top: true }, ChildOf(container)))
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
        .insert((LogsVirtualSpacer { top: false }, ChildOf(container)))
        .id();
    state.top_spacer = Some(top);
    state.bottom_spacer = Some(bottom);
    state.slots = slots;
    state.capacity = capacity;
    state.slot_states = (0..capacity)
        .map(|slot| VirtualScrollSlot {
            slot,
            ..VirtualScrollSlot::default()
        })
        .collect();
}

/// Scroll-area read shape: the scroll position plus the optional measured node
/// and global transform the window offset derives from.
type LogsScrollQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ScrollPosition,
        Option<&'static ComputedNode>,
        Option<&'static UiGlobalTransform>,
    ),
    With<LogsPageRoot>,
>;

/// Cohesive recycler surface for [`sync_logs_virtual_window`]: the projection
/// inputs plus the mounted-tree queries, bundled so the system keeps a small,
/// typed parameter list (BEVY-ECS-009).
#[derive(SystemParam)]
pub(crate) struct LogsVirtualSurface<'w, 's> {
    state: ResMut<'w, LogsVirtualState>,
    last: Option<Res<'w, LastLogsProjection>>,
    palette: Res<'w, UiPalette>,
    scrolls: LogsScrollQuery<'w, 's>,
    rows_globals: Query<'w, 's, &'static UiGlobalTransform, With<LogsVirtualNodes>>,
    containers: Query<'w, 's, Entity, With<LogsVirtualNodes>>,
    slots: Query<
        'w,
        's,
        (
            Entity,
            &'static LogsVirtualSlot,
            &'static mut Node,
            Option<&'static Children>,
        ),
        Without<LogsVirtualSpacer>,
    >,
    spacers:
        Query<'w, 's, (&'static LogsVirtualSpacer, &'static mut Node), Without<LogsVirtualSlot>>,
    row_nodes: Query<'w, 's, &'static ComputedNode>,
}

/// Mount exactly the shared recycler window of the log entry stream into the
/// virtual container. Inert unless the large-list container is mounted.
pub(crate) fn sync_logs_virtual_window(mut commands: Commands, surface: LogsVirtualSurface) {
    let LogsVirtualSurface {
        mut state,
        last,
        palette,
        scrolls,
        rows_globals,
        containers,
        mut slots,
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

    let (offset, viewport) = scrolls.iter().next().map_or(
        (0.0, LOGS_VIRTUAL_FALLBACK_VIEWPORT_PX),
        |(position, computed, scroll_global)| {
            let height = computed
                .map(|node| node.size().y * node.inverse_scale_factor())
                .filter(|height| *height > 0.0)
                .unwrap_or(LOGS_VIRTUAL_FALLBACK_VIEWPORT_PX);
            let offset = virtual_window_offset_px(
                position,
                scroll_global,
                rows_globals.iter().next(),
                computed,
            );
            (offset, height)
        },
    );

    let signature = entry_signature(&projection.entries);
    let structure_changed = signature != state.signature;

    if structure_changed {
        state.signature = signature;
        state.list = VirtualListState::new_variable_extent(
            projection.entries.len(),
            LOG_ROW_HEIGHT_PX,
            viewport,
        )
        .with_overscan(LOGS_VIRTUAL_OVERSCAN);
    } else {
        state.list.set_viewport_height(viewport);
    }
    let _ = state.list.set_scroll_offset(offset);

    let window = state.list.window();

    // One shared band rule sizes the pool: bounded by the viewport, never by the
    // ring buffer. The floor is this page's policy — a small or jittering viewport
    // must not make the pool resize every frame.
    let capacity = max_band_rows(LOG_ROW_HEIGHT_PX, viewport, LOGS_VIRTUAL_OVERSCAN).max(8);
    if structure_changed || capacity != state.capacity || state.slots.is_empty() {
        spawn_slots(&mut commands, &mut state, container, capacity);
    }

    // The recycle rule is the shared owner's; this page maps its slot actions
    // onto the slot entities it owns.
    let actions = sync_virtual_slots(&window, &mut state.slot_states);
    let rebind_happened = !actions.is_empty();
    for action in actions {
        match action {
            SlotAction::Rebind { slot, index, .. } => {
                let Some(entity) = state.slots.get(slot).copied() else {
                    continue;
                };
                commands.entity(entity).despawn_children();
                if let Some(entry) = projection.entries.get(index) {
                    let scene = log_row_scene(index, entry, &palette);
                    commands.spawn_scene(scene).insert(ChildOf(entity));
                }
            }
            SlotAction::Release { slot, .. } => {
                let Some(entity) = state.slots.get(slot).copied() else {
                    continue;
                };
                commands.entity(entity).despawn_children();
            }
        }
    }

    // Slot heights track their bound row so the mounted window and the scroll
    // extent agree. Inactive slots collapse to zero height.
    for (_entity, slot, mut node, children) in &mut slots {
        let active = slot.slot_idx < window.materialized_len();
        let item_idx = window.start + slot.slot_idx;
        let height = if active {
            state
                .list
                .item_extent(item_idx)
                .unwrap_or(LOG_ROW_HEIGHT_PX)
        } else {
            0.0
        };
        let target = px(height);
        if node.height != target {
            node.height = target;
        }
        // Variable extent: feed the mounted row's measured layout height back
        // into the index so wrapped messages never overlap. Skip the frame that
        // rebound slots (the old child would measure against the new index).
        if active
            && !rebind_happened
            && let Some(child) = children.and_then(|children| children.iter().next())
            && let Ok(computed) = row_nodes.get(*child)
        {
            let measured = computed.size().y * computed.inverse_scale_factor();
            if measured > 0.0 {
                let _ = state.list.update_measured_extent(item_idx, measured);
            }
        }
    }
    for (spacer, mut node) in &mut spacers {
        let target = px(if spacer.top {
            window.top
        } else {
            window.bottom
        });
        if node.height != target {
            node.height = target;
        }
    }
}

#[cfg(test)]
mod tests {
    use zhuge_ui_core::virtual_list::VirtualWindow;

    use super::*;
    use crate::pages::logs::LogsProjection;
    use infiltrator_contract::logs::LogLevel;
    use lurker_core_contract::page::PageStatus;

    fn entry(index: usize) -> LogEntry {
        LogEntry {
            id: index as u64,
            timestamp: format!("10:00:{:02}.000", index % 60),
            level: LogLevel::Info,
            tag: "TCP".to_owned(),
            message: format!("log line {index}"),
        }
    }

    fn projection_with(count: usize) -> LogsProjection {
        LogsProjection {
            status: PageStatus::Ready,
            generation: 0,
            session_token: None,
            total_entries: count,
            active_level: None,
            entries: (0..count).map(entry).collect(),
        }
    }

    #[test]
    fn large_log_stream_mounts_a_bounded_window_not_the_full_list() {
        let projection = projection_with(50_000);
        let viewport = LOGS_VIRTUAL_FALLBACK_VIEWPORT_PX;
        let list = VirtualListState::new_variable_extent(
            projection.entries.len(),
            LOG_ROW_HEIGHT_PX,
            viewport,
        )
        .with_overscan(LOGS_VIRTUAL_OVERSCAN);
        let bound = max_band_rows(LOG_ROW_HEIGHT_PX, viewport, LOGS_VIRTUAL_OVERSCAN);
        let window = list.window();
        assert!(
            window.materialized_len() <= bound,
            "mounted window {} must be bounded by {bound}",
            window.materialized_len()
        );
        assert!(
            window.materialized_len() < projection.entries.len(),
            "a 50k-log stream must not mount every row"
        );
    }

    #[test]
    fn growing_stream_slides_the_window_and_the_tail_is_reachable() {
        let mut projection = projection_with(10);
        let viewport = LOGS_VIRTUAL_FALLBACK_VIEWPORT_PX;
        let mut list = VirtualListState::new_variable_extent(
            projection.entries.len(),
            LOG_ROW_HEIGHT_PX,
            viewport,
        )
        .with_overscan(LOGS_VIRTUAL_OVERSCAN);

        let top = list.window();
        assert_eq!(top.start, 0);

        // The ring buffer grows; the window follows the new total.
        for index in 10..50_000 {
            projection.entries.push(entry(index));
        }
        list.set_item_count(projection.entries.len());
        let _ = list.scroll_to_bottom();
        let bottom = list.window();
        assert_eq!(bottom.end, projection.entries.len());
        assert!(bottom.top > 0.0);
        assert!(bottom.bottom <= f32::EPSILON);
        assert!(
            bottom.start > top.start,
            "the window must slide as the offset advances"
        );
    }

    #[test]
    fn signature_tracks_growth_and_stable_identity() {
        let base = projection_with(5);
        let signature = entry_signature(&base.entries);
        // A same-length restamp keeps the mounted topology stable.
        let mut restamped = projection_with(5);
        restamped.entries[0].message = "fresh copy".to_owned();
        assert_eq!(signature, entry_signature(&restamped.entries));
        // Growth changes the signature so the window rebuilds.
        let mut grown = projection_with(5);
        grown.entries.push(entry(99));
        assert_ne!(signature, entry_signature(&grown.entries));
    }

    #[test]
    fn rebinding_preserves_stable_slot_identity() {
        // The shared recycle rule owns which slot shows which row; this page
        // owns the slot entities and maps the owner's slot actions onto them.
        // A rebind replaces the slot's children, never the slot entity itself,
        // so a stale queued click on the removed child cannot fire.
        let mut slots: Vec<VirtualScrollSlot> = (0..16)
            .map(|slot| VirtualScrollSlot {
                slot,
                ..VirtualScrollSlot::default()
            })
            .collect();

        let window = VirtualWindow::visible_for_sticky_rows(20, 0.0, 200.0, 20.0);
        let first = sync_virtual_slots(&window, &mut slots);
        assert_eq!(
            first.len(),
            10,
            "the window binds exactly its materialized rows"
        );
        assert_eq!(
            slots
                .iter()
                .filter(|slot| slot.bound_index().is_some())
                .count(),
            10,
            "ten slots are bound after the first sync"
        );

        // Slide the window: the same slots are reused, never respawned.
        let scrolled = VirtualWindow::visible_for_sticky_rows(20, 100.0, 200.0, 20.0);
        let second = sync_virtual_slots(&scrolled, &mut slots);
        assert!(
            second.iter().all(|action| action.slot() < 16),
            "every action names a slot of this pool"
        );
        assert_eq!(
            slots
                .iter()
                .filter(|slot| slot.bound_index().is_some())
                .count(),
            10,
            "the pool stays bounded by the window, not by the dataset"
        );
    }
}
