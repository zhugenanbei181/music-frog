//! Declarative BSN scenes for the Bevy shell layout.
//!
//! Subtree scenes composing the window root, sidebar rail, navigation
//! items, mode segment pills, content column, title header and bottom bar.

use bevy::color::Color;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::pill_caption_scene;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, NavLabel, nav_fill};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::app::{
    BottomNavActive, BottomNavBar, BottomNavItem, ContentColumn, ContentSlot, ContentTitleLabel,
    DensityToggle, GlobalModeCapsule, GlobalStatusDot, HistoryBackButton, HistoryForwardButton,
    IDENTITY_TILE_PX, SIDEBAR_WIDTH_PX, ShellHeader, ShellRoot, SidebarActiveProfileCard,
    SidebarFoot, SidebarNavItem, SidebarPanel, SidebarScriptModePill, SidebarShortcutMatrix,
    SidebarShortcutTile, SidebarSpeedFooter, SidebarSystemProxyCard, SidebarSystemProxyToggle,
    SidebarTunCard, SidebarTunToggle, ThemeToggle, header_semantic_node, nav_semantic_node,
    region_semantic_node, toggle_semantic_node, window_semantic_node,
};
use crate::pages::overview::{OverviewModePill, mode_label};
use crate::route::Route;
use infiltrator_contract::command::ProxyMode;

/// The root shell scene.
pub fn shell_scene(title: String, palette: &UiPalette) -> impl Scene + use<> {
    let window_node = window_semantic_node(&title);
    let region_node = region_semantic_node("核心概览");
    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            overflow: Overflow::clip(),
        }
        ShellRoot
        template_value(window_node)
        Children [
            (
                Node {
                    width: percent(100),
                    min_width: px(0.0),
                    max_width: percent(100),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    min_height: px(0.0),
                    flex_direction: FlexDirection::Row,
                    overflow: Overflow::clip(),
                }
                Children [
                    ( { sidebar_scene(palette) } ),
                    (
                        Node {
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            min_width: px(0.0),
                            max_width: percent(100),
                            min_height: px(0.0),
                            height: percent(100),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::all(Val::Px(space::S16)),
                            row_gap: Val::Px(space::S16),
                            overflow: Overflow::clip(),
                        }
                        ContentColumn
                        Children [
                            ( { content_title_row(&title, palette) } ),
                            (
                                Node {
                                    width: percent(100),
                                    min_width: px(0.0),
                                    max_width: percent(100),
                                    flex_grow: 1.0,
                                    flex_shrink: 1.0,
                                    min_height: px(0.0),
                                    overflow: Overflow::clip(),
                                }
                                ContentSlot
                                template_value(region_node)
                            ),
                        ]
                    ),
                ]
            ),
            ( { bottom_nav_scene(palette) } ),
        ]
    }
}

/// The bottom navigation bar for Compact mode (<600px).
pub fn bottom_nav_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let overview_node = nav_semantic_node(Route::Overview.label(), false);
    let proxies_node = nav_semantic_node(Route::Proxies.label(), false);
    let profiles_node = nav_semantic_node(Route::Profiles.label(), false);
    let settings_node = nav_semantic_node(Route::Settings.label(), false);
    let edge = palette.border;

    Box::new(bsn! {
        Node {
            width: percent(100),
            height: px(58.0),
            min_height: px(58.0),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceAround,
            border: UiRect::top(Val::Px(palette.hairline_px)),
            padding: UiRect::bottom(Val::Px(space::S6)),
            display: Display::None,
        }
        BackgroundColor({ palette.sidebar })
        BorderColor {
            top: edge,
            right: Color::NONE,
            bottom: Color::NONE,
            left: Color::NONE,
        }
        BottomNavBar
        Children [
            (
                { bottom_nav_item_scene(Route::Overview.label(), Route::Overview, IconId::Activity, true, palette) }
                template_value(overview_node)
            ),
            (
                { bottom_nav_item_scene(Route::Proxies.label(), Route::Proxies, IconId::Globe, false, palette) }
                template_value(proxies_node)
            ),
            (
                { bottom_nav_item_scene(Route::Profiles.label(), Route::Profiles, IconId::FileText, false, palette) }
                template_value(profiles_node)
            ),
            (
                { bottom_nav_item_scene(Route::Settings.label(), Route::Settings, IconId::Settings, false, palette) }
                template_value(settings_node)
            ),
        ]
    })
}

