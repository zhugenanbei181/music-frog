//! The shell's navigation column: the destination rows, their hover fill and
//! the icon-only tooltip.
//!
//! The route row is the shell's own control: the shared widget layer owns the
//! generic nav item, the shell owns the route row — the route icon, its label,
//! the hover fill that eases in under the pointer and, in the icon-only rail,
//! the tooltip that names the route.
//!
//! **Which rows exist is not this file's decision.** The set, order and
//! grouping come from `infiltrator_contract::shell_navigation::desktop_items`
//! — the same registry the Slint rail projects — so a row appears here exactly
//! when the contract says the surface owns that destination. The column used to
//! enumerate `Route::ALL`, i.e. one row per *route* (fifteen), including DNS,
//! Doctor, Sync, Tools, About, Disclaimer and Licenses, which the registry
//! folds under `Settings`; those extra rows pushed the stack below the fold and
//! the space above it went to the shell's own status cards. A surface never
//! keeps a second navigation list.
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
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::FixedNode;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, JustifyContent,
    Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui_widgets::{Button, ScrollArea};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{icon_scene_val, plate_for_canonical_name};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::motion::Spring;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, NavLabel, NavRowKind, nav_row_visual};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::picking;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_bevy_widgets::tooltip::TooltipBubble;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::design_resources;
use infiltrator_contract::responsive_viewport::SidebarForm;
use infiltrator_contract::shell_navigation::{
    NavigationGroup, NavigationItem, NavigationTarget, desktop_items,
};
use infiltrator_contract::shell_sidebar_presentation::{NavigationSeparator, separator_for_form};
use infiltrator_contract::surface::SurfaceKind;

/// The destination row's height in baseline pixels. Published because the
/// rail's morphology system sets the row's padding and the tests assert the
/// row's geometry, so neither restates it.
///
/// The reference's row geometry, measured and adjudicated once for the
/// desktop rail (`docs/reference/FLCLASH_DESKTOP_REFERENCE.md` §2): the same
/// 44 the Slint surface publishes as `rail-row-height`. Seven rows at 44 plus
/// their 4 gaps still clear the 480-pixel compact viewport, which is what the
/// old 32 was justified by — that justification was counting the fifteen
/// `Route::ALL` rows this column no longer mounts.
pub const NAV_ROW_HEIGHT_PX: f32 = 44.0;

/// The destination row's icon edge in baseline pixels: the reference's
/// `_iconSize = 24`, the same number Slint publishes as `nav-icon-size`.
pub const NAV_ROW_ICON_PX: f32 = 24.0;

/// The gap between the icon and the label: the reference's `_labelGap = 12`.
pub const NAV_LABEL_GAP_PX: f32 = space::S12;

/// The ground a labelled column gives away before its first glyph box. The
/// reference's shipped scene measures 7.2dp inside a 220dp column; Slint
/// publishes the same fact as `rail-content-leading: space-2`.
pub const NAV_CONTENT_LEADING_PX: f32 = space::S8;

/// The ground a labelled row leaves after its label: the reference's
/// `_itemGap`-scale inner padding (`rail-item-padding: space-4`).
pub const NAV_TRAILING_PADDING_PX: f32 = space::S4;

/// The gap between destination rows: the reference's `_itemGap = 4`.
pub const NAV_ROW_GAP_PX: f32 = space::S4;

/// The selected row's indicator bar: the reference's `_indicatorWidth = 3`
/// and `nav-indicator-height: 16`.
pub const NAV_INDICATOR_WIDTH_PX: f32 = 3.0;
pub const NAV_INDICATOR_HEIGHT_PX: f32 = 16.0;

/// The indicator's top offset: the row's height minus the bar, centred.
pub const NAV_INDICATOR_TOP_PX: f32 = (NAV_ROW_HEIGHT_PX - NAV_INDICATOR_HEIGHT_PX) / 2.0;

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

/// Marker on the selected destination row's indicator bar.
///
/// The bar is the row's only accent, and it exists only on the selected row:
/// an idle row mounts a `Display::None` node so the stack's geometry does not
/// depend on which row is current.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavIndicator;

