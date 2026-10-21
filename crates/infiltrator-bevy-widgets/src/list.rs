//! High-Performance Virtual List, Viewport Culling, Variable Extent Indexing,
//! Entity Recycling Pool over Bevy's official ScrollArea.
//!
//! **Product adapter**:
//! - [`visible_window`] & [`visible_window_with_overscan`]: adapters over the shared
//!   `zhuge-ui-core` fixed-extent geometry;
//! - [`VariableExtentIndex`]: shared O(log N) binary search prefix-sum indexing;
//! - [`VirtualEntityPool`]: Zero-allocation slot recycling preventing entity churn over 10,000+ items;
//! - [`VirtualListState`]: Unified virtual list model for massive collections (1,000,000+ items).
//!
//! **Scene adapter**: [`list_scene`] and [`list_row_scene`] build clipped token-spaced columns
//! over nav-vocabulary rows with in-place component restamping on selection and theme flips.
//!
//! **Scrolling**: the viewport is Bevy's official [`ScrollArea`], the bar beside it is the
//! official [`bevy::ui_widgets::Scrollbar`], and a row is revealed through the official
//! [`bevy::ui_widgets::ScrollIntoView`] event. The list never keeps a scroll offset of its
//! own next to the viewport's.

pub mod scroll_core;
pub mod scrollbar;
use scroll_core::*;
use zhuge_ui_core::virtual_list::{ExtentUpdate, VariableExtentIndex, VirtualListConfig};

use crate::feathers_theme::Token;
use crate::nav::{NavActive, NavItem, NavLabel, nav_fill_token};
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::{Message, MessageReader};
use bevy::ecs::query::{Changed, QueryFilter, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::Query;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, GridPlacement, Node, Overflow, UiRect, percent,
};
use bevy::ui::widget::Text;
use bevy::ui::{UiSystems, ui_layout_system};
use bevy::ui_widgets::{ScrollArea, ScrollbarPlugin};
use scrollbar::{
    ListScrollContainer, bind_list_scrollbars, list_scroll_frame, reveal_selected_list_rows,
};

// ===========================================================================
// 5. Virtual List State
// ===========================================================================

/// Row height calculation mode.
#[derive(Clone, Debug, PartialEq)]
pub enum RowHeightMode {
    /// Uniform fixed row height.
    Fixed(f32),
    /// Measured variable row extents backed by the shared prefix-sum index.
    VariableExtent(VariableExtentIndex),
}

/// Explicit result of changing a fixed-height list's uniform extent.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FixedExtentUpdate {
    VariableExtentMode,
    InvalidExtent {
        extent_px: f32,
    },
    Unchanged {
        extent_px: f32,
    },
    Updated {
        previous_extent_px: f32,
        extent_px: f32,
    },
}

/// Explicit result of recording a measured row extent.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RowMeasurementOutcome {
    FixedExtentMode,
    VariableExtent(ExtentUpdate),
}

/// Explicit result of changing a list scroll offset.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScrollOffsetOutcome {
    InvalidRequest {
        requested_offset_px: f32,
        current_offset_px: f32,
    },
    Unchanged {
        offset_px: f32,
    },
    Clamped {
        requested_offset_px: f32,
        previous_offset_px: f32,
        offset_px: f32,
    },
    Changed {
        previous_offset_px: f32,
        offset_px: f32,
    },
}

/// Explicit result of changing a selected row.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionOutcome {
    NoItems,
    IndexOutOfRange {
        index: usize,
        item_count: usize,
    },
    Unchanged {
        selected_index: Option<usize>,
    },
    Changed {
        previous_index: Option<usize>,
        selected_index: Option<usize>,
    },
}

/// High-performance virtual list model with explicit height modes and operation outcomes.
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualListState {
    item_count: usize,
    height_mode: RowHeightMode,
    viewport_height_px: f32,
    scroll_offset_px: f32,
    overscan: usize,
    selected_index: Option<usize>,
}

impl Default for VirtualListState {
    fn default() -> Self {
        Self {
            item_count: 0,
            height_mode: RowHeightMode::Fixed(36.0),
            viewport_height_px: 0.0,
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
        }
    }
}