fn bottom_nav_item_scene(
    label: &str,
    route: Route,
    icon: IconId,
    active: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let ink = if active {
        palette.accent
    } else {
        palette.ink_dim
    };
    let role = if active {
        Role::BodyStrong
    } else {
        Role::Caption
    };
    Box::new(bsn! {
        Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(space::S4),
            padding: UiRect::vertical(Val::Px(space::S4)),
        }
        Button
        BottomNavItem(route)
        BottomNavActive(active)
        Children [
            ( { icon_scene(icon, 20.0, ink) } ),
            ( Text({ label.to_owned() }) TextRole(role) ),
        ]
    })
}

/// The title row with page heading, history navigation, and status indicators.
pub fn content_title_row(title: &str, palette: &UiPalette) -> impl Scene + use<> {
    let header_node = header_semantic_node(title);
    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(space::S4),
        }
        ShellHeader
        template_value(header_node)
        Children [
            (
                { pill_caption_scene("‹".to_owned(), false, palette) }
                HistoryBackButton
            ),
            (
                { pill_caption_scene("›".to_owned(), false, palette) }
                HistoryForwardButton
            ),
            ( Text({ "核心概览".to_owned() }) TextRole(Role::Heading) ContentTitleLabel ),
            ( Node { flex_grow: 1.0 } ),
            (
                Node {
                    width: px(8.0),
                    height: px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.success })
                GlobalStatusDot
            ),
            (
                { pill_caption_scene("规则模式".to_owned(), true, palette) }
                GlobalModeCapsule
            ),
        ]
    }
}

/// Sidebar scene supporting Standard (240px) and polymorphic Rail/Wide modes.
pub fn sidebar_scene(palette: &UiPalette) -> impl Scene + use<> {
    let pill_node = toggle_semantic_node("Toggle color theme");
    let density_node = toggle_semantic_node("Toggle layout density");
    bsn! {
        Node {
            width: px(SIDEBAR_WIDTH_PX),
            height: percent(100),
            flex_shrink: 0.0,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::S12)),
            row_gap: Val::Px(space::S8),
            overflow: Overflow::clip(),
        }
        BackgroundColor({ palette.sidebar })
        SidebarPanel
        Children [
            ( { identity_scene(palette) } ),
            ( { mode_segment_scene(ProxyMode::default(), palette) } ),
            ( { sidebar_system_toggles_scene(palette) } ),
            ( { sidebar_profile_card_scene(palette) } ),
            ( { sidebar_shortcut_matrix_scene(palette) } ),
            ( { sidebar_speed_footer_scene(palette) } ),
            ( { nav_column_scene(palette) } ),
            ( Node { flex_grow: 1.0 } ),
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    (
                        Text({ "0.30 demo".to_owned() }) TextRole(Role::Caption)
                        SidebarFoot
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Row,
                            column_gap: Val::Px(space::S4),
                        }
                        Children [
                            (
                                { pill_caption_scene("Theme".to_owned(), false, palette) }
                                ThemeToggle
                                template_value(pill_node)
                            ),
                            (
                                { pill_caption_scene("Density".to_owned(), false, palette) }
                                DensityToggle
                                template_value(density_node)
                            ),
                        ]
                    ),
                ]
            ),
        ]
    }
}

