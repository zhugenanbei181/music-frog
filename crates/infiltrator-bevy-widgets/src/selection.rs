//! Multi-Selection & Range Selection State Machine for Virtual Lists & Tables.
//!
//! **Pure State Core**:
//! - Single selection, contiguous Range selection (Shift-Click), and toggle multi-selection (Ctrl/Cmd-Click);
//! - Select All, Clear Selection, Invert Selection;
//! - Batch action payload extraction and operations over 10,000+ items.

use std::collections::BTreeSet;

/// Selection interaction mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// Selection disabled.
    None,
    /// Single item selection only.
    Single,
    /// Multi-selection enabled with range anchor support.
    #[default]
    Multiple,
}

/// Selection state tracking selected indices and range anchors.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SelectionState {
    mode: SelectionMode,
    selected: BTreeSet<usize>,
    anchor: Option<usize>,
}

impl SelectionState {
    /// Create a new selection state in the given mode.
    pub fn new(mode: SelectionMode) -> Self {
        Self {
            mode,
            selected: BTreeSet::new(),
            anchor: None,
        }
    }

    /// Set selection mode.
    pub fn set_mode(&mut self, mode: SelectionMode) {
        self.mode = mode;
        if mode == SelectionMode::Single && self.selected.len() > 1 {
            let first = self.selected.iter().next().copied();
            self.selected.clear();
            if let Some(idx) = first {
                self.selected.insert(idx);
            }
        } else if mode == SelectionMode::None {
            self.clear();
        }
    }

    /// Current selection mode.
    pub fn mode(&self) -> SelectionMode {
        self.mode
    }

    /// Number of selected items.
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    /// Whether no items are selected.
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    /// Whether item at `index` is currently selected.
    pub fn contains(&self, index: usize) -> bool {
        self.selected.contains(&index)
    }

    /// Primary / single selected index if exactly one item is selected.
    pub fn single_selected(&self) -> Option<usize> {
        if self.selected.len() == 1 {
            self.selected.iter().next().copied()
        } else {
            None
        }
    }

    /// All selected indices in ascending order.
    pub fn to_vec(&self) -> Vec<usize> {
        self.selected.iter().copied().collect()
    }

    /// Iterator over selected indices.
    pub fn iter(&self) -> impl Iterator<Item = &usize> {
        self.selected.iter()
    }

    /// Range anchor index used for Shift-Click range selection.
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    // --- Mutations ---

    /// Clear all selected items.
    pub fn clear(&mut self) -> bool {
        let changed = !self.selected.is_empty();
        self.selected.clear();
        self.anchor = None;
        changed
    }

    /// Select a single item, replacing previous selection.
    pub fn select_single(&mut self, index: usize) -> bool {
        if self.mode == SelectionMode::None {
            return false;
        }
        if self.selected.len() == 1 && self.selected.contains(&index) {
            return false;
        }
        self.selected.clear();
        self.selected.insert(index);
        self.anchor = Some(index);
        true
    }

    /// Toggle selection state of an item (Ctrl/Cmd-Click behavior).
    pub fn toggle(&mut self, index: usize) -> bool {
        match self.mode {
            SelectionMode::None => false,
            SelectionMode::Single => {
                if self.selected.contains(&index) {
                    self.selected.clear();
                    self.anchor = None;
                } else {
                    self.selected.clear();
                    self.selected.insert(index);
                    self.anchor = Some(index);
                }
                true
            }
            SelectionMode::Multiple => {
                if !self.selected.remove(&index) {
                    self.selected.insert(index);
                }
                self.anchor = Some(index);
                true
            }
        }
    }

    /// Select a contiguous range of items `[from, to]` inclusive (Shift-Click behavior).
    pub fn select_range(&mut self, from: usize, to: usize) -> bool {
        if self.mode != SelectionMode::Multiple {
            return self.select_single(to);
        }
        let start = from.min(to);
        let end = from.max(to);
        let mut changed = false;
        for idx in start..=end {
            if self.selected.insert(idx) {
                changed = true;
            }
        }
        self.anchor = Some(from);
        changed
    }

    /// Handle a click with modifier flags:
    /// - `shift = true`: range selection from anchor to `index`;
    /// - `ctrl = true`: toggle `index` without clearing others;
    /// - default: select single `index`.
    pub fn handle_click(&mut self, index: usize, shift: bool, ctrl: bool) -> bool {
        if shift && self.mode == SelectionMode::Multiple {
            let anchor = self.anchor.unwrap_or(index);
            self.select_range(anchor, index)
        } else if ctrl && self.mode == SelectionMode::Multiple {
            self.toggle(index)
        } else {
            self.select_single(index)
        }
    }

    /// Select all `total_count` items in multiple selection mode.
    pub fn select_all(&mut self, total_count: usize) -> bool {
        if self.mode != SelectionMode::Multiple || total_count == 0 {
            return false;
        }
        if self.selected.len() == total_count {
            return false;
        }
        self.selected.clear();
        for i in 0..total_count {
            self.selected.insert(i);
        }
        true
    }

    /// Invert current selection against `total_count` items.
    pub fn invert(&mut self, total_count: usize) -> bool {
        if self.mode != SelectionMode::Multiple {
            return false;
        }
        let mut new_set = BTreeSet::new();
        for i in 0..total_count {
            if !self.selected.contains(&i) {
                new_set.insert(i);
            }
        }
        self.selected = new_set;
        true
    }

    /// Filter a slice of items to only those currently selected.
    pub fn filter_selected<'a, T>(&self, items: &'a [T]) -> Vec<&'a T> {
        self.selected
            .iter()
            .filter_map(|&idx| items.get(idx))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_selection_behavior() {
        let mut state = SelectionState::new(SelectionMode::Single);
        assert!(state.is_empty());

        assert!(state.select_single(5));
        assert_eq!(state.len(), 1);
        assert_eq!(state.single_selected(), Some(5));

        // Selecting another replaces
        assert!(state.select_single(8));
        assert_eq!(state.len(), 1);
        assert_eq!(state.single_selected(), Some(8));
        assert!(!state.contains(5));
        assert!(state.contains(8));
    }

    #[test]
    fn multi_selection_toggle_and_range() {
        let mut state = SelectionState::new(SelectionMode::Multiple);

        // Click item 2
        state.handle_click(2, false, false);
        assert_eq!(state.to_vec(), vec![2]);

        // Shift-click item 6 (select range 2..=6)
        state.handle_click(6, true, false);
        assert_eq!(state.to_vec(), vec![2, 3, 4, 5, 6]);

        // Ctrl-click item 4 (toggle off item 4)
        state.handle_click(4, false, true);
        assert_eq!(state.to_vec(), vec![2, 3, 5, 6]);

        // Select all of 10 items
        state.select_all(10);
        assert_eq!(state.len(), 10);

        // Invert against 10 items (now 0)
        state.invert(10);
        assert_eq!(state.len(), 0);
    }
}
