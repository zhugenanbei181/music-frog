//! Multi-window management, Picture-in-Picture (PiP) floating overlays, and dockable panels.

use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

/// Docking split slot orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockSlot {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

/// A dockable workspace panel node.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct DockPanel {
    pub id: String,
    pub title: String,
    pub current_slot: DockSlot,
    pub is_floating: bool,
    pub floating_position: Vec2,
    pub floating_size: Vec2,
}

impl DockPanel {
    pub fn new(id: impl Into<String>, title: impl Into<String>, slot: DockSlot) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            current_slot: slot,
            is_floating: false,
            floating_position: Vec2::new(100.0, 100.0),
            floating_size: Vec2::new(300.0, 200.0),
        }
    }
}

/// Picture-in-Picture (PiP) floating mini-window state machine.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PipOverlayState {
    pub is_active: bool,
    pub is_pinned_top: bool,
    pub is_click_through: bool,
    pub position: Vec2,
    pub size: Vec2,
    pub opacity: f32,
}

impl Default for PipOverlayState {
    fn default() -> Self {
        Self {
            is_active: false,
            is_pinned_top: true,
            is_click_through: false,
            position: Vec2::new(20.0, 20.0),
            size: Vec2::new(180.0, 64.0),
            opacity: 0.9,
        }
    }
}

impl PipOverlayState {
    pub fn snap_to_corner(&mut self, screen_size: Vec2, top_right: bool) {
        if top_right {
            self.position = Vec2::new(screen_size.x - self.size.x - 20.0, 20.0);
        } else {
            self.position = Vec2::new(20.0, screen_size.y - self.size.y - 20.0);
        }
    }
}