/// App identity block: Logo plate + "MusicFrog" title + version "v0.20.0".
pub fn identity_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S12),
        }
        Children [
            ( { icon_tile_scene(IconId::Network, IDENTITY_TILE_PX, palette) } ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    ( Text({ "MusicFrog".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "v0.20.0".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    }
}

/// Proxy-mode segment control pills (Rule, Global, Direct, Script).
pub fn mode_segment_scene(mode: ProxyMode, palette: &UiPalette) -> impl Scene + use<> {
    let rule_node = toggle_semantic_node(mode_label(ProxyMode::Rule));
    let global_node = toggle_semantic_node(mode_label(ProxyMode::Global));
    let direct_node = toggle_semantic_node(mode_label(ProxyMode::Direct));
    let script_node = toggle_semantic_node("脚本模式");
    bsn! {
        Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S4),
        }
        Children [
            (
                { pill_caption_scene(mode_label(ProxyMode::Rule).to_owned(), mode == ProxyMode::Rule, palette) }
                OverviewModePill(ProxyMode::Rule)
                template_value(rule_node)
            ),
            (
                { pill_caption_scene(mode_label(ProxyMode::Global).to_owned(), mode == ProxyMode::Global, palette) }
                OverviewModePill(ProxyMode::Global)
                template_value(global_node)
            ),
            (
                { pill_caption_scene(mode_label(ProxyMode::Direct).to_owned(), mode == ProxyMode::Direct, palette) }
                OverviewModePill(ProxyMode::Direct)
                template_value(direct_node)
            ),
            (
                { pill_caption_scene("脚本模式".to_owned(), false, palette) }
                SidebarScriptModePill
                template_value(script_node)
            ),
        ]
    }
}

/// Double system toggle cards in sidebar: 系统代理 and TUN 模式.
pub fn sidebar_system_toggles_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(space::S6),
        }
        Children [
            (
                Node {
                    flex_grow: 1.0,
                    flex_basis: percent(50),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S8)),
                    row_gap: Val::Px(space::S6),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.surface_elevated })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                SidebarSystemProxyCard
                Children [
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            ( { icon_scene(IconId::Network, 14.0, palette.accent) } ),
                            (
                                { pill_caption_scene("开".to_owned(), true, palette) }
                                SidebarSystemProxyToggle
                            ),
                        ]
                    ),
                    (
                        Text({ "系统代理".to_owned() })
                        TextRole(Role::Caption)
                        TextColor({ palette.ink })
                    ),
                ]
            ),
            (
                Node {
                    flex_grow: 1.0,
                    flex_basis: percent(50),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S8)),
                    row_gap: Val::Px(space::S6),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.surface_elevated })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                SidebarTunCard
                Children [
                    (
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            ( { icon_scene(IconId::Zap, 14.0, palette.warning) } ),
                            (
                                { pill_caption_scene("关".to_owned(), false, palette) }
                                SidebarTunToggle
                            ),
                        ]
                    ),
                    (
                        Text({ "TUN 模式".to_owned() })
                        TextRole(Role::Caption)
                        TextColor({ palette.ink })
                    ),
                ]
            ),
        ]
    }
}

/// Active profile card in sidebar showing subscription name, usage progress bar and percentage.
pub fn sidebar_profile_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::S8)),
            row_gap: Val::Px(space::S6),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        SidebarActiveProfileCard
        Children [
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [
                            ( { icon_scene(IconId::FileText, 14.0, palette.accent) } ),
                            ( Text({ "Default Profile".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                    ( { pill_caption_scene("订阅".to_owned(), true, palette) } ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    height: px(4.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    overflow: Overflow::clip(),
                }
                BackgroundColor({ palette.accent_container })
                Children [
                    (
                        Node {
                            width: percent(25),
                            height: percent(100),
                            border_radius: BorderRadius::all(Val::Px(2.0)),
                        }
                        BackgroundColor({ palette.accent })
                    ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    (
                        Text({ "46.4 GB / 186.2 GB".to_owned() })
                        TextRole(Role::Caption)
                        TextColor({ palette.ink_dim })
                    ),
                    (
                        Text({ "25%".to_owned() })
                        TextRole(Role::Caption)
                        TextColor({ palette.ink_dim })
                    ),
                ]
            ),
        ]
    }
}

/// 2x2 Shortcut Grid Matrix in sidebar: 代理策略 (8), 分流规则 (2842), 连接审计 (12), 域名解析 (4).
pub fn sidebar_shortcut_matrix_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S6),
        }
        SidebarShortcutMatrix
        Children [
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(space::S6),
                }
                Children [
                    ( { shortcut_tile_scene(IconId::Globe, "代理策略", "8", Route::Proxies, palette) } ),
                    ( { shortcut_tile_scene(IconId::FileText, "分流规则", "2842", Route::Rules, palette) } ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(space::S6),
                }
                Children [
                    ( { shortcut_tile_scene(IconId::Activity, "连接审计", "12", Route::Connections, palette) } ),
                    ( { shortcut_tile_scene(IconId::Network, "域名解析", "4", Route::Dns, palette) } ),
                ]
            ),
        ]
    }
}

