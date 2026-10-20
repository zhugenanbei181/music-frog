//! Virtual-scrolling adapter core: shared fixed/variable geometry projection and the
//! zero-allocation entity recycling pool.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use zhuge_ui_core::virtual_list::{self, VariableExtentIndex, VirtualListConfig};

// ===========================================================================
// 1. Fixed-Height Virtual Window Math
// ===========================================================================

/// The half-open visible window `[start, end)` of item indices.
pub fn visible_window(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
) -> (usize, usize) {
    if item_count == 0 || viewport_h <= 0.0 {
        return (0, 0);
    }
    let window = VirtualListConfig::new(item_count, row_h, viewport_h)
        .with_scroll_offset(scroll_offset)
        .compute_visible_viewport();
    (window.start, window.end)
}

/// Resolved virtual window geometry for recycled virtual lists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualWindow {
    /// Starting index (inclusive) of the window including overscan buffer.
    pub start: usize,
    /// Ending index (exclusive) of the window including overscan buffer.
    pub end: usize,
    /// Number of items mounted in this window (`end - start`).
    pub visible_count: usize,
    /// Virtual spacer height (px) preceding the first mounted row.
    pub top_spacer_px: f32,
    /// Virtual spacer height (px) succeeding the last mounted row.
    pub bottom_spacer_px: f32,
    /// Total virtual height (px) of all `item_count` rows.
    pub total_height_px: f32,
}

impl VirtualWindow {
    pub(super) fn from_shared(window: virtual_list::VirtualWindow, total_height_px: f32) -> Self {
        let visible_count = window.end.saturating_sub(window.start);
        let (top_spacer_px, bottom_spacer_px) = if visible_count == 0 && total_height_px > 0.0 {
            (total_height_px, 0.0)
        } else {
            (window.top, window.bottom)
        };
        Self {
            start: window.start,
            end: window.end,
            visible_count,
            top_spacer_px,
            bottom_spacer_px,
            total_height_px,
        }
    }
}

/// Compute a virtual window with an overscan buffer for smooth scrolling.
pub fn visible_window_with_overscan(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
    overscan: usize,
) -> VirtualWindow {
    if item_count == 0 {
        return VirtualWindow {
            start: 0,
            end: 0,
            visible_count: 0,
            top_spacer_px: 0.0,
            bottom_spacer_px: 0.0,
            total_height_px: 0.0,
        };
    }
    let row_extent = if row_h.is_finite() && row_h > 0.0 {
        row_h
    } else {
        1.0
    };
    if viewport_h.is_finite() && viewport_h <= 0.0 {
        let total_height_px = item_count as f32 * row_extent;
        return VirtualWindow {
            start: 0,
            end: 0,
            visible_count: 0,
            top_spacer_px: total_height_px,
            bottom_spacer_px: 0.0,
            total_height_px,
        };
    }
    let shared_window = VirtualListConfig::new(item_count, row_h, viewport_h)
        .with_scroll_offset(scroll_offset)
        .with_overscan(overscan)
        .compute_viewport();
    let visible_count = shared_window.materialized_len();
    let top_spacer_px = shared_window.top;
    let bottom_spacer_px = shared_window.bottom;
    let total_height_px = top_spacer_px + visible_count as f32 * row_extent + bottom_spacer_px;

    VirtualWindow {
        start: shared_window.start,
        end: shared_window.end,
        visible_count,
        top_spacer_px,
        bottom_spacer_px,
        total_height_px,
    }
}

/// Clamp a raw scroll offset to valid bounds for a virtual list.
pub fn clamp_scroll_offset(
    scroll_offset: f32,
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
) -> f32 {
    if item_count == 0 || viewport_h <= 0.0 {
        return 0.0;
    }
    VirtualListConfig::new(item_count, row_h, viewport_h)
        .with_scroll_offset(scroll_offset)
        .clamped_scroll_offset()
}

// ===========================================================================
// 2. Variable Extent Index
// ===========================================================================

// Prefix sums, extent updates, resize outcomes, and variable viewport geometry are
// owned by `zhuge-ui-core`; this crate keeps only the Bevy scroll adapter below.

// ===========================================================================
// 3. Zero-Allocation Virtual Entity Pool & Recycler
// ===========================================================================

/// What a recycled pool slot currently displays.
///
/// The old design kept "which item is bound" and "is the slot active" in two
/// fields that were always written together, so "active but unbound" was
/// representable and only an implicit guard kept it from being observed. The
/// pool kept a second copy of the same pair in its slot tuple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SlotBinding {
    /// The slot is free: it displays no dataset item.
    #[default]
    Free,
    /// The slot displays one dataset item.
    Bound {
        /// The dataset item index the slot displays.
        item_idx: usize,
    },
}

