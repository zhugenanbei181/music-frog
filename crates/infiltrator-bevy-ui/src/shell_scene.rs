//! Declarative BSN scenes for the Bevy shell layout.
//!
//! Subtree scenes composing the window root, the navigation-only sidebar rail,
//! the content column, the title header and the registry-driven bottom dock.

#[path = "shell_nav.rs"]
pub mod nav;

use crate::a11y::{nav_semantic_node, semantic_node};
use crate::app::shell_bidi::{ShellDirectionRoot, ShellDirectionRow};
use crate::app::shell_focus::ShellFocusable;
use crate::app::{
    BOTTOM_NAV_CONTENT_HEIGHT, BottomNavActive, BottomNavBar, BottomNavIconPill, BottomNavItem,
    ContentColumn, ContentSlot, ContentTitleLabel, HistoryBackButton, HistoryForwardButton,
    RoutingIntentStatusCapsule, SIDEBAR_WIDTH_PX, ShellHeader, ShellRoot, SidebarPanel,
    StatusDotImage,
};
use crate::chrome::chrome_bar_scene;
use crate::pages::connections_confirm::confirmation_scene;
use crate::pages::overview_speedtest::overview_speedtest_detail_modal_scene;
use crate::route::Route;
use crate::shell_readout::{StatusDotSlot, status_point_image, status_point_slot_scene};
use crate::shell_routing_intent_issue;
use bevy::color::Color;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Commands, Res};
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene_val, plate_for_canonical_name};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::responsive::TouchHitbox;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::design_resources;
use infiltrator_contract::shell_navigation::{NavigationItem, NavigationSurface};

/// The sidebar speed footer's waveform strip width in baseline pixels.
pub const SHELL_WAVEFORM_WIDTH_PX: f32 = 60.0;

/// The history arrow button's edge in baseline pixels.
///
/// The chevron is the shared chevron plate, so its box is sized for the
/// plate: a 14-pixel chevron in a 28-pixel button reads as a control, not as
/// a smudge.
const HISTORY_ARROW_PX: f32 = 28.0;

/// The history chevron's plate box, in baseline pixels.
const HISTORY_ARROW_ICON_PX: f32 = 14.0;

/// The bottom navigation's icon edge in baseline pixels.
const BOTTOM_NAV_ICON_PX: f32 = 18.0;

/// The root shell scene.
///
/// One entry point, and no toggle parameter: the sidebar mounts navigation
/// only, so the system-toggle snapshot has no consumer in the shell tree. The
/// dashboard's master-switch cards read the same shared projection from the
/// page's own snapshot — one publisher, one mount.
pub fn shell_scene(title: String, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let window_node = semantic_node(ShellA11yNode::Window);
    let region_node = semantic_node(ShellA11yNode::ContentRegion);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0), max_width: percent(100),
                height: percent(100),
                min_height: px(0.0), max_height: percent(100),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
            }
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            ShellRoot
            window_node
            Children [
                @{ chrome_bar_scene(palette) }
                --
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    max_width: percent(100),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_basis: px(0.0),
                    min_height: px(0.0),
                    flex_direction: FlexDirection::Row,
                    overflow: Overflow::clip(),
                }
                ShellDirectionRoot
                Children [
                    @{ sidebar_scene(palette) }
                    --
                    Node {
                        flex_grow: 1.0,
                        flex_shrink: 1.0,
                        min_width: px(0.0),
                        max_width: percent(100),
                        min_height: px(0.0),
                        height: percent(100),
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(rem(space::S16)),
                        row_gap: rem(space::S16),
                        overflow: Overflow::clip(),
                    }
                    ContentColumn
                    Children [
                        @{ content_title_row(&title, palette) }
                        --
                        Node {
                            width: percent(100),
                            min_width: px(0.0),
                            max_width: percent(100),
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            flex_basis: px(0.0),
                            min_height: px(0.0),
                            overflow: Overflow::clip(),
                        }
                        ContentSlot
                        region_node
                    ]
                ]
                --
                @{ bottom_nav_scene(palette) }
                --
                @{ overview_speedtest_detail_modal_scene(palette) }
                --
                @{ confirmation_scene(palette) }
                --
                @{ nav::tooltip_overlay_scene() }
            ]
    }
}