fn shortcut_tile_scene(
    icon: IconId,
    label: &'static str,
    count: &'static str,
    route: Route,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_basis: percent(50),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::S8)),
            row_gap: Val::Px(space::S4),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        Button
        SidebarShortcutTile(route)
        Children [
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    ( { icon_scene(icon, 14.0, palette.accent) } ),
                    (
                        Node {
                            padding: UiRect::horizontal(Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(space::S4)),
                        }
                        BackgroundColor({ palette.accent_container })
                        Children [
                            (
                                Text({ count.to_owned() })
                                TextRole(Role::Caption)
                                TextColor({ palette.accent })
                            ),
                        ]
                    ),
                ]
            ),
            (
                Text({ format!("{label} ({count})") })
                TextRole(Role::Caption)
                TextColor({ palette.ink })
            ),
        ]
    }
}

/// Live speed footer in sidebar with throughput and mini trend indicator.
pub fn sidebar_speed_footer_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border: UiRect::all(Val::Px(palette.hairline_px)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        BorderColor { top: edge, right: edge, bottom: edge, left: edge }
        SidebarSpeedFooter
        Children [
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S4),
                        }
                        Children [
                            ( { icon_scene(IconId::ArrowUp, 10.0, palette.success) } ),
                            (
                                Text({ "↑ 124.5 KB/s".to_owned() })
                                TextRole(Role::Caption)
                                TextColor({ palette.success })
                            ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S4),
                        }
                        Children [
                            ( { icon_scene(IconId::ArrowDown, 10.0, palette.accent) } ),
                            (
                                Text({ "↓ 1.8 MB/s".to_owned() })
                                TextRole(Role::Caption)
                                TextColor({ palette.accent })
                            ),
                        ]
                    ),
                ]
            ),
            (
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::FlexEnd,
                    column_gap: Val::Px(2.0),
                    height: px(18.0),
                }
                Children [
                    ( Node { width: px(3.0), height: px(6.0) } BackgroundColor({ palette.accent }) ),
                    ( Node { width: px(3.0), height: px(10.0) } BackgroundColor({ palette.accent }) ),
                    ( Node { width: px(3.0), height: px(8.0) } BackgroundColor({ palette.accent }) ),
                    ( Node { width: px(3.0), height: px(14.0) } BackgroundColor({ palette.accent }) ),
                    ( Node { width: px(3.0), height: px(11.0) } BackgroundColor({ palette.accent }) ),
                    ( Node { width: px(3.0), height: px(16.0) } BackgroundColor({ palette.accent }) ),
                ]
            ),
        ]
    }
}

/// Sidebar navigation item scene.
pub fn sidebar_nav_item_scene(route: Route, active: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let semantic = nav_semantic_node(route.label(), false);
    Box::new(bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ nav_fill(active, palette) })
        Button
        SidebarNavItem(route)
        NavItem
        NavActive(active)
        template_value(semantic)
        Children [
            (
                Text({ route.label().to_owned() })
                TextRole({
                    if active {
                        Role::BodyStrong
                    } else {
                        Role::Body
                    }
                })
                NavLabel
            ),
        ]
    })
}

/// Sidebar nav column rendering all 11 routes in Route::ALL.
pub fn nav_column_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let nav_items: Vec<Box<dyn Scene>> = Route::ALL
        .iter()
        .map(|&route| sidebar_nav_item_scene(route, route == Route::Overview, palette))
        .collect();

    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S4),
        }
        Children [
            { nav_items },
        ]
    })
}
