//! The shell's navigation column: the route rows, their hover fill and the
//! icon-only tooltip.
//!
//! The route row is the shell's own control: the shared widget layer owns the
//! generic nav item, the shell owns the route row — the route icon, its label,
//! the hover fill that eases in under the pointer and, in the icon-only rail,
//! the tooltip that names the route.
//!
//! The tooltip is the shared widget layer's: the row's measured box is the
//! anchor, the shared owner places the bubble and clamps it to the viewport,
//! and the bubble is mounted in the shell's viewport-level overlay so it is
//! never clipped by the rail.

use crate::a11y::{nav_semantic_node, semantic_node};
use crate::app::shell_focus::ShellFocusable;
use crate::app::{NavSpacer, RailNavTooltip, SidebarExpandedOnly, SidebarNavItem};
use crate::route::Route;
use bevy::color::Alpha;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::picking::Pickable;
use bevy::scene::{Scene, bsn};
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, JustifyContent,
    Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui_widgets::{Button, ScrollArea};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::icon_scene_val;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::motion::Spring;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, NavLabel, nav_fill_token};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_bevy_widgets::tooltip::TooltipBubble;
use infiltrator_contract::a11y::ShellA11yNode;

/// The route row's height in baseline pixels.
///
/// Eleven routes have to fit the 480-pixel compact viewport without cutting
/// the last one, so the row is tighter than the shared control height.
const NAV_ROW_HEIGHT_PX: f32 = 32.0;

/// The route row's icon edge in baseline pixels.
const NAV_ROW_ICON_PX: f32 = 18.0;

/// The hover fill's spring stiffness.
const HOVER_SPRING_STIFFNESS: f32 = 260.0;

/// The hover fill's spring damping.
const HOVER_SPRING_DAMPING: f32 = 20.0;

/// The scroll feather's strip height in baseline pixels.
const NAV_FEATHER_STRIP_PX: f32 = 4.0;

/// The scroll feather's strip count.
const NAV_FEATHER_STRIPS: usize = 4;

/// The hover fill drawn under a route row.
///
/// The fill is the *animated* half of the row's hover state: the pointer's
/// position is the official binary fact, and the spring is the eased value
/// that follows it. The fill's alpha carries the progress, so the row's own
/// fill (the active route's accent) is never repainted.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct NavHoverFill(pub Spring);

impl Default for NavHoverFill {
    fn default() -> Self {
        Self(hover_spring())
    }
}

/// A fully transparent hover fill: the spring that eases the fill in.
pub fn hover_spring() -> Spring {
    Spring::new(0.0, HOVER_SPRING_STIFFNESS, HOVER_SPRING_DAMPING)
}

/// Marker on the viewport-level overlay that carries the shell's tooltips.
///
/// The overlay is a [`FixedNode`]: a tooltip is positioned against the
/// camera's viewport, not against the rail it names.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellTooltipOverlay;

/// The viewport-level overlay the shell's tooltips are mounted into.
pub fn tooltip_overlay_scene() -> impl Scene + use<> {
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                top: px(0.0),
                left: px(0.0),
                width: percent(100),
                height: percent(100),
                overflow: Overflow::visible(),
            }
            FixedNode
            ShellTooltipOverlay
    }
}

/// The hairline that separates the rail's cards from its route rows.
pub fn nav_divider_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                width: percent(100),
                height: rem(space::S2),
                flex_shrink: 0.0,
            }
            BackgroundColor({ edge })
    }
}

/// The route row: the icon, the label and the hover fill under them.
pub fn sidebar_nav_item_scene(route: Route, active: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let semantic = nav_semantic_node(&route.label(), false);
    let ink = if active {
        palette.on_accent
    } else {
        palette.ink
    };
    Box::new(bsn! {
            Node {
                width: percent(100),
                min_height: rem(NAV_ROW_HEIGHT_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::horizontal(rem(space::S12)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                position_type: PositionType::Relative,
            }
            ThemeBackgroundColor({ nav_fill_token(active) })
            Button
            SidebarNavItem(route)
            NavItem
            NavActive(active)
            ShellFocusable
            semantic LocalizedLabel::plain(route.label_key())
            Children [
                @{ nav_hover_fill_scene(palette) }
                --
                @{ icon_scene_val(route.icon(), rem(NAV_ROW_ICON_PX), ink) }
                --
                Node { width: rem(space::S8) } NavSpacer SidebarExpandedOnly
                --
                LocalizedText::plain(route.label_key())
                TextRole({
                        if active {
                            Role::BodyStrong
                        } else {
                            Role::Body
                        }
                    })
                NavLabel
                SidebarExpandedOnly
                --
                @{ rail_tooltip_scene(route, palette) }
            ]
    })
}

/// The icon-only rail's tooltip: the route's name, placed by the shared
/// tooltip owner when the row is hovered.
fn rail_tooltip_scene(route: Route, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                padding: UiRect::new(rem(space::S8), rem(space::S8), rem(space::S4), rem(space::S4)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(radius::XS)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            TooltipBubble
            RailNavTooltip(route)
            Children [
                LocalizedText::plain(route.label_key()) TextRole(Role::Caption)
            ]
    }
}

/// The hover fill: a transparent sheet under the row's content that the
/// shared motion owner eases in.
fn nav_hover_fill_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                top: px(0.0),
                left: px(0.0),
                right: px(0.0),
                bottom: px(0.0),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.hover_bg.with_alpha(0.0) })
            NavHoverFill({ hover_spring() })
    }
}

/// The navigation column: the hairline, then one row per route.
///
/// The column is the shell's scroll area: when the rail's cards and rows do
/// not fit the viewport, the rows stay reachable instead of being cut off.
/// The feather at its foot says so — the last row dissolves into the rail
/// instead of being sliced by the edge.
pub fn nav_column_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let nav_node = semantic_node(ShellA11yNode::SidebarNav);
    let nav_items: Vec<Box<dyn Scene>> = Route::ALL
        .iter()
        .map(|&route| sidebar_nav_item_scene(route, route == Route::Overview, palette))
        .collect();

    Box::new(bsn! {
            Node {
                width: percent(100),
                min_height: px(0.0),
                flex_shrink: 1.0,
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
            }
            nav_node
            Children [
                Node {
                    width: percent(100),
                    min_height: px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S4),
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [
                    @{ nav_divider_scene(palette) }
                    --
                    { nav_items }
                ]
                --
                @{ nav_scroll_feather_scene(palette) }
            ]
    })
}

/// The scroll-edge feather: the route list dissolves into the rail instead of
/// being cut by the edge.
///
/// The feather never blocks the pointer, so the wheel keeps scrolling the
/// rows underneath it.
fn nav_scroll_feather_scene(palette: &UiPalette) -> impl Scene + use<> {
    let strips: Vec<Box<dyn Scene>> = (0..NAV_FEATHER_STRIPS)
        .map(|index| {
            let alpha = (index + 1) as f32 / NAV_FEATHER_STRIPS as f32;
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        height: rem(NAV_FEATHER_STRIP_PX),
                    }
                    BackgroundColor({ palette.sidebar.with_alpha(alpha) })
            }) as Box<dyn Scene>
        })
        .collect();
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                right: px(0.0),
                bottom: px(0.0),
                height: rem(NAV_FEATHER_STRIP_PX * NAV_FEATHER_STRIPS as f32),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
            }
            Pickable { should_block_lower: false, is_hoverable: true }
            Children [
                { strips }
            ]
    }
}