/// The bottom navigation bar for Compact mode (<600px).
///
/// The dock's destinations are the registry's [`NavigationSurface::Dock`]
/// list, the same typed source the desktop rail reads. The bar used to
/// restate four routes, four plates and four copy keys as literals, so a
/// destination added to the contract reached the rail and not the dock.
pub fn bottom_nav_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let edge = palette.border;
    let entries: Vec<Box<dyn Scene>> = NavigationItem::items(NavigationSurface::Dock)
        .iter()
        .map(|item| {
            let route = nav::route_for_destination(item.target);
            bottom_nav_item_scene(route, item, route == Route::Overview, palette)
        })
        .collect();

    Box::new(bsn! {
            Node {
                width: percent(100),
                height: rem(BOTTOM_NAV_CONTENT_HEIGHT),
                min_height: rem(BOTTOM_NAV_CONTENT_HEIGHT),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                border: UiRect::top(Val::Px(palette.hairline_px)),
                padding: UiRect::bottom(rem(space::S6)),
                display: Display::None,
            }
            ThemeBackgroundColor({ Token::Sidebar })
            BorderColor {
                top: edge,
                right: Color::NONE,
                bottom: Color::NONE,
                left: Color::NONE,
            }
            BottomNavBar
            Children [
                { entries }
            ]
    })
}

/// One bottom navigation entry: the route icon above its label.
fn bottom_nav_item_scene(
    route: Route,
    item: &NavigationItem,
    active: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let plate =
        plate_for_canonical_name(item.icon.canonical_name()).unwrap_or_else(|| route.icon());
    let label_key = item.target.label_key();
    let semantic = nav_semantic_node(&route.label(), false);
    let role = if active {
        design_resources::TextRole::BodyStrong
    } else {
        design_resources::TextRole::Caption
    };
    Box::new(bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: percent(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: rem(space::S4),
                padding: UiRect::vertical(rem(space::S4)),
            }
            Button
            BottomNavItem(route)
            BottomNavActive(active)
            ShellFocusable
            semantic LocalizedLabel::plain(label_key)
            Children [
                @{ bottom_nav_icon_seat_scene(plate, active, palette) }
                --
                LocalizedText::plain(label_key) TextRole(role)
            ]
    })
}

/// The icon's seat: the active route wears the shared selection-wash pill
/// (M3 bottom navigation), the inactive route stays bare. Both seats share
/// one geometry, so a route switch recolors without shifting the row.
fn bottom_nav_icon_seat_scene(icon: IconId, active: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let ink = if active {
        palette.accent
    } else {
        palette.ink_dim
    };
    if active {
        Box::new(bsn! {
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(rem(space::S12), rem(space::S2)),
                    border_radius: BorderRadius::MAX,
                }
                ThemeBackgroundColor({ Token::SelectionFill })
                BottomNavIconPill
                Children [
                    @{ icon_scene_val(icon, rem(BOTTOM_NAV_ICON_PX), ink) }
                ]
        })
    } else {
        Box::new(bsn! {
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(rem(space::S12), rem(space::S2)),
                    border_radius: BorderRadius::MAX,
                }
                BottomNavIconPill
                Children [
                    @{ icon_scene_val(icon, rem(BOTTOM_NAV_ICON_PX), ink) }
                ]
        })
    }
}

/// Mount the inline status point into its seat inside the title text.
///
/// The point is a *child* of the text: an inline image is an inline box, so
/// the text layout measures it, wraps with it and keeps it on the baseline.
/// Stamping the component on the text root instead would leave the dot
/// outside the paragraph, where it neither flows nor renders.
pub fn stamp_status_dot(
    trigger: On<Add<StatusDotSlot>>,
    palette: Res<UiPalette>,
    dot_image: Option<Res<StatusDotImage>>,
    mut commands: Commands,
) {
    let Some(dot_image) = dot_image else {
        return;
    };
    let handle = dot_image.0.clone();
    commands
        .entity(trigger.event().entity)
        .insert(status_point_image(handle, palette.ink_dim));
}

