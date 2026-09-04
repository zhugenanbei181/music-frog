//! Full keyboard focus navigation, focus ring styling and directional spatial navigation.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::math::Vec2;

/// Navigation direction for spatial focus movement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDirection {
    Next,
    Previous,
    Up,
    Down,
    Left,
    Right,
}

/// Marker component for focusable UI entities.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focusable {
    pub tab_index: i32,
    pub is_enabled: bool,
}

impl Focusable {
    pub fn new(tab_index: i32) -> Self {
        Self {
            tab_index,
            is_enabled: true,
        }
    }
}

/// Marker component for the currently focused entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focused;

/// Component on focus ring container describing outline geometry and active styling.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct FocusRingStyle {
    pub width_px: f32,
    pub offset_px: f32,
}

/// Global focus manager tracking the active focus target and tab order chain.
#[derive(Resource, Clone, Debug, Default)]
pub struct FocusManager {
    pub focused_entity: Option<Entity>,
    pub focus_trap_root: Option<Entity>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_focus(&mut self, entity: Option<Entity>) {
        self.focused_entity = entity;
    }

    pub fn clear_focus(&mut self) {
        self.focused_entity = None;
    }

    pub fn trap_focus(&mut self, root: Entity) {
        self.focus_trap_root = Some(root);
    }

    pub fn release_trap(&mut self) {
        self.focus_trap_root = None;
    }
}

/// Event requesting focus transfer.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusMoveEvent(pub FocusDirection);

/// Pure calculation of the nearest neighbor in a given direction for spatial navigation.
pub fn find_spatial_neighbor(
    current_pos: Vec2,
    direction: FocusDirection,
    candidates: &[(Entity, Vec2)],
) -> Option<Entity> {
    let mut best_candidate = None;
    let mut min_score = f32::MAX;

    for &(entity, pos) in candidates {
        let delta = pos - current_pos;
        let is_in_direction = match direction {
            FocusDirection::Up => delta.y < -1.0,
            FocusDirection::Down => delta.y > 1.0,
            FocusDirection::Left => delta.x < -1.0,
            FocusDirection::Right => delta.x > 1.0,
            _ => false,
        };

        if !is_in_direction {
            continue;
        }

        // Distance metric weighted towards primary direction axis
        let (primary, secondary) = match direction {
            FocusDirection::Up | FocusDirection::Down => (delta.y.abs(), delta.x.abs()),
            FocusDirection::Left | FocusDirection::Right => (delta.x.abs(), delta.y.abs()),
            _ => (delta.length(), 0.0),
        };

        let score = primary + secondary * 2.0;
        if score < min_score {
            min_score = score;
            best_candidate = Some(entity);
        }
    }

    best_candidate
}

/// Sync visual focus rings on focused entities.
pub fn sync_focus_rings(
    focused_query: Query<Entity, With<Focused>>,
    mut manager: ResMut<FocusManager>,
) {
    manager.focused_entity = focused_query.iter().next();
}

use crate::palette::UiPalette;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{BorderRadius, Node, PositionType, UiRect, Val, percent};

/// Construct an explicit focus outline box scene.
pub fn focus_ring_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let accent = palette.accent;
    Box::new(bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            border: UiRect::all(Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px + 2.0)),
        }
        BorderColor { top: accent, right: accent, bottom: accent, left: accent }
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

/// Modal dialog focus trap manager enforcing WAI-ARIA accessibility constraints.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct FocusTrapManager {
    pub is_trapped: bool,
    pub trap_root: Option<Entity>,
    pub focusable_entities: Vec<Entity>,
    pub current_focus_idx: usize,
    pub restored_focus_target: Option<Entity>,
}

impl FocusTrapManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Engage focus trap for a modal dialog, remembering previous focus target.
    pub fn engage(&mut self, root: Entity, focusables: Vec<Entity>, prev_focus: Option<Entity>) {
        self.is_trapped = true;
        self.trap_root = Some(root);
        self.focusable_entities = focusables;
        self.current_focus_idx = 0;
        self.restored_focus_target = prev_focus;
    }

    /// Advance to next focusable entity within the trap, wrapping to beginning.
    pub fn cycle_next(&mut self) -> Option<Entity> {
        if self.focusable_entities.is_empty() {
            return None;
        }
        self.current_focus_idx = (self.current_focus_idx + 1) % self.focusable_entities.len();
        Some(self.focusable_entities[self.current_focus_idx])
    }

    /// Step to previous focusable entity within the trap, wrapping to end.
    pub fn cycle_prev(&mut self) -> Option<Entity> {
        if self.focusable_entities.is_empty() {
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
        self.is_trapped = false;
        self.trap_root = None;
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

        trap.engage(root, vec![e1, e2], Some(e_bg));
        assert!(trap.is_trapped);
        assert_eq!(trap.focusable_entities.len(), 2);

        // Next wraps around [e1 -> e2 -> e1]
        assert_eq!(trap.cycle_next(), Some(e2));
        assert_eq!(trap.cycle_next(), Some(e1));

        // Prev wraps around
        assert_eq!(trap.cycle_prev(), Some(e2));

        // Release restores previous background target
        let restored = trap.release();
        assert!(!trap.is_trapped);
        assert_eq!(restored, Some(e_bg));
    }
}
