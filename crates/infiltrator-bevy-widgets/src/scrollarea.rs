//! Scroll area: the official unstyled `bevy_ui_widgets` [`ScrollArea`]
//! behavior wrapped in token container chrome. The official widget owns the
//! wheel/trackpad scroll semantics (driven by pointer scroll events on the
//! windowed composition); this module owns the clipped viewport geometry and
//! the card surface it sits on.
//!
//! [`clamp_scroll`] is the pure projection the official observer applies
//! (delta applied, then clamped into the over-scroll range); it is kept
//! here as the headless-testable statement of that contract — the viewport
//! never scrolls past its content, and short content never scrolls at all.
//!
//! **Soft keyboard avoidance**: [`calculate_focus_avoidance_scroll`] and
//! [`focus_avoidance_auto_scroll_system`] ensure focused input fields stay
//! visible when an on-screen keyboard (mobile / Android / touch) appears.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::transform::components::GlobalTransform;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    BackgroundColor, BorderRadius, ComputedNode, FlexDirection, Node, Overflow, ScrollPosition,
    UiRect, Val, percent, px,
};
use bevy::ui_widgets::ScrollArea;

use crate::palette::UiPalette;

/// Virtual / soft keyboard state (e.g. on Android / mobile / touch screens).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SoftKeyboardState {
    /// Whether the soft keyboard is currently open / visible.
    pub is_open: bool,
    /// Height of the soft keyboard in pixels.
    pub height_px: f32,
}

impl SoftKeyboardState {
    /// Keyboard open with the given height in pixels.
    pub fn open(height_px: f32) -> Self {
        Self {
            is_open: true,
            height_px,
        }
    }

    /// Keyboard closed (height = 0).
    pub fn closed() -> Self {
        Self {
            is_open: false,
            height_px: 0.0,
        }
    }
}

/// Marker component placed on the focused text input entity requiring soft keyboard avoidance.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FocusedTextInput;

/// Geometry parameters for calculating soft keyboard auto-scroll avoidance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusAvoidanceParams {
    /// Top coordinate (Y) of the scroll viewport in screen / window coordinates.
    pub viewport_top: f32,
    /// Height of the scroll viewport in pixels.
    pub viewport_height: f32,
    /// Total scrollable content height in pixels.
    pub content_height: f32,
    /// Current vertical scroll offset (0.0 means scrolled to top).
    pub current_scroll: f32,
    /// Top coordinate (Y) of the focused input widget in screen / window coordinates.
    pub target_top: f32,
    /// Height of the focused input widget in pixels.
    pub target_height: f32,
    /// Height of the soft keyboard in pixels (0.0 if closed).
    pub keyboard_height: f32,
    /// Safety comfort margin above the keyboard in pixels (e.g. 16.0 px).
    pub margin_px: f32,
}

/// Calculate the updated scroll offset so the focused input is not occluded by the soft keyboard.
/// Pure, total, clamped into `[0, max_scroll]`.
pub fn calculate_focus_avoidance_scroll(params: FocusAvoidanceParams) -> f32 {
    if !params.keyboard_height.is_finite()
        || params.keyboard_height <= 0.0
        || params.viewport_height <= 0.0
    {
        return params.current_scroll;
    }

    let visible_viewport_h = (params.viewport_height - params.keyboard_height).max(0.0);
    if visible_viewport_h <= 0.0 {
        return params.current_scroll;
    }

    let target_bottom = params.target_top + params.target_height;
    let visible_bottom = params.viewport_top + visible_viewport_h;
    let visible_top = params.viewport_top;

    let mut new_scroll = params.current_scroll;

    // If target bottom is below visible bottom (occluded or too close to keyboard)
    if target_bottom + params.margin_px > visible_bottom {
        let overflow = (target_bottom + params.margin_px) - visible_bottom;
        new_scroll += overflow;
    }
    // If target top is above visible top (scrolled too far down)
    else if params.target_top - params.margin_px < visible_top {
        let underflow = visible_top - (params.target_top - params.margin_px);
        new_scroll = (new_scroll - underflow).max(0.0);
    }

    clamp_scroll(new_scroll, params.content_height, params.viewport_height)
}

/// ECS system: automatically adjusts `ScrollPosition` on `ScrollArea` nodes
/// when a focused input field is occluded by the soft keyboard.
#[allow(clippy::type_complexity)]
pub fn focus_avoidance_auto_scroll_system(
    keyboard: Option<Res<SoftKeyboardState>>,
    focused_inputs: Query<
        (Option<&GlobalTransform>, Option<&ComputedNode>),
        With<FocusedTextInput>,
    >,
    mut scroll_areas: Query<
        (
            Option<&GlobalTransform>,
            Option<&ComputedNode>,
            &mut ScrollPosition,
            Option<&Children>,
        ),
        With<ScrollArea>,
    >,
    node_query: Query<(&ComputedNode, Option<&Children>)>,
) {
    let Some(keyboard) = keyboard else {
        return;
    };
    if !keyboard.is_open || keyboard.height_px <= 0.0 {
        return;
    }

    let Some((target_transform, target_node)) = focused_inputs.iter().next() else {
        return;
    };

    let target_size = target_node
        .map(|n| n.size())
        .unwrap_or(bevy::math::Vec2::new(200.0, 44.0));
    let target_top = target_transform
        .map(|t| t.translation().y - target_size.y * 0.5)
        .unwrap_or(0.0);

    for (scroll_transform, scroll_node, mut scroll_pos, children) in &mut scroll_areas {
        let viewport_size = scroll_node
            .map(|n| n.size())
            .unwrap_or(bevy::math::Vec2::new(300.0, 400.0));
        let viewport_top = scroll_transform
            .map(|t| t.translation().y - viewport_size.y * 0.5)
            .unwrap_or(0.0);

        let mut content_height = viewport_size.y;
        if let Some(children) = children {
            for child in children.iter() {
                if let Ok((child_node, _)) = node_query.get(*child) {
                    content_height = content_height.max(child_node.size().y);
                }
            }
        }

        let params = FocusAvoidanceParams {
            viewport_top,
            viewport_height: viewport_size.y,
            content_height,
            current_scroll: scroll_pos.0.y,
            target_top,
            target_height: target_size.y,
            keyboard_height: keyboard.height_px,
            margin_px: 16.0,
        };

        let target_scroll = calculate_focus_avoidance_scroll(params);
        if (scroll_pos.0.y - target_scroll).abs() > 0.5 {
            scroll_pos.0.y = target_scroll;
        }
    }
}

/// Clamp a scroll offset into the legal range for a viewport: at most
/// `content - viewport` (never less than zero), and zero entirely when the
/// content fits. Mirrors the official `ScrollArea` observer's clamp.
pub fn clamp_scroll(position: f32, content: f32, viewport: f32) -> f32 {
    position.clamp(0.0, (content - viewport).max(0.0))
}

/// A token card hosting one scrollable content scene. The viewport is
/// vertically scrollable and clips at the card's rounded edge; the caller
/// owns the viewport height (their layout decides) and the content scene.
pub fn scrollarea_scene(
    content: Box<dyn Scene>,
    viewport_height_px: f32,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
        }
        BackgroundColor({ palette.surface })
        BorderColor {
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        }
        Children [
            (
                Node {
                    width: percent(100),
                    height: px(viewport_height_px),
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [
                    ( { content } ),
                ]
            ),
        ]
    }
}
