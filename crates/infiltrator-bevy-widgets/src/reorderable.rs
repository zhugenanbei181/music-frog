//! Reorderable list state machine supporting drag-and-drop and keyboard reordering.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure state machine enabling users to reorder proxy node priority chains, rules, and profiles.

use bevy::ecs::resource::Resource;

/// Action requesting a reordering movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderAction {
    MoveUp(usize),
    MoveDown(usize),
    DragMove { from: usize, to: usize },
}

/// Pure state machine managing an ordered collection of item identifiers.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ReorderableListState {
    pub items: Vec<String>,
    pub active_drag_index: Option<usize>,
    pub target_drop_index: Option<usize>,
}

impl ReorderableListState {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            active_drag_index: None,
            target_drop_index: None,
        }
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// Move an item from `from_index` to `to_index`. Returns true if moved.
    pub fn move_item(&mut self, from_index: usize, to_index: usize) -> bool {
        if from_index >= self.items.len() || to_index >= self.items.len() || from_index == to_index
        {
            return false;
        }
        let item = self.items.remove(from_index);
        self.items.insert(to_index, item);
        true
    }

    /// Move item up one position (keyboard Alt+Up).
    pub fn move_up(&mut self, index: usize) -> bool {
        if index > 0 && index < self.items.len() {
            self.items.swap(index, index - 1);
            true
        } else {
            false
        }
    }

    /// Move item down one position (keyboard Alt+Down).
    pub fn move_down(&mut self, index: usize) -> bool {
        if index + 1 < self.items.len() {
            self.items.swap(index, index + 1);
            true
        } else {
            false
        }
    }

    /// Execute a generic reorder action.
    pub fn apply_action(&mut self, action: ReorderAction) -> bool {
        match action {
            ReorderAction::MoveUp(idx) => self.move_up(idx),
            ReorderAction::MoveDown(idx) => self.move_down(idx),
            ReorderAction::DragMove { from, to } => self.move_item(from, to),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reorderable_list_move_up_and_down() {
        let mut list = ReorderableListState::new(vec![
            "Node1".to_string(),
            "Node2".to_string(),
            "Node3".to_string(),
        ]);

        // Move Node2 up -> [Node2, Node1, Node3]
        assert!(list.move_up(1));
        assert_eq!(list.items, vec!["Node2", "Node1", "Node3"]);

        // Move top item up -> fails, no-op
        assert!(!list.move_up(0));

        // Move Node2 down -> [Node1, Node2, Node3]
        assert!(list.move_down(0));
        assert_eq!(list.items, vec!["Node1", "Node2", "Node3"]);
    }

    #[test]
    fn test_reorderable_list_drag_move() {
        let mut list = ReorderableListState::new(vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
            "D".to_string(),
        ]);

        // Drag A (index 0) to index 2 -> [B, C, A, D]
        assert!(list.apply_action(ReorderAction::DragMove { from: 0, to: 2 }));
        assert_eq!(list.items, vec!["B", "C", "A", "D"]);

        // Out of bounds drag fails
        assert!(!list.apply_action(ReorderAction::DragMove { from: 10, to: 0 }));
    }
}
