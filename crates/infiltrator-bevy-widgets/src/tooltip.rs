//! Tooltip: floating informative text bubble anchored to a target element.
use bevy::ecs::hierarchy::Children;

use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, JustifyContent, Node, PositionType, UiRect, Val, px,
};
use bevy::ui::widget::Text;

use crate::palette::UiPalette;
use crate::popover::Rect;
use crate::text::{Role, TextRole};
use crate::theme::space;

/// Preferred anchor direction for tooltip popover.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TooltipPosition {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Pure state of a tooltip.
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipState {
    pub text: String,
    pub position: TooltipPosition,
    pub is_visible: bool,
}

impl TooltipState {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            position: TooltipPosition::Top,
            is_visible: true,
        }
    }

    pub fn with_position(mut self, position: TooltipPosition) -> Self {
        self.position = position;
        self
    }
}

/// Compute optimal tooltip rectangle with viewport bounds clamping. Pure function.
pub fn compute_tooltip_rect(
    target: Rect,
    tooltip_size: (f32, f32),
    viewport: Rect,
    preferred: TooltipPosition,
    gap: f32,
) -> Rect {
    let (tw, th) = tooltip_size;
    let (mut x, mut y) = match preferred {
        TooltipPosition::Top => {
            let cx = target.x + (target.w - tw) * 0.5;
            let top_y = target.y - gap - th;
            (cx, top_y)
        }
        TooltipPosition::Bottom => {
            let cx = target.x + (target.w - tw) * 0.5;
            let bot_y = target.bottom() + gap;
            (cx, bot_y)
        }
        TooltipPosition::Left => {
            let left_x = target.x - gap - tw;
            let cy = target.y + (target.h - th) * 0.5;
            (left_x, cy)
        }
        TooltipPosition::Right => {
            let right_x = target.right() + gap;
            let cy = target.y + (target.h - th) * 0.5;
            (right_x, cy)
        }
    };

    // Clamp inside viewport
    x = x.clamp(viewport.x, (viewport.right() - tw).max(viewport.x));
    y = y.clamp(viewport.y, (viewport.bottom() - th).max(viewport.y));

    Rect { x, y, w: tw, h: th }
}

/// Marker component on tooltip floating container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TooltipRoot;

/// Marker component on tooltip bubble node.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TooltipBubble;

/// Declarative scene constructor for positioned tooltip bubble.
pub fn tooltip_scene(
    text: String,
    target: Rect,
    viewport: Rect,
    position: TooltipPosition,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let font_size = palette.caption_font_px;
    let estimated_w =
        (text.chars().count() as f32 * font_size * 0.65 + space::S16).clamp(60.0, 320.0);
    let estimated_h = palette.control_height_px * 0.75;
    let rect = compute_tooltip_rect(
        target,
        (estimated_w, estimated_h),
        viewport,
        position,
        space::S8,
    );
    let edge = palette.border;

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            left: px(rect.x),
            top: px(rect.y),
            padding: UiRect::new(Val::Px(space::S8), Val::Px(space::S8), Val::Px(space::S4), Val::Px(space::S4)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px * 0.75)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        TooltipBubble
        Children [
            ( Text(text) TextRole(Role::Caption) ),
        ]
    }
}

/// Repaint tooltip bubbles from live palette.
pub fn sync_tooltip_visuals(
    palette: Res<UiPalette>,
    mut bubbles: Query<(&mut BackgroundColor, &mut BorderColor), With<TooltipBubble>>,
) {
    let bg = palette.surface_elevated;
    let edge = palette.border;

    for (mut fill, mut border) in &mut bubbles {
        if fill.0 != bg {
            fill.0 = bg;
        }
        if border.top != edge {
            border.set_all(edge);
        }
    }
}