/// Marker on a group heading row.
///
/// Whether this column mounts one at all is not this file's decision: it is
/// `shell_sidebar_presentation::separator_for_form`. The desktop reference
/// measures one uninterrupted destination list, and a heading spends a line
/// box plus its own air, so the current policy paints none for every desktop
/// form and the rows below carry the registry's `group` fact without a title.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavGroupHeading;

/// Marker on the viewport-level overlay that carries the shell's tooltips.
///
/// The overlay is a [`FixedNode`]: a tooltip is positioned against the
/// camera's viewport, not against the rail it names.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ShellTooltipOverlay;

/// The viewport-level overlay the shell's tooltips are mounted into.
///
/// The overlay spans the whole surface, so it must not take part in
/// picking: without an explicit policy Bevy lets every node that carries no
/// picking policy block the hits underneath it, and a full-screen container
/// would then swallow every touch on the surface below it.
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
            picking::NON_INTERACTIVE
            ShellTooltipOverlay
    }
}

/// The destination row: the indicator, the icon, the label and the hover fill
/// under them.
///
/// The row is the reference's geometry, not a control's: a full-width wash at
/// [`infiltrator_bevy_widgets::palette::NAV_SELECTION_WASH_ALPHA`] plus a narrow
/// accent indicator at the column
/// edge, with the accent ink saying "here". The old reading painted the
/// selected row in solid accent with `on_accent` ink, which made the current
/// destination the loudest thing in the column and turned the row's empty
/// right side — the desktop reference's legitimate breathing room — into a
/// filled capsule.
///
/// Fill, ink and label weight are **not** decided here: the row declares
/// [`NavRowKind::CurrentDestination`] and the widget layer's
/// [`infiltrator_bevy_widgets::nav::sync_nav_visuals`] paints it, so a route
/// switch restamps the same entities instead of racing a second writer. The
/// indicator is mounted on every row and only shown on the current one, so the
/// stack's geometry never depends on which row is active.
///
/// The label copy is the registry's key, never a route's own: a row that
/// folds pages under it is named by its destination, and the page-name owner
/// (`ShellPage::sidebar_key`) is what the other surfaces read.
pub fn sidebar_nav_item_scene(
    route: Route,
    item: &NavigationItem,
    active: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let label_key = item.target.label_key().unwrap_or_else(|| route.label_key());
    let semantic = nav_semantic_node(&route.label(), false);
    let plate =
        plate_for_canonical_name(item.icon.canonical_name()).unwrap_or_else(|| route.icon());
    let visual = nav_row_visual(NavRowKind::CurrentDestination, active);
    Box::new(bsn! {
            Node {
                width: percent(100),
                min_height: rem(NAV_ROW_HEIGHT_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                position_type: PositionType::Relative,
            }
            Button
            SidebarNavItem(route)
            NavItem
            NavRowKind::CurrentDestination
            NavActive(active)
            ShellFocusable
            semantic LocalizedLabel::plain(label_key)
            Children [
                @{ nav_hover_fill_scene(palette) }
                --
                @{ nav_indicator_scene(active) }
                --
                @{ icon_scene_val(plate, rem(NAV_ROW_ICON_PX), palette.ink) }
                --
                Node { width: rem(NAV_LABEL_GAP_PX) } NavSpacer SidebarExpandedOnly
                --
                LocalizedText::plain(label_key)
                TextRole({ visual.role })
                NavLabel
                SidebarExpandedOnly
                --
                @{ rail_tooltip_scene(route, palette) }
            ]
    })
}

/// The selected row's indicator: a narrow accent bar at the column edge.
///
/// It is the row's only accent. Every row mounts one and `sync_nav_indicator`
/// shows only the current row's, so the stack's geometry is the same whichever
/// destination is active.
fn nav_indicator_scene(active: bool) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: rem(NAV_INDICATOR_TOP_PX),
                width: rem(NAV_INDICATOR_WIDTH_PX),
                height: rem(NAV_INDICATOR_HEIGHT_PX),
                border_radius: BorderRadius::MAX,
                display: { if active { Display::Flex } else { Display::None } },
            }
            ThemeBackgroundColor({ Token::Accent })
            picking::HOVER_PASSTHROUGH
            NavIndicator
    })
}