impl VirtualListState {
    /// Create a new virtual list state with uniform fixed row height.
    pub fn new(item_count: usize, row_height_px: f32, viewport_height_px: f32) -> Self {
        let mut state = Self {
            item_count,
            height_mode: RowHeightMode::Fixed(row_height_px.max(1.0)),
            viewport_height_px: viewport_height_px.max(0.0),
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
        };
        state.clamp_scroll();
        state
    }

    /// Create a virtual list using measured, variable row extents.
    pub fn new_variable_extent(
        item_count: usize,
        default_extent_px: f32,
        viewport_height_px: f32,
    ) -> Self {
        let mut state = Self {
            item_count,
            height_mode: RowHeightMode::VariableExtent(VariableExtentIndex::new(
                item_count,
                default_extent_px,
            )),
            viewport_height_px: viewport_height_px.max(0.0),
            scroll_offset_px: 0.0,
            overscan: 2,
            selected_index: None,
        };
        state.clamp_scroll();
        state
    }

    /// Set overscan buffer count.
    pub fn with_overscan(mut self, overscan: usize) -> Self {
        self.overscan = overscan;
        self
    }

    /// Set initial scroll offset.
    pub fn with_scroll_offset(mut self, offset_px: f32) -> Self {
        let _ = self.set_scroll_offset(offset_px);
        self
    }

    /// Set initial selected index.
    pub fn with_selected(mut self, selected: Option<usize>) -> Self {
        self.selected_index = selected.filter(|&idx| idx < self.item_count);
        self
    }

    // --- Getters ---