/// A recycled pool slot entity tracking its bound dataset index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VirtualSlot {
    /// Slot index in the pre-allocated pool `0..capacity`.
    pub slot_idx: usize,
    /// What the slot currently displays.
    pub binding: SlotBinding,
    /// Monotonically increasing generation number tracking rebind events.
    pub generation: u64,
}

impl VirtualSlot {
    pub const fn new(slot_idx: usize) -> Self {
        Self {
            slot_idx,
            binding: SlotBinding::Free,
            generation: 0,
        }
    }

    /// The dataset item the slot displays, if any.
    pub fn bound_item_idx(&self) -> Option<usize> {
        match self.binding {
            SlotBinding::Free => None,
            SlotBinding::Bound { item_idx } => Some(item_idx),
        }
    }

    /// Whether the slot currently displays a dataset item.
    pub fn is_active(&self) -> bool {
        matches!(self.binding, SlotBinding::Bound { .. })
    }

    /// Whether this slot is actively bound to `item_idx`.
    pub fn is_bound_to(&self, item_idx: usize) -> bool {
        matches!(self.binding, SlotBinding::Bound { item_idx: bound } if bound == item_idx)
    }

    /// Strict guard against stale / queued clicks or delayed command receipts:
    /// returns true only if the slot is currently bound to `expected_item_idx`,
    /// and has not been rebound since `expected_generation`.
    pub fn is_valid_receipt(&self, expected_item_idx: usize, expected_generation: u64) -> bool {
        self.is_bound_to(expected_item_idx) && self.generation == expected_generation
    }

    /// Apply rebind action to update the slot's tracking state.
    pub fn apply_rebind(&mut self, item_idx: usize, generation: u64) {
        self.binding = SlotBinding::Bound { item_idx };
        self.generation = generation;
    }

    /// Apply deactivation to clear the slot's tracking state.
    pub fn apply_deactivate(&mut self, generation: u64) {
        self.binding = SlotBinding::Free;
        self.generation = generation;
    }
}

/// Action to apply when synchronizing the entity pool to a new virtual window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SlotRebindAction {
    /// Rebind slot entity to dataset item `item_idx` with top offset `top_offset_px`.
    Rebind {
        slot_idx: usize,
        entity: Entity,
        item_idx: usize,
        top_offset_px: f32,
        generation: u64,
    },
    /// Deactivate slot entity (hide or move offscreen).
    Deactivate {
        slot_idx: usize,
        entity: Entity,
        generation: u64,
    },
}

impl SlotRebindAction {
    pub const fn slot_idx(&self) -> usize {
        match *self {
            Self::Rebind { slot_idx, .. } | Self::Deactivate { slot_idx, .. } => slot_idx,
        }
    }

    pub const fn entity(&self) -> Entity {
        match *self {
            Self::Rebind { entity, .. } | Self::Deactivate { entity, .. } => entity,
        }
    }

    pub const fn generation(&self) -> u64 {
        match *self {
            Self::Rebind { generation, .. } | Self::Deactivate { generation, .. } => generation,
        }
    }
}

/// One slot of the entity pool: the entity plus what it currently displays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PoolSlot {
    entity: Entity,
    binding: SlotBinding,
    generation: u64,
}

/// Entity object pool manager for virtual lists.
///
/// Ensures exactly `pool_capacity` entities are spawned once and perpetually recycled.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualEntityPool {
    capacity: usize,
    slots: Vec<PoolSlot>,
}

impl VirtualEntityPool {
    /// Create a new entity pool from pre-spawned slot entities.
    pub fn new(slot_entities: Vec<Entity>) -> Self {
        let capacity = slot_entities.len();
        let slots = slot_entities
            .into_iter()
            .map(|entity| PoolSlot {
                entity,
                binding: SlotBinding::Free,
                generation: 0,
            })
            .collect();
        Self { capacity, slots }
    }

    /// Pool capacity in slots.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of currently bound slots.
    pub fn active_count(&self) -> usize {
        self.slots
            .iter()
            .filter(|slot| matches!(slot.binding, SlotBinding::Bound { .. }))
            .count()
    }

    /// Return the binding and generation of a slot by its slot_idx.
    pub fn slot_binding(&self, slot_idx: usize) -> Option<(SlotBinding, u64)> {
        self.slots
            .get(slot_idx)
            .map(|slot| (slot.binding, slot.generation))
    }

    /// Return the current generation of a slot.
    pub fn generation(&self, slot_idx: usize) -> Option<u64> {
        self.slots.get(slot_idx).map(|slot| slot.generation)
    }