/// A group heading: the registry's own title, mounted once where the group
/// changes *and only where the contract's separator policy paints one*. It is
/// a title and nothing else — no hairline repeats the boundary a second time,
/// and the heading owns the air above itself. An icon-only rail hides it (it
/// carries `SidebarExpandedOnly`): glyphs are readable without headings, and a
/// heading would cost the rail its last rows.
fn nav_group_heading_scene(group: NavigationGroup) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                padding: UiRect::new(
                    rem(NAV_CONTENT_LEADING_PX),
                    px(0.0),
                    rem(space::S4),
                    rem(space::S2),
                ),
            }
            SidebarExpandedOnly
            NavGroupHeading
            Children [
                LocalizedText::plain(group.title_key())
                TextRole(design_resources::TextRole::Tag)
                ThemeTextColor({ Token::InkDim })
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
                LocalizedText::plain(route.label_key()) TextRole(design_resources::TextRole::Caption)
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

/// The navigation column: one uninterrupted row per mounted destination.
///
/// The column is the shell's scroll area: when the rows do not fit the
/// viewport they stay reachable instead of being cut off, and the feather at
/// its foot says so — the last row dissolves into the rail rather than being
/// sliced by the edge.
///
/// Whether a group boundary is painted as a heading is the contract's
/// separator policy, read here and never restated as a local boolean: the
/// desktop reference measures one uninterrupted destination list, and every
/// exported desktop form currently answers `None`, so the registry's `group`
/// fact stays a fact without spending a title line box in the column.
pub fn nav_column_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let nav_node = semantic_node(ShellA11yNode::SidebarNav);
    let items = desktop_items(SurfaceKind::BevyDesktop);
    let paints_headings = separator_for_labelled_form().paints_heading();
    let mut rows: Vec<Box<dyn Scene>> = Vec::with_capacity(items.len() + 2);
    for (index, item) in items.iter().enumerate() {
        if paints_headings && (index == 0 || item.group != items[index - 1].group) {
            rows.push(nav_group_heading_scene(item.group));
        }
        let route = route_for_destination(item.target);
        rows.push(sidebar_nav_item_scene(
            route,
            item,
            route == Route::Overview,
            palette,
        ));
    }

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
                    row_gap: rem(NAV_ROW_GAP_PX),
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [
                    { rows }
                ]
                --
                @{ nav_scroll_feather_scene(palette) }
            ]
    })
}

/// Resolve one registry destination onto this surface's route.
///
/// A shared page maps one-to-one onto a Bevy route. The surface-local row the
/// registry offers to other surfaces has no Bevy route: [`desktop_items`]
/// filters it out for this surface because Bevy folds traffic into its
/// overview, so reaching this arm means the contract published a row this
/// surface cannot navigate to — a defect to fix in the registry, never a page
/// to guess here.
pub(crate) fn route_for_destination(target: NavigationTarget) -> Route {
    match target {
        NavigationTarget::Shared(page) => Route::from_shell_page(page),
        NavigationTarget::SurfaceLocal(id) => {
            unreachable!("the registry offered {id} to a surface that does not own it")
        }
    }
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
            picking::HOVER_PASSTHROUGH
            Children [
                { strips }
            ]
    }
}
/// The separator policy this column reads from the contract.
///
/// `Standard` is the desktop default form and `Wide` shares its answer: the
/// contract's own `every_desktop_tier_uses_the_reference_navigation_rhythm`
/// walks every exported tier and answers `None`, and a surface that painted a
/// boundary at one labelled width while hiding it at the next would contradict
/// that. The rail needs no separate read — `nav_group_heading_scene` carries
/// `SidebarExpandedOnly`, so an icon-only column cannot pay a title line for
/// its glyphs whatever the policy says.
fn separator_for_labelled_form() -> NavigationSeparator {
    separator_for_form(SidebarForm::Standard)
}