    /// Number of total items in the virtual dataset.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Row height in pixels (fixed mode baseline).
    pub fn row_height_px(&self) -> f32 {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => *h,
            RowHeightMode::VariableExtent(idx) => idx.default_extent(),
        }
    }

    /// Height mode descriptor.
    pub fn height_mode(&self) -> &RowHeightMode {
        &self.height_mode
    }

    /// Visible height of the viewport container in pixels.
    pub fn viewport_height_px(&self) -> f32 {
        self.viewport_height_px
    }

    /// Current scroll offset in pixels from top.
    pub fn scroll_offset_px(&self) -> f32 {
        self.scroll_offset_px
    }

    /// Number of overscan buffer items.
    pub fn overscan(&self) -> usize {
        self.overscan
    }

    /// Current selected item index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Total height of all items combined in pixels. O(1).
    pub fn total_height_px(&self) -> f32 {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => self.item_count as f32 * *h,
            RowHeightMode::VariableExtent(idx) => idx.total_extent(),
        }
    }

    /// Maximum valid scroll offset in pixels. O(1).
    pub fn max_scroll_offset(&self) -> f32 {
        (self.total_height_px() - self.viewport_height_px).max(0.0)
    }

    /// Whether the list is scrolled to top boundary.
    pub fn is_at_top(&self) -> bool {
        self.scroll_offset_px <= 0.0
    }

    /// Whether the list is scrolled to bottom boundary.
    pub fn is_at_bottom(&self) -> bool {
        self.scroll_offset_px >= self.max_scroll_offset()
    }

    /// Scroll progress fraction from 0.0 to 1.0.
    pub fn scroll_progress(&self) -> f32 {
        let max = self.max_scroll_offset();
        if max <= 0.0 {
            0.0
        } else {
            (self.scroll_offset_px / max).clamp(0.0, 1.0)
        }
    }

    /// The visible range `[start, end)` directly visible without overscan.
    pub fn visible_range(&self) -> (usize, usize) {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => visible_window(
                self.item_count,
                self.viewport_height_px,
                *h,
                self.scroll_offset_px,
            ),
            RowHeightMode::VariableExtent(idx) => {
                let win = idx.compute_window(self.viewport_height_px, self.scroll_offset_px, 0);
                (win.start, win.end)
            }
        }
    }

    /// The resolved virtual window including overscan buffer and spacer heights.
    pub fn window(&self) -> VirtualWindow {
        match &self.height_mode {
            RowHeightMode::Fixed(h) => visible_window_with_overscan(
                self.item_count,
                self.viewport_height_px,
                *h,
                self.scroll_offset_px,
                self.overscan,
            ),
            RowHeightMode::VariableExtent(idx) => VirtualWindow::from_shared(
                idx.compute_window(
                    self.viewport_height_px,
                    self.scroll_offset_px,
                    self.overscan,
                ),
                idx.total_extent(),
            ),
        }
    }

    /// Item vertical bounds (top_y, bottom_y) in content space.
    pub fn item_rect_y(&self, index: usize) -> Option<(f32, f32)> {
        let top = self.item_offset(index)?;
        let h = self.item_extent(index)?;
        Some((top, top + h))
    }

    /// Whether the item at index intersects the visible viewport.
    pub fn is_item_visible(&self, index: usize) -> bool {
        let (start, end) = self.visible_range();
        index >= start && index < end
    }

    /// Whether the item at index is inside the mounted overscan window.
    pub fn is_item_mounted(&self, index: usize) -> bool {
        let win = self.window();
        index >= win.start && index < win.end
    }

    /// Find item index at given vertical offset in content space.
    pub fn item_at_offset(&self, offset_y_px: f32) -> Option<usize> {
        match &self.height_mode {
            RowHeightMode::Fixed(height) => {
                VirtualListConfig::new(self.item_count, *height, self.viewport_height_px)
                    .item_at_offset(offset_y_px)
            }
            RowHeightMode::VariableExtent(idx) => idx.item_at_offset(offset_y_px),
        }
    }

    /// Top coordinate (px) in content space of the item at `index`.
    pub fn item_offset(&self, index: usize) -> Option<f32> {
        match &self.height_mode {
            RowHeightMode::Fixed(height) => {
                VirtualListConfig::new(self.item_count, *height, self.viewport_height_px)
                    .item_offset(index)
            }
            RowHeightMode::VariableExtent(idx) => idx.item_offset(index),
        }
    }

    /// Height of item at `index`.
    pub fn item_extent(&self, index: usize) -> Option<f32> {
        match &self.height_mode {
            RowHeightMode::Fixed(height) => {
                VirtualListConfig::new(self.item_count, *height, self.viewport_height_px)
                    .item_extent(index)
            }
            RowHeightMode::VariableExtent(idx) => idx.item_extent(index),
        }
    }

    // --- State Mutations ---

    fn clamp_scroll(&mut self) {
        let max_scroll = self.max_scroll_offset();
        self.scroll_offset_px = self.scroll_offset_px.clamp(0.0, max_scroll);
    }

    /// Update total item count and re-clamp scroll and selection.
    pub fn set_item_count(&mut self, count: usize) {
        self.item_count = count;
        if let RowHeightMode::VariableExtent(idx) = &mut self.height_mode {
            let _ = idx.resize(count);
        }
        self.clamp_scroll();
        if let Some(sel) = self.selected_index
            && sel >= count
        {
            self.selected_index = count.checked_sub(1);
        }
    }

    /// Update viewport height and re-clamp scroll offset.
    pub fn set_viewport_height(&mut self, height_px: f32) {
        self.viewport_height_px = height_px.max(0.0);
        self.clamp_scroll();
    }

    /// Change the fixed extent without silently replacing a measured variable index.
    pub fn set_fixed_extent(&mut self, extent_px: f32) -> FixedExtentUpdate {
        match &mut self.height_mode {
            RowHeightMode::Fixed(current) => {
                let next = if extent_px.is_finite() && extent_px > 0.0 {
                    extent_px
                } else {
                    return FixedExtentUpdate::InvalidExtent { extent_px };
                };
                if (*current - next).abs() <= f32::EPSILON {
                    return FixedExtentUpdate::Unchanged {
                        extent_px: *current,
                    };
                }
                let previous_extent_px = *current;
                *current = next;
                self.clamp_scroll();
                FixedExtentUpdate::Updated {
                    previous_extent_px,
                    extent_px: next,
                }
            }
            RowHeightMode::VariableExtent(_) => FixedExtentUpdate::VariableExtentMode,
        }
    }

    /// Record one measured row extent with an explicit mode and validation result.
    pub fn update_measured_extent(
        &mut self,
        row_index: usize,
        extent_px: f32,
    ) -> RowMeasurementOutcome {
        match &mut self.height_mode {
            RowHeightMode::Fixed(_) => RowMeasurementOutcome::FixedExtentMode,
            RowHeightMode::VariableExtent(index) => {
                let outcome = index.update_extent(row_index, extent_px);
                if matches!(outcome, ExtentUpdate::Updated { .. }) {
                    self.clamp_scroll();
                }
                RowMeasurementOutcome::VariableExtent(outcome)
            }
        }
    }

    /// Set overscan count.
    pub fn set_overscan(&mut self, overscan: usize) {
        self.overscan = overscan;
    }

    /// Set scroll offset directly and report invalid, clamped, unchanged, or changed state.
    pub fn set_scroll_offset(&mut self, offset_px: f32) -> ScrollOffsetOutcome {
        if !offset_px.is_finite() {
            return ScrollOffsetOutcome::InvalidRequest {
                requested_offset_px: offset_px,
                current_offset_px: self.scroll_offset_px,
            };
        }
        let old = self.scroll_offset_px;
        self.scroll_offset_px = offset_px;
        self.clamp_scroll();
        let next = self.scroll_offset_px;
        let changed = (next - old).abs() > f32::EPSILON;
        let clamped = (next - offset_px).abs() > f32::EPSILON;
        match (changed, clamped) {
            (false, false) => ScrollOffsetOutcome::Unchanged { offset_px: next },
            (false, true) => ScrollOffsetOutcome::Clamped {
                requested_offset_px: offset_px,
                previous_offset_px: old,
                offset_px: next,
            },
            (true, false) => ScrollOffsetOutcome::Changed {
                previous_offset_px: old,
                offset_px: next,
            },
            (true, true) => ScrollOffsetOutcome::Clamped {
                requested_offset_px: offset_px,
                previous_offset_px: old,
                offset_px: next,
            },
        }
    }

    /// Scroll by a delta in pixels.
    pub fn scroll_by(&mut self, delta_px: f32) -> ScrollOffsetOutcome {
        self.set_scroll_offset(self.scroll_offset_px + delta_px)
    }

    /// Scroll to top.
    pub fn scroll_to_top(&mut self) -> ScrollOffsetOutcome {
        self.set_scroll_offset(0.0)
    }

    /// Scroll to bottom.
    pub fn scroll_to_bottom(&mut self) -> ScrollOffsetOutcome {
        self.set_scroll_offset(self.max_scroll_offset())
    }

    /// Page down.
    pub fn page_down(&mut self) -> ScrollOffsetOutcome {
        self.scroll_by(self.viewport_height_px)
    }

    /// Page up.
    pub fn page_up(&mut self) -> ScrollOffsetOutcome {
        self.scroll_by(-self.viewport_height_px)
    }

    /// Select an index.
    pub fn select(&mut self, index: Option<usize>) -> SelectionOutcome {
        if let Some(idx) = index
            && idx >= self.item_count
        {
            return SelectionOutcome::IndexOutOfRange {
                index: idx,
                item_count: self.item_count,
            };
        }
        let previous_index = self.selected_index;
        if previous_index == index {
            return SelectionOutcome::Unchanged {
                selected_index: index,
            };
        }
        self.selected_index = index;
        SelectionOutcome::Changed {
            previous_index,
            selected_index: index,
        }
    }

    /// Select next item.
    pub fn select_next(&mut self) -> SelectionOutcome {
        if self.item_count == 0 {
            return SelectionOutcome::NoItems;
        }
        let next = match self.selected_index {
            Some(curr) => (curr + 1).min(self.item_count - 1),
            None => 0,
        };
        self.select(Some(next))
    }

    /// Select previous item.
    pub fn select_previous(&mut self) -> SelectionOutcome {
        if self.item_count == 0 {
            return SelectionOutcome::NoItems;
        }
        let prev = match self.selected_index {
            Some(curr) => curr.saturating_sub(1),
            None => 0,
        };
        self.select(Some(prev))
    }
}

