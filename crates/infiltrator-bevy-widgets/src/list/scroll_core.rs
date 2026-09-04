//! Virtual-scrolling engine core: fixed-height window math, dynamic row
//! height indexing, inertial scroll physics, and the zero-allocation
//! entity recycling pool.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;

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
    if item_count == 0 || viewport_h <= 0.0 || row_h <= 0.0 {
        return (0, 0);
    }
    let visible = ((viewport_h / row_h).ceil() as usize).clamp(1, item_count);
    let scrolled = ((scroll_offset / row_h).floor().max(0.0)) as usize;
    let start = scrolled.min(item_count - visible);
    (start, start + visible)
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

/// Compute a virtual window with an overscan buffer for smooth scrolling.
pub fn visible_window_with_overscan(
    item_count: usize,
    viewport_h: f32,
    row_h: f32,
    scroll_offset: f32,
    overscan: usize,
) -> VirtualWindow {
    if item_count == 0 || viewport_h <= 0.0 || row_h <= 0.0 {
        return VirtualWindow {
            start: 0,
            end: 0,
            visible_count: 0,
            top_spacer_px: 0.0,
            bottom_spacer_px: 0.0,
            total_height_px: 0.0,
        };
    }
    let total_height_px = item_count as f32 * row_h;
    let visible_rows = ((viewport_h / row_h).ceil() as usize).clamp(1, item_count);
    let scrolled = ((scroll_offset / row_h).floor().max(0.0)) as usize;
    let base_start = scrolled.min(item_count.saturating_sub(visible_rows));
    let base_end = (base_start + visible_rows).min(item_count);

    let start = base_start.saturating_sub(overscan);
    let end = (base_end + overscan).min(item_count);
    let visible_count = end.saturating_sub(start);
    let top_spacer_px = start as f32 * row_h;
    let bottom_spacer_px = (item_count.saturating_sub(end)) as f32 * row_h;

    VirtualWindow {
        start,
        end,
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
    let max_scroll = (item_count as f32 * row_h - viewport_h).max(0.0);
    scroll_offset.clamp(0.0, max_scroll)
}

// ===========================================================================
// 2. Dynamic Row Height Indexing (Binary Search Prefix Sums)
// ===========================================================================

/// Dynamic row height indexing engine using cumulative prefix sums and binary search.
///
/// Supports arbitrary non-uniform row heights with $O(\log N)$ viewport range queries,
/// $O(1)$ coordinate offsets, and dynamic measurement updates.
#[derive(Clone, Debug, PartialEq)]
pub struct DynamicHeightIndex {
    item_count: usize,
    default_height_px: f32,
    heights: Vec<f32>,
    prefix_sums: Vec<f32>,
}

impl Default for DynamicHeightIndex {
    fn default() -> Self {
        Self::new(0, 36.0)
    }
}

impl DynamicHeightIndex {
    /// Create a new dynamic height index with uniform default estimated heights.
    pub fn new(item_count: usize, default_height_px: f32) -> Self {
        let def_h = default_height_px.max(1.0);
        let heights = vec![def_h; item_count];
        let mut prefix_sums = Vec::with_capacity(item_count + 1);
        prefix_sums.push(0.0);
        let mut cum = 0.0;
        for &h in &heights {
            cum += h;
            prefix_sums.push(cum);
        }

        Self {
            item_count,
            default_height_px: def_h,
            heights,
            prefix_sums,
        }
    }

    /// Number of items indexed.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Default row height used for unmeasured items.
    pub fn default_height_px(&self) -> f32 {
        self.default_height_px
    }

    /// Total combined height in pixels of all items. O(1).
    pub fn total_height_px(&self) -> f32 {
        *self.prefix_sums.last().unwrap_or(&0.0)
    }

    /// Height of item at `index` in pixels. O(1).
    pub fn item_height(&self, index: usize) -> Option<f32> {
        self.heights.get(index).copied()
    }

    /// Vertical top offset (px) of item at `index` in content coordinates. O(1).
    pub fn item_offset_y(&self, index: usize) -> Option<f32> {
        if index < self.item_count {
            Some(self.prefix_sums[index])
        } else {
            None
        }
    }

    /// Vertical bounding interval `(top_y, bottom_y)` of item at `index`. O(1).
    pub fn item_rect_y(&self, index: usize) -> Option<(f32, f32)> {
        if index < self.item_count {
            Some((self.prefix_sums[index], self.prefix_sums[index + 1]))
        } else {
            None
        }
    }

    /// Find item index located at content vertical offset `offset_y_px`.
    ///
    /// Executes binary search in $O(\log N)$ time.
    pub fn find_index_at_offset(&self, offset_y_px: f32) -> Option<usize> {
        if self.item_count == 0 || offset_y_px < 0.0 {
            return None;
        }
        if offset_y_px >= self.total_height_px() {
            return Some(self.item_count - 1);
        }

        // Binary search prefix_sums: find index where prefix_sums[i] <= offset_y < prefix_sums[i+1]
        let idx = match self.prefix_sums.binary_search_by(|probe| {
            probe
                .partial_cmp(&offset_y_px)
                .unwrap_or(std::cmp::Ordering::Equal)
        }) {
            Ok(exact) => exact.min(self.item_count.saturating_sub(1)),
            Err(insertion) => insertion
                .saturating_sub(1)
                .min(self.item_count.saturating_sub(1)),
        };

        Some(idx)
    }

    /// Record a measured dynamic row height for item at `index`.
    ///
    /// Recomputes suffix prefix sums. Returns `true` if height changed.
    pub fn update_measured_height(&mut self, index: usize, measured_height_px: f32) -> bool {
        if index >= self.item_count {
            return false;
        }
        let new_h = measured_height_px.max(1.0);
        let old_h = self.heights[index];
        if (new_h - old_h).abs() <= f32::EPSILON {
            return false;
        }

        let delta = new_h - old_h;
        self.heights[index] = new_h;

        // Update subsequent prefix sums
        for i in (index + 1)..=self.item_count {
            self.prefix_sums[i] += delta;
        }

        true
    }

    /// Resize the item count, extending with default heights or truncating.
    pub fn resize(&mut self, new_count: usize) {
        if new_count == self.item_count {
            return;
        }
        if new_count < self.item_count {
            self.heights.truncate(new_count);
            self.prefix_sums.truncate(new_count + 1);
            self.item_count = new_count;
        } else {
            let mut cum = self.total_height_px();
            for _ in self.item_count..new_count {
                self.heights.push(self.default_height_px);
                cum += self.default_height_px;
                self.prefix_sums.push(cum);
            }
            self.item_count = new_count;
        }
    }

    /// Compute visible virtual window with dynamic row heights in $O(\log N)$ time.
    pub fn compute_window(
        &self,
        viewport_h: f32,
        scroll_offset: f32,
        overscan: usize,
    ) -> VirtualWindow {
        if self.item_count == 0 || viewport_h <= 0.0 {
            return VirtualWindow {
                start: 0,
                end: 0,
                visible_count: 0,
                top_spacer_px: 0.0,
                bottom_spacer_px: 0.0,
                total_height_px: 0.0,
            };
        }

        let total_h = self.total_height_px();
        let clamped_scroll = scroll_offset.clamp(0.0, (total_h - viewport_h).max(0.0));

        let base_start = self.find_index_at_offset(clamped_scroll).unwrap_or(0);
        let base_end = self
            .find_index_at_offset(clamped_scroll + viewport_h)
            .map(|i| (i + 1).min(self.item_count))
            .unwrap_or(self.item_count);

        let start = base_start.saturating_sub(overscan);
        let end = (base_end + overscan).min(self.item_count);
        let visible_count = end.saturating_sub(start);

        let top_spacer_px = self.prefix_sums[start];
        let bottom_spacer_px = total_h - self.prefix_sums[end];

        VirtualWindow {
            start,
            end,
            visible_count,
            top_spacer_px,
            bottom_spacer_px,
            total_height_px: total_h,
        }
    }
}

// ===========================================================================
// 3. Smooth Inertial Scrolling Engine
// ===========================================================================

/// Physics-based smooth inertial scrolling and spring damping engine.
#[derive(Clone, Debug, PartialEq)]
pub struct InertialScroller {
    velocity_px_s: f32,
    friction_decay: f32,
    spring_target: Option<f32>,
    spring_stiffness: f32,
    spring_damping: f32,
}

impl Default for InertialScroller {
    fn default() -> Self {
        Self {
            velocity_px_s: 0.0,
            friction_decay: 7.5,
            spring_target: None,
            spring_stiffness: 220.0,
            spring_damping: 26.0,
        }
    }
}

impl InertialScroller {
    /// Create a new scroller with customized friction decay.
    pub fn new(friction_decay: f32) -> Self {
        Self {
            velocity_px_s: 0.0,
            friction_decay: friction_decay.max(0.1),
            spring_target: None,
            spring_stiffness: 220.0,
            spring_damping: 26.0,
        }
    }

    /// Current velocity in pixels per second.
    pub fn velocity(&self) -> f32 {
        self.velocity_px_s
    }

    /// Whether an active inertial scroll or spring animation is running.
    pub fn is_active(&self) -> bool {
        self.velocity_px_s.abs() > 1.0 || self.spring_target.is_some()
    }

    /// Apply an instantaneous fling gesture with an initial velocity.
    pub fn fling(&mut self, velocity_px_s: f32) {
        self.spring_target = None;
        self.velocity_px_s = velocity_px_s;
    }

    /// Add a velocity impulse to current motion.
    pub fn add_velocity(&mut self, delta_v: f32) {
        self.spring_target = None;
        self.velocity_px_s += delta_v;
    }

    /// Trigger a smooth spring scroll towards target offset.
    pub fn scroll_to_smooth(&mut self, target_offset_px: f32) {
        self.spring_target = Some(target_offset_px);
    }

    /// Stop all motion immediately.
    pub fn stop(&mut self) {
        self.velocity_px_s = 0.0;
        self.spring_target = None;
    }

    /// Step the simulation with delta time `dt_secs`.
    ///
    /// Returns the updated scroll offset and a boolean indicating if position changed.
    pub fn tick(
        &mut self,
        dt_secs: f32,
        current_offset_px: f32,
        max_offset_px: f32,
    ) -> (f32, bool) {
        let dt = dt_secs.clamp(0.0, 0.1);
        if dt <= 0.0 {
            return (current_offset_px, false);
        }

        if let Some(target) = self.spring_target {
            let clamped_target = target.clamp(0.0, max_offset_px);
            let displacement = clamped_target - current_offset_px;

            if displacement.abs() < 0.5 && self.velocity_px_s.abs() < 2.0 {
                self.stop();
                let final_offset = clamped_target;
                let changed = (final_offset - current_offset_px).abs() > f32::EPSILON;
                return (final_offset, changed);
            }

            // Hooke's spring force: F = -k * x - c * v
            let force =
                self.spring_stiffness * displacement - self.spring_damping * self.velocity_px_s;
            self.velocity_px_s += force * dt;
            let new_offset = current_offset_px + self.velocity_px_s * dt;
            let clamped_offset = new_offset.clamp(0.0, max_offset_px);
            let changed = (clamped_offset - current_offset_px).abs() > f32::EPSILON;
            return (clamped_offset, changed);
        }

        if self.velocity_px_s.abs() > 1.0 {
            let decay = (-self.friction_decay * dt).exp();
            let avg_v = self.velocity_px_s * (1.0 + decay) * 0.5;
            self.velocity_px_s *= decay;

            let delta = avg_v * dt;
            let new_offset = (current_offset_px + delta).clamp(0.0, max_offset_px);

            if (new_offset <= 0.0 && self.velocity_px_s < 0.0)
                || (new_offset >= max_offset_px && self.velocity_px_s > 0.0)
            {
                self.velocity_px_s = 0.0;
            }

            let changed = (new_offset - current_offset_px).abs() > f32::EPSILON;
            return (new_offset, changed);
        }

        self.velocity_px_s = 0.0;
        (current_offset_px, false)
    }
}

// ===========================================================================
// 4. Zero-Allocation Virtual Entity Pool & Recycler
// ===========================================================================

/// A recycled pool slot entity tracking its bound dataset index.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualSlot {
    /// Slot index in the pre-allocated pool `0..capacity`.
    pub slot_idx: usize,
    /// Currently bound data item index.
    pub bound_item_idx: Option<usize>,
    /// Whether this slot is actively visible in the current viewport window.
    pub is_active: bool,
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
    },
    /// Deactivate slot entity (hide or move offscreen).
    Deactivate { slot_idx: usize, entity: Entity },
}

