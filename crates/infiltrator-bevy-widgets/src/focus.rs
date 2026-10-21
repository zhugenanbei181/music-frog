//! Focus ring styling and the focus-trap / grid navigation helpers.
//!
//! The focus *itself* is not owned here: the official
//! `bevy_input_focus::InputFocus` is the single focus truth, and the official
//! directional and tab navigation own the movement. This module owns the
//! product visuals (the focus ring scene and its style) and the two focus
//! helpers that have no official counterpart yet (the grid navigator and the
//! modal focus trap).

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::resource::Resource;

/// Marker component for the currently focused entity.
///
/// The shell stamps it on the officially focused control; it is a marker, not
/// a second focus truth — the official `InputFocus` says which control holds
/// the focus.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focused;

/// Component on focus ring container describing outline geometry and active styling.
/// The widths are the baseline-pixel values: the ring itself is stamped
/// through the rem channel, so it grows with the root font size.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct FocusRingStyle {
    pub width_px: f32,
    pub offset_px: f32,
}

use crate::palette::UiPalette;
use crate::palette::rem;
use crate::theme::space;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{BorderRadius, Node, PositionType, UiRect, percent};

/// Construct an explicit focus outline box scene.
pub fn focus_ring_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let ring = palette.focus_ring;
    Box::new(bsn! {
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                border: UiRect::all(rem(space::S2)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px + 2.0)),
            }
            BorderColor { top: ring, right: ring, bottom: ring, left: ring }
            FocusRingStyle { width_px: 2.0, offset_px: 2.0 }
    })
}

/// 2D grid focus navigator for table cells, proxy node matrices, and rule lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridFocusNavigator {
    pub rows: usize,
    pub cols: usize,
    pub current_row: usize,
    pub current_col: usize,
    pub wrap_around: bool,
}

impl GridFocusNavigator {
    pub fn new(rows: usize, cols: usize, wrap_around: bool) -> Self {
        Self {
            rows: rows.max(1),
            cols: cols.max(1),
            current_row: 0,
            current_col: 0,
            wrap_around,
        }
    }

    /// Linear 1D index into a flat item collection.
    pub fn current_index(&self) -> usize {
        self.current_row * self.cols + self.current_col
    }

    /// Move focus up by one row.
    pub fn move_up(&mut self) -> bool {
        if self.current_row > 0 {
            self.current_row -= 1;
            true
        } else if self.wrap_around && self.rows > 1 {
            self.current_row = self.rows - 1;
            true
        } else {
            false
        }
    }

    /// Move focus down by one row.
    pub fn move_down(&mut self) -> bool {
        if self.current_row + 1 < self.rows {
            self.current_row += 1;
            true
        } else if self.wrap_around && self.rows > 1 {
            self.current_row = 0;
            true
        } else {
            false
        }
    }

    /// Move focus left by one column.
    pub fn move_left(&mut self) -> bool {
        if self.current_col > 0 {
            self.current_col -= 1;
            true
        } else if self.wrap_around && self.cols > 1 {
            self.current_col = self.cols - 1;
            true
        } else {
            false
        }
    }

    /// Move focus right by one column.
    pub fn move_right(&mut self) -> bool {
        if self.current_col + 1 < self.cols {
            self.current_col += 1;
            true
        } else if self.wrap_around && self.cols > 1 {
            self.current_col = 0;
            true
        } else {
            false
        }
    }
}

/// The stage a [`FocusTrapManager`] is in.
///
/// The trap is either released or engaged around one root. The old design kept
/// a boolean and the root in two fields, so "trapped without a root" was
/// representable and only the setters kept the two apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FocusTrapStage {
    /// No trap is engaged; focus moves freely.
    #[default]
    Released,
    /// A trap is engaged around a modal root.
    Trapped {
        /// The entity the trap is rooted at.
        root: Entity,
    },
}

/// Typed refusal of a focus trap move the current stage forbids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusTrapTransitionError {
    /// The stage that was already running when the move was refused.
    pub from: FocusTrapStage,
}

/// Modal dialog focus trap manager enforcing WAI-ARIA accessibility constraints.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusTrapManager {
    /// The single stage the trap is in.
    pub stage: FocusTrapStage,
    pub focusable_entities: Vec<Entity>,
    pub current_focus_idx: usize,
    pub restored_focus_target: Option<Entity>,
}