    /// Whether a queued click / late receipt for `(slot_idx, expected_item_idx, expected_generation)` is valid.
    pub fn is_receipt_valid(
        &self,
        slot_idx: usize,
        expected_item_idx: usize,
        expected_generation: u64,
    ) -> bool {
        match self.slots.get(slot_idx) {
            Some(slot) => {
                matches!(
                    slot.binding,
                    SlotBinding::Bound { item_idx } if item_idx == expected_item_idx
                ) && slot.generation == expected_generation
            }
            None => false,
        }
    }

    /// Compute rebind actions needed to display item range `[window_start, window_end)`.
    pub fn sync_window<F>(
        &mut self,
        window_start: usize,
        window_end: usize,
        offset_fn: F,
    ) -> Vec<SlotRebindAction>
    where
        F: Fn(usize) -> f32,
    {
        let needed_count = window_end.saturating_sub(window_start);
        let mut actions = Vec::with_capacity(self.capacity);

        for slot_idx in 0..self.capacity {
            if slot_idx < needed_count {
                let item_idx = window_start + slot_idx;
                let slot = &mut self.slots[slot_idx];

                let already_bound = matches!(
                    slot.binding,
                    SlotBinding::Bound { item_idx: bound } if bound == item_idx
                );
                if !already_bound {
                    slot.binding = SlotBinding::Bound { item_idx };
                    slot.generation = slot.generation.wrapping_add(1);
                    actions.push(SlotRebindAction::Rebind {
                        slot_idx,
                        entity: slot.entity,
                        item_idx,
                        top_offset_px: offset_fn(item_idx),
                        generation: slot.generation,
                    });
                }
            } else {
                let slot = &mut self.slots[slot_idx];
                if matches!(slot.binding, SlotBinding::Bound { .. }) {
                    slot.binding = SlotBinding::Free;
                    slot.generation = slot.generation.wrapping_add(1);
                    actions.push(SlotRebindAction::Deactivate {
                        slot_idx,
                        entity: slot.entity,
                        generation: slot.generation,
                    });
                }
            }
        }

        actions
    }
}

// ===========================================================================
// 4. Scroll Position Anchor Bookmark & Restoration
// ===========================================================================

/// Scroll anchor bookmark remembering relative viewport position across route navigations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchorBookmark {
    pub item_index: usize,
    pub offset_in_item_px: f32,
    pub original_scroll_offset_px: f32,
}

impl ScrollAnchorBookmark {
    /// Create an anchor bookmark for the current scroll position using a variable extent index.
    pub fn create(scroll_offset_px: f32, index_engine: &VariableExtentIndex) -> Self {
        let item_index = index_engine.item_at_offset(scroll_offset_px).unwrap_or(0);
        let item_top = index_engine.item_offset(item_index).unwrap_or(0.0);
        let offset_in_item_px = (scroll_offset_px - item_top).max(0.0);

        Self {
            item_index,
            offset_in_item_px,
            original_scroll_offset_px: scroll_offset_px,
        }
    }

    /// Restore scroll offset from this bookmark, adapting to any intervening row height mutations.
    pub fn restore_scroll_offset(&self, index_engine: &VariableExtentIndex) -> f32 {
        if index_engine.item_count() == 0 {
            return 0.0;
        }
        let clamped_idx = self.item_index.min(index_engine.item_count() - 1);
        let item_top = index_engine.item_offset(clamped_idx).unwrap_or(0.0);
        let item_h = index_engine.item_extent(clamped_idx).unwrap_or(0.0);
        let intra = self.offset_in_item_px.min(item_h);
        item_top + intra
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zhuge_ui_core::virtual_list::ExtentUpdate;

    #[test]
    fn test_scroll_anchor_bookmark_restoration() {
        let mut index = VariableExtentIndex::new(20, 50.0);
        assert_eq!(index.total_extent(), 1000.0);

        // Scroll offset 225.0 -> item 4 (top=200, intra=25)
        let bookmark = ScrollAnchorBookmark::create(225.0, &index);
        assert_eq!(bookmark.item_index, 4);
        assert_eq!(bookmark.offset_in_item_px, 25.0);

        // Perfect restoration without mutations
        assert_eq!(bookmark.restore_scroll_offset(&index), 225.0);

        // Mutate preceding row height: item 1 grows from 50 to 100 (+50px)
        assert_eq!(
            index.update_extent(1, 100.0),
            ExtentUpdate::Updated {
                index: 1,
                previous_extent: 50.0,
                extent: 100.0,
            }
        );
        // Restored offset should shift by +50px -> 275.0
        assert_eq!(bookmark.restore_scroll_offset(&index), 275.0);
    }
}