/// Entity object pool manager for virtual lists.
///
/// Ensures exactly `pool_capacity` entities are spawned once and perpetually recycled.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualEntityPool {
    capacity: usize,
    slots: Vec<(Entity, Option<usize>, bool)>, // (Entity, bound_item_idx, is_active)
}

impl VirtualEntityPool {
    /// Create a new entity pool from pre-spawned slot entities.
    pub fn new(slot_entities: Vec<Entity>) -> Self {
        let capacity = slot_entities.len();
        let slots = slot_entities
            .into_iter()
            .map(|e| (e, None, false))
            .collect();
        Self { capacity, slots }
    }

    /// Pool capacity in slots.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of currently active bound slots.
    pub fn active_count(&self) -> usize {
        self.slots.iter().filter(|(_, _, active)| *active).count()
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
                let (entity, ref mut bound_idx, ref mut is_active) = self.slots[slot_idx];

                if *bound_idx != Some(item_idx) || !*is_active {
                    *bound_idx = Some(item_idx);
                    *is_active = true;
                    actions.push(SlotRebindAction::Rebind {
                        slot_idx,
                        entity,
                        item_idx,
                        top_offset_px: offset_fn(item_idx),
                    });
                }
            } else {
                let (entity, ref mut bound_idx, ref mut is_active) = self.slots[slot_idx];
                if *is_active {
                    *bound_idx = None;
                    *is_active = false;
                    actions.push(SlotRebindAction::Deactivate { slot_idx, entity });
                }
            }
        }

        actions
    }
}