impl FocusTrapManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// The stage the trap is in.
    pub fn stage(&self) -> &FocusTrapStage {
        &self.stage
    }

    /// Whether a trap is engaged.
    pub fn is_trapped(&self) -> bool {
        matches!(self.stage, FocusTrapStage::Trapped { .. })
    }

    /// The entity the trap is rooted at, if a trap is engaged.
    pub fn trap_root(&self) -> Option<Entity> {
        match self.stage {
            FocusTrapStage::Released => None,
            FocusTrapStage::Trapped { root } => Some(root),
        }
    }

    /// Engage focus trap for a modal dialog, remembering previous focus target.
    ///
    /// Engaging while a trap is already engaged is refused: the running stage
    /// comes back in [`FocusTrapTransitionError::from`], so one manager never
    /// traps two modals at once.
    pub fn engage(
        &mut self,
        root: Entity,
        focusables: Vec<Entity>,
        prev_focus: Option<Entity>,
    ) -> Result<(), FocusTrapTransitionError> {
        if let FocusTrapStage::Trapped { .. } = self.stage {
            return Err(FocusTrapTransitionError { from: self.stage });
        }
        self.stage = FocusTrapStage::Trapped { root };
        self.focusable_entities = focusables;
        self.current_focus_idx = 0;
        self.restored_focus_target = prev_focus;
        Ok(())
    }

    /// Advance to next focusable entity within the trap, wrapping to beginning.
    ///
    /// A released trap has no focusables to cycle, so the move is refused.
    pub fn cycle_next(&mut self) -> Option<Entity> {
        if !self.is_trapped() || self.focusable_entities.is_empty() {
            return None;
        }
        self.current_focus_idx = (self.current_focus_idx + 1) % self.focusable_entities.len();
        Some(self.focusable_entities[self.current_focus_idx])
    }

    /// Step to previous focusable entity within the trap, wrapping to end.
    ///
    /// A released trap has no focusables to cycle, so the move is refused.
    pub fn cycle_prev(&mut self) -> Option<Entity> {
        if !self.is_trapped() || self.focusable_entities.is_empty() {
            return None;
        }
        if self.current_focus_idx == 0 {
            self.current_focus_idx = self.focusable_entities.len() - 1;
        } else {
            self.current_focus_idx -= 1;
        }
        Some(self.focusable_entities[self.current_focus_idx])
    }

    /// Release focus trap, returning the entity that should restore focus.
    pub fn release(&mut self) -> Option<Entity> {
        self.stage = FocusTrapStage::Released;
        self.focusable_entities.clear();
        self.current_focus_idx = 0;
        self.restored_focus_target.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_focus_navigator_2d_movement_and_wrapping() {
        let mut grid = GridFocusNavigator::new(3, 4, true);
        assert_eq!(grid.current_index(), 0);

        // Move right across column
        assert!(grid.move_right());
        assert_eq!(grid.current_col, 1);
        assert_eq!(grid.current_index(), 1);

        // Move down across row
        assert!(grid.move_down());
        assert_eq!(grid.current_row, 1);
        assert_eq!(grid.current_index(), 5);

        // Wrap around right edge: from col=1, move right 3 times -> wrap to 0
        grid.move_right(); // 2
        grid.move_right(); // 3
        grid.move_right(); // wrap -> 0
        assert_eq!(grid.current_col, 0);

        // Wrap around top edge
        grid.current_row = 0;
        assert!(grid.move_up());
        assert_eq!(grid.current_row, 2);
    }
    #[test]
    fn test_focus_trap_cycling_and_restoration() {
        let mut trap = FocusTrapManager::new();
        let e_bg = Entity::from_raw_u32(1).unwrap();
        let e1 = Entity::from_raw_u32(2).unwrap();
        let e2 = Entity::from_raw_u32(3).unwrap();
        let root = Entity::from_raw_u32(4).unwrap();

        trap.engage(root, vec![e1, e2], Some(e_bg))
            .expect("released trap engages");
        assert!(trap.is_trapped());
        assert_eq!(trap.trap_root(), Some(root));
        assert_eq!(trap.focusable_entities.len(), 2);

        // Next wraps around [e1 -> e2 -> e1]
        assert_eq!(trap.cycle_next(), Some(e2));
        assert_eq!(trap.cycle_next(), Some(e1));

        // Prev wraps around
        assert_eq!(trap.cycle_prev(), Some(e2));

        // Release restores previous background target
        let restored = trap.release();
        assert!(!trap.is_trapped());
        assert_eq!(trap.trap_root(), None);
        assert_eq!(restored, Some(e_bg));
    }
}