// ===========================================================================
// 6. Bevy ECS Components & Messages
// ===========================================================================

/// Component carrying virtual list state.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct VirtualList(pub VirtualListState);

/// Message requesting a scroll delta on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct VirtualListScroll(pub f32);

/// Message requesting a selection change on virtual lists.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct VirtualListSelect(pub Option<usize>);

#[derive(QueryFilter)]
pub struct ListChangedFilter {
    with_list: With<List>,
    changed_list_selection: Changed<ListSelection>,
}

/// Apply scroll deltas and selection requests to virtual list projections.
pub fn advance_virtual_lists(
    mut scrolls: MessageReader<VirtualListScroll>,
    mut selects: MessageReader<VirtualListSelect>,
    mut lists: Query<&mut VirtualList>,
) {
    for event in scrolls.read() {
        for mut list in &mut lists {
            let _ = list.0.scroll_by(event.0);
        }
    }
    for event in selects.read() {
        for mut list in &mut lists {
            let _ = list.0.select(event.0);
        }
    }
}

/// Marker on the list's scroll container: the column that owns the rows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct List;

/// The list-owned selection: the index of the selected row, or `None`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListSelection(pub Option<usize>);

/// One list row: the nav-item pill vocabulary.
pub fn list_row_scene(label: String, selected: bool, palette: &UiPalette) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                min_height: rem(palette.control_height_px),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ nav_fill_token(selected) })
            NavItem
            NavActive({ selected })
            Children [
                Text(label) TextRole(Role::Body) NavLabel
            ]
    })
}