/// The title row with page heading, history navigation, and status indicators.
pub fn content_title_row(_title: &str, palette: &UiPalette) -> impl Scene + use<> {
    let header_node = semantic_node(ShellA11yNode::ShellHeader);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                column_gap: rem(space::S8),
                flex_wrap: FlexWrap::Wrap,
                row_gap: rem(space::S4),
            }
            ShellHeader
            header_node
            ShellDirectionRow
            Children [
                @{ history_arrow_scene(IconId::ChevronLeft, palette) }
                HistoryBackButton
                ShellFocusable
                --
                @{ history_arrow_scene(IconId::ChevronRight, palette) }
                HistoryForwardButton
                ShellFocusable
                --
                LocalizedText::plain("nav_overview") TextRole(design_resources::TextRole::Heading) ContentTitleLabel
                Children [
                    @{ status_point_slot_scene() }
                ]
                --
                Node { flex_grow: 1.0 }
                --
                @{ routing_status_capsule_scene(palette) }
                RoutingIntentStatusCapsule
                --
                @{ shell_routing_intent_issue::scene(palette) }
            ]
    }
}

/// The history arrow: a square control whose chevron is sized for its box.
///
/// The chevron is the shared chevron plate, not a text glyph: the embedded
/// faces carry no decorative Unicode, so a "‹" glyph would be tofu.
fn history_arrow_scene(icon: IconId, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                width: rem(HISTORY_ARROW_PX),
                height: rem(HISTORY_ARROW_PX),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            BackgroundColor({ palette.surface_elevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Button
            ControlVisual(false)
            TouchHitbox::default()
            Children [
                @{ icon_scene_val(icon, rem(HISTORY_ARROW_ICON_PX), palette.ink_dim) }
            ]
    }
}

/// The header's routing intent read-out.
///
/// The chip states the live routing intent and offers nothing to press, so it
/// must not wear the skin of a disabled button: a quiet surface, a hairline
/// edge and full ink.
fn routing_status_capsule_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
            Node {
                padding: UiRect::horizontal(rem(space::S12)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                align_items: AlignItems::Center,
            }
            BackgroundColor({ palette.surface_elevated })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            Children [
                Text({ String::new() }) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::Ink })
            ]
    }
}

/// Sidebar scene supporting Standard (240px) and polymorphic Rail/Wide modes.
/// The sidebar: the destination rows, and nothing else.
///
/// The rail is navigation only, which is what the desktop reference measures
/// and the space-utilization standard turns into a gate: `chrome_band` 0%
/// means nothing but navigation may sit in the column
/// (`docs/SPACE_UTILIZATION_STANDARD.md` §2,
/// `docs/reference/FLCLASH_DESKTOP_REFERENCE.md` §2 — FlClash's
/// `NavigationSidebar` receives destinations, selectedIndex and expanded, and
/// no state/rate/version property at all). The Bevy rail used to mount the
/// brand block, the version line, the routing-mode pills, the two master
/// switches, the active-profile card, the shortcut matrix and the rate footer
/// above the rows: every one of those facts already has a single owner in the
/// title bar or the dashboard, and the sidebar was the second publisher. The
/// mode pills and master switches were worse than a duplicate — they were a
/// second *live* copy of controls the Overview band already mounts, wired to
/// the same commands, so one screen offered two answers to one intent.
pub fn sidebar_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: rem(SIDEBAR_WIDTH_PX),
                height: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(rem(space::S12)),
                row_gap: rem(space::S8),
                overflow: Overflow::visible(),
            }
            ThemeBackgroundColor({ Token::Sidebar })
            SidebarPanel
            Children [
                @{ nav::nav_column_scene(palette) }
            ]
    }
}
