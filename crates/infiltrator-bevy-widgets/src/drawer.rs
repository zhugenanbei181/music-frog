//! Drawer: edge-docked sliding sheet / navigation drawer over a dark scrim.
//!
//! **Pure Geometry Core**: [`DrawerPlacement`] (Left, Right, Top, Bottom) and
//! [`drawer_rect`] compute the panel's exact bounding box and slide trajectory
//! relative to the viewport. Headless-testable with zero Bevy dependency.
//!
//! **Scene Adapter**: [`drawer_scene`] builds a declarative full-viewport scrim
//! hosting the edge-docked panel at token borders and surface fills.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::message::Message;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, Node, PositionType, UiRect, Val,
    percent, px,
};

use crate::palette::UiPalette;
use crate::popover::Rect;
use crate::theme::space;

/// Docking edge for the drawer panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrawerPlacement {
    /// Left edge (e.g. mobile navigation drawer).
    #[default]
    Left,
    /// Right edge (e.g. inspector/filter sidebar).
    Right,
    /// Top edge (e.g. banner notification sheet).
    Top,
    /// Bottom edge (e.g. bottom sheet / action tray).
    Bottom,
}

/// Pure state of a drawer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawerState {
    pub placement: DrawerPlacement,
    pub size_px: f32,
    pub is_open: bool,
    pub slide_progress: f32,
}

impl DrawerState {
    pub fn new(placement: DrawerPlacement, size_px: f32) -> Self {
        Self {
            placement,
            size_px,
            is_open: true,
            slide_progress: 1.0,
        }
    }
}

/// Compute absolute bounding rectangle for a drawer panel during slide animation.
/// `open_ratio` is clamped to `0.0..=1.0`. Pure function.
pub fn drawer_rect(
    placement: DrawerPlacement,
    size_px: f32,
    viewport: Rect,
    open_ratio: f32,
) -> Rect {
    let ratio = open_ratio.clamp(0.0, 1.0);
    match placement {
        DrawerPlacement::Left => {
            let w = size_px.min(viewport.w);
            let x = viewport.x - w * (1.0 - ratio);
            Rect {
                x,
                y: viewport.y,
                w,
                h: viewport.h,
            }
        }
        DrawerPlacement::Right => {
            let w = size_px.min(viewport.w);
            let x = viewport.right() - w * ratio;
            Rect {
                x,
                y: viewport.y,
                w,
                h: viewport.h,
            }
        }
        DrawerPlacement::Top => {
            let h = size_px.min(viewport.h);
            let y = viewport.y - h * (1.0 - ratio);
            Rect {
                x: viewport.x,
                y,
                w: viewport.w,
                h,
            }
        }
        DrawerPlacement::Bottom => {
            let h = size_px.min(viewport.h);
            let y = viewport.bottom() - h * ratio;
            Rect {
                x: viewport.x,
                y,
                w: viewport.w,
                h,
            }
        }
    }
}

/// Marker on drawer full-screen scrim.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerScrim;

/// Marker on drawer docked panel.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerPanel(pub DrawerPlacement);

/// Marker on drawer close button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawerCloseButton;

/// Message to open a drawer.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawerOpenEvent(pub Entity);

/// Message to close a drawer.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawerCloseEvent(pub Entity);

/// Declarative drawer scene constructor.
pub fn drawer_scene(
    placement: DrawerPlacement,
    size_px: f32,
    content: Box<dyn Scene>,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;

    let (width_val, height_val, left_val, top_val, right_val, bottom_val, radius_val) =
        match placement {
            DrawerPlacement::Left => (
                px(size_px),
                percent(100),
                px(0.0),
                px(0.0),
                Val::Auto,
                Val::Auto,
                BorderRadius {
                    top_left: Val::Px(0.0),
                    top_right: Val::Px(palette.card_radius_px),
                    bottom_left: Val::Px(0.0),
                    bottom_right: Val::Px(palette.card_radius_px),
                },
            ),
            DrawerPlacement::Right => (
                px(size_px),
                percent(100),
                Val::Auto,
                px(0.0),
                px(0.0),
                Val::Auto,
                BorderRadius {
                    top_left: Val::Px(palette.card_radius_px),
                    top_right: Val::Px(0.0),
                    bottom_left: Val::Px(palette.card_radius_px),
                    bottom_right: Val::Px(0.0),
                },
            ),
            DrawerPlacement::Top => (
                percent(100),
                px(size_px),
                px(0.0),
                px(0.0),
                Val::Auto,
                Val::Auto,
                BorderRadius {
                    top_left: Val::Px(0.0),
                    top_right: Val::Px(0.0),
                    bottom_left: Val::Px(palette.card_radius_px),
                    bottom_right: Val::Px(palette.card_radius_px),
                },
            ),
            DrawerPlacement::Bottom => (
                percent(100),
                px(size_px),
                px(0.0),
                Val::Auto,
                Val::Auto,
                px(0.0),
                BorderRadius {
                    top_left: Val::Px(palette.card_radius_px),
                    top_right: Val::Px(palette.card_radius_px),
                    bottom_left: Val::Px(0.0),
                    bottom_right: Val::Px(0.0),
                },
            ),
        };

    bsn! {
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            align_items: AlignItems::FlexStart,
        }
        BackgroundColor({ palette.scrim() })
        DrawerScrim
        Children [
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: left_val,
                    top: top_val,
                    right: right_val,
                    bottom: bottom_val,
                    width: width_val,
                    height: height_val,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S16)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: radius_val,
                }
                BackgroundColor({ palette.surface })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                DrawerPanel(placement)
                Children [
                    ( { content } ),
                ]
            ),
        ]
    }
}

/// Repaint drawer panels and scrims from live palette.
#[allow(clippy::type_complexity)]
pub fn sync_drawer_visuals(
    palette: Res<UiPalette>,
    mut scrims: Query<&mut BackgroundColor, (With<DrawerScrim>, Without<DrawerPanel>)>,
    mut panels: Query<
        (&mut BackgroundColor, &mut BorderColor),
        (With<DrawerPanel>, Without<DrawerScrim>),
    >,
) {
    let scrim_bg = palette.scrim();
    for mut fill in &mut scrims {
        if fill.0 != scrim_bg {
            fill.0 = scrim_bg;
        }
    }

    let edge = palette.border;
    for (mut fill, mut border) in &mut panels {
        if fill.0 != palette.surface {
            fill.0 = palette.surface;
        }
        if border.top != edge {
            border.set_all(edge);
        }
    }
}