/// The list: a clipped, token-spaced column over caller-composed rows, framed
/// by the official scrollbar beside the official scroll container.
///
/// The container owns the rows and the selection; the bar is its sibling, so
/// the official scrollbar can drive the container's own scroll position.
pub fn list_scene(rows: Vec<Box<dyn Scene>>, selected: Option<usize>) -> impl Scene + use<> {
    let container = Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S4),
                overflow: Overflow::scroll_y(),
                grid_row: GridPlacement::start(1),
                grid_column: GridPlacement::start(1),
            }
            ThemeBackgroundColor({ Token::Surface })
            List
            ListSelection({ selected })
            ScrollArea
            ListScrollContainer
            Children [
                { rows }
            ]
    }) as Box<dyn Scene>;
    list_scroll_frame(container)
}

/// Project the list's [`ListSelection`] onto its rows' [`NavActive`] bits.
pub fn sync_list_selection(
    mut lists: Query<(&ListSelection, &Children), ListChangedFilter>,
    mut rows: Query<&mut NavActive>,
) {
    for (selection, children) in &mut lists {
        for (index, child) in children.iter().enumerate() {
            if let Ok(mut active) = rows.get_mut(*child) {
                let target = NavActive(selection.0 == Some(index));
                if active.0 != target.0 {
                    *active = target;
                }
            }
        }
    }
}

/// The official scroll primitives the shared list needs: the scrollbar
/// plugin, the bar-to-container binding and the official scroll-into-view
/// that reveals the selected row.
pub struct ListScrollPlugin;

impl Plugin for ListScrollPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<ScrollbarPlugin>() {
            app.add_plugins(ScrollbarPlugin);
        }
        app.add_systems(Update, bind_list_scrollbars);
        // The reveal runs after the official layout so the viewport's measured
        // geometry is the one the observer reads.
        app.add_systems(
            PostUpdate,
            reveal_selected_list_rows
                .in_set(UiSystems::Layout)
                .after(ui_layout_system),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_extent_adapter_uses_shared_prefix_index() {
        let mut index = VariableExtentIndex::new(5, 30.0);
        assert_eq!(index.total_extent(), 150.0);
        assert_eq!(index.item_offset(0), Some(0.0));
        assert_eq!(index.item_offset(3), Some(90.0));

        // Update height of item 1 from 30px to 50px
        assert_eq!(
            index.update_extent(1, 50.0),
            ExtentUpdate::Updated {
                index: 1,
                previous_extent: 30.0,
                extent: 50.0,
            }
        );
        assert_eq!(index.total_extent(), 170.0);
        assert_eq!(index.item_offset(0), Some(0.0));
        assert_eq!(index.item_offset(1), Some(30.0));
        assert_eq!(index.item_offset(2), Some(80.0));
        assert_eq!(index.item_offset(3), Some(110.0));

        // Binary search lookup
        assert_eq!(index.item_at_offset(0.0), Some(0));
        assert_eq!(index.item_at_offset(29.9), Some(0));
        assert_eq!(index.item_at_offset(30.0), Some(1));
        assert_eq!(index.item_at_offset(79.9), Some(1));
        assert_eq!(index.item_at_offset(80.0), Some(2));
        assert_eq!(index.item_at_offset(160.0), Some(4));
    }
}
