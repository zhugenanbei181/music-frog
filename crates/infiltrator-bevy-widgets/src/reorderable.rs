//! Reorderable list state machine supporting drag-and-drop and keyboard reordering.
//!
//! Charter (crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md):
//! Pure state machine enabling users to reorder proxy node priority chains, rules, and profiles.

use bevy::ecs::resource::Resource;

/// Action requesting a reordering movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderAction {
    MoveUp(usize),
    MoveDown(usize),
    DragMove { from: usize, to: usize },
}

/// The drag stage of a [`ReorderableListState`].
///
/// The old design kept the dragged item and the drop target in two separate
/// options, so "a drop target without a dragged item" was representable and
/// nothing kept the pair in step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReorderDragStage {
    /// No item is being dragged.
    #[default]
    Idle,
    /// One item is being dragged towards a drop position.
    Dragging {
        /// Index of the dragged item.
        item_index: usize,
        /// Index the item would land on, once a drop position is known.
        target_drop_index: Option<usize>,
    },
}

/// Typed refusal of a drag move the current stage forbids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReorderTransitionError {
    /// The stage that was already running when the move was refused.
    pub from: ReorderDragStage,
}

/// Pure state machine managing an ordered collection of item identifiers.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ReorderableListState {
    pub items: Vec<String>,
    /// The single drag stage the list is in.
    pub drag: ReorderDragStage,
}

impl ReorderableListState {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            drag: ReorderDragStage::Idle,
        }
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
    }

    /// The drag stage the list is in.
    pub fn drag_stage(&self) -> ReorderDragStage {
        self.drag
    }

    /// The item being dragged, if any.
    pub fn dragged_item_index(&self) -> Option<usize> {
        match self.drag {
            ReorderDragStage::Idle => None,
            ReorderDragStage::Dragging { item_index, .. } => Some(item_index),
        }
    }

    /// The drop position the dragged item would land on, if one is known.
    pub fn target_drop_index(&self) -> Option<usize> {
        match self.drag {
            ReorderDragStage::Idle => None,
            ReorderDragStage::Dragging {
                target_drop_index, ..
            } => target_drop_index,
        }
    }

    /// Begin dragging an item.
    ///
    /// Starting a second drag while one is running is refused: the running
    /// stage comes back in [`ReorderTransitionError::from`].
    pub fn start_drag(&mut self, item_index: usize) -> Result<(), ReorderTransitionError> {
        if let ReorderDragStage::Dragging { .. } = self.drag {
            return Err(ReorderTransitionError { from: self.drag });
        }
        self.drag = ReorderDragStage::Dragging {
            item_index,
            target_drop_index: None,
        };
        Ok(())
    }

    /// Move the drop position of the running drag.
    ///
    /// Refused when no drag is running: a drop position without a dragged item
    /// is not a state the list can hold.
    pub fn set_drop_target(
        &mut self,
        target_drop_index: usize,
    ) -> Result<(), ReorderTransitionError> {
        let ReorderDragStage::Dragging { item_index, .. } = self.drag else {
            return Err(ReorderTransitionError { from: self.drag });
        };
        self.drag = ReorderDragStage::Dragging {
            item_index,
            target_drop_index: Some(target_drop_index),
        };
        Ok(())
    }

    /// End the running drag, returning the drop position it reached.
    pub fn end_drag(&mut self) -> Option<usize> {
        let ReorderDragStage::Dragging {
            target_drop_index, ..
        } = self.drag
        else {
            return None;
        };
        self.drag = ReorderDragStage::Idle;
        target_drop_index
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

    #[test]
    fn test_reorderable_list_drag_stage_is_typed() {
        let mut list = ReorderableListState::new(vec![
            "A".to_string(),
            "B".to_string(),
            "C".to_string(),
        ]);

        assert_eq!(list.drag_stage(), ReorderDragStage::Idle);
        assert_eq!(list.dragged_item_index(), None);
        assert_eq!(list.target_drop_index(), None);

        // A drop position without a running drag is refused.
        assert!(
            list.set_drop_target(2).is_err(),
            "a drop position needs a dragged item"
        );

        list.start_drag(0).expect("idle list starts a drag");
        assert_eq!(list.dragged_item_index(), Some(0));
        assert_eq!(list.target_drop_index(), None);

        // A second drag while one runs is refused.
        assert!(list.start_drag(1).is_err());

        list.set_drop_target(2).expect("running drag takes a drop position");
        assert_eq!(list.target_drop_index(), Some(2));

        assert_eq!(list.end_drag(), Some(2));
        assert_eq!(list.drag_stage(), ReorderDragStage::Idle);
        assert_eq!(list.dragged_item_index(), None);
        assert_eq!(list.target_drop_index(), None);
    }
}