// ===========================================================================
// 5. Scroll Position Anchor Bookmark & Restoration
// ===========================================================================

/// Scroll anchor bookmark remembering relative viewport position across route navigations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchorBookmark {
    pub item_index: usize,
    pub offset_in_item_px: f32,
    pub original_scroll_offset_px: f32,
}

impl ScrollAnchorBookmark {
    /// Create an anchor bookmark for the current scroll position using a dynamic height index.
    pub fn create(scroll_offset_px: f32, index_engine: &DynamicHeightIndex) -> Self {
        let item_index = index_engine
            .find_index_at_offset(scroll_offset_px)
            .unwrap_or(0);
        let item_top = index_engine.item_offset_y(item_index).unwrap_or(0.0);
        let offset_in_item_px = (scroll_offset_px - item_top).max(0.0);

        Self {
            item_index,
            offset_in_item_px,
            original_scroll_offset_px: scroll_offset_px,
        }
    }

    /// Restore scroll offset from this bookmark, adapting to any intervening row height mutations.
    pub fn restore_scroll_offset(&self, index_engine: &DynamicHeightIndex) -> f32 {
        if index_engine.item_count() == 0 {
            return 0.0;
        }
        let clamped_idx = self.item_index.min(index_engine.item_count() - 1);
        let item_top = index_engine.item_offset_y(clamped_idx).unwrap_or(0.0);
        let item_h = index_engine.item_height(clamped_idx).unwrap_or(0.0);
        let intra = self.offset_in_item_px.min(item_h);
        item_top + intra
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_anchor_bookmark_restoration() {
        let mut index = DynamicHeightIndex::new(20, 50.0);
        assert_eq!(index.total_height_px(), 1000.0);

        // Scroll offset 225.0 -> item 4 (top=200, intra=25)
        let bookmark = ScrollAnchorBookmark::create(225.0, &index);
        assert_eq!(bookmark.item_index, 4);
        assert_eq!(bookmark.offset_in_item_px, 25.0);

        // Perfect restoration without mutations
        assert_eq!(bookmark.restore_scroll_offset(&index), 225.0);

        // Mutate preceding row height: item 1 grows from 50 to 100 (+50px)
        index.update_measured_height(1, 100.0);
        // Restored offset should shift by +50px -> 275.0
        assert_eq!(bookmark.restore_scroll_offset(&index), 275.0);
    }
}
