//! Right-click context menu system with viewport boundary flipping and action dispatch.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Pure state machine with analytical edge clamping and declarative `bsn!` overlay panel.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, PositionType,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;

use crate::icon::IconId;
use crate::icon_tile::icon_tile_scene;
use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// An individual action entry inside a context menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextMenuItem {
    pub id: String,
    pub label: String,
    pub icon: Option<IconId>,
    pub is_destructive: bool,
    pub is_disabled: bool,
}

impl ContextMenuItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            is_destructive: false,
            is_disabled: false,
        }
    }

    pub fn with_icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn destructive(mut self) -> Self {
        self.is_destructive = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.is_disabled = true;
        self
    }
}

/// Computes clamped and flipped 2D screen placement for a context menu.
pub struct ContextMenuPlacement;

impl ContextMenuPlacement {
    /// Calculate top-left origin (x, y) ensuring the menu never overflows the viewport boundaries.
    pub fn calculate_origin(cursor: Vec2, menu_size: Vec2, viewport_size: Vec2) -> Vec2 {
        let mut x = cursor.x;
        let mut y = cursor.y;

        // Horizontal overflow: flip to the left of cursor
        if x + menu_size.x > viewport_size.x {
            x = (cursor.x - menu_size.x).max(0.0);
        }

        // Vertical overflow: flip upwards above cursor
        if y + menu_size.y > viewport_size.y {
            y = (cursor.y - menu_size.y).max(0.0);
        }

        Vec2::new(x, y)
    }
}

/// Pure state of the active context menu.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ContextMenuState {
    pub is_open: bool,
    pub position: Vec2,
    pub items: Vec<ContextMenuItem>,
}

impl ContextMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open context menu at cursor position with boundary flipping.
    pub fn open_at(
        &mut self,
        cursor: Vec2,
        items: Vec<ContextMenuItem>,
        menu_size: Vec2,
        viewport_size: Vec2,
    ) {
        self.position = ContextMenuPlacement::calculate_origin(cursor, menu_size, viewport_size);
        self.items = items;
        self.is_open = true;
    }

    /// Close the context menu.
    pub fn close(&mut self) {
        self.is_open = false;
        self.items.clear();
    }
}

/// Marker component on context menu root overlay.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextMenuRoot;

/// Marker component on context menu card container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ContextMenuCard;

/// Construct a declarative context menu overlay scene.
pub fn context_menu_scene(palette: &UiPalette, state: &ContextMenuState) -> Box<dyn Scene> {
    let edge = palette.border;
    let pos_x = state.position.x;
    let pos_y = state.position.y;

    let item_scenes: Vec<Box<dyn Scene>> = state
        .items
        .iter()
        .map(|item| {
            let label = item.label.clone();
            let text_color = if item.is_destructive {
                palette.danger
            } else {
                Color::NONE
            };
            let _ = text_color;

            let icon_node: Option<Box<dyn Scene>> = item
                .icon
                .map(|ic| Box::new(icon_tile_scene(ic, 16.0, palette)) as Box<dyn Scene>);

            Box::new(bsn! {
                Node {
                    width: percent(100),
                    height: px(32.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::FlexStart,
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    column_gap: Val::Px(space::S8),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                Button
                Children [
                    { icon_node.into_iter().collect::<Vec<_>>() },
                    (
                        Text({ label })
                        TextRole(Role::Body)
                    ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    Box::new(bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
        }
        ContextMenuRoot
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(pos_x),
                    top: px(pos_y),
                    width: px(180.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S4)),
                    row_gap: Val::Px(2.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                }
                BackgroundColor({ palette.surface_elevated })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                ContextMenuCard
                Children [
                    { item_scenes },
                ]
            ),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_menu_placement_flipping() {
        let viewport = Vec2::new(1000.0, 800.0);
        let menu_size = Vec2::new(180.0, 200.0);

        // 1. Normal interior point: no flipping
        let pos1 =
            ContextMenuPlacement::calculate_origin(Vec2::new(200.0, 300.0), menu_size, viewport);
        assert_eq!(pos1, Vec2::new(200.0, 300.0));

        // 2. Near right edge: flips left
        let pos2 =
            ContextMenuPlacement::calculate_origin(Vec2::new(900.0, 300.0), menu_size, viewport);
        assert_eq!(pos2, Vec2::new(900.0 - 180.0, 300.0));

        // 3. Near bottom edge: flips upwards
        let pos3 =
            ContextMenuPlacement::calculate_origin(Vec2::new(200.0, 750.0), menu_size, viewport);
        assert_eq!(pos3, Vec2::new(200.0, 750.0 - 200.0));

        // 4. Bottom-right corner: flips both X and Y
        let pos4 =
            ContextMenuPlacement::calculate_origin(Vec2::new(950.0, 780.0), menu_size, viewport);
        assert_eq!(pos4, Vec2::new(950.0 - 180.0, 780.0 - 200.0));
    }

    #[test]
    fn test_context_menu_state_lifecycle() {
        let mut state = ContextMenuState::new();
        assert!(!state.is_open);

        let items = vec![
            ContextMenuItem::new("ping", "测速此节点"),
            ContextMenuItem::new("delete", "删除节点").destructive(),
        ];

        state.open_at(
            Vec2::new(100.0, 100.0),
            items,
            Vec2::new(180.0, 100.0),
            Vec2::new(800.0, 600.0),
        );
        assert!(state.is_open);
        assert_eq!(state.items.len(), 2);
        assert!(state.items[1].is_destructive);

        state.close();
        assert!(!state.is_open);
        assert!(state.items.is_empty());
    }
}
