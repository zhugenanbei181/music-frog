//! Declarative BSN scenes for the Bevy shell layout.
//!
//! Subtree scenes composing the window root, sidebar rail, navigation
//! items, mode segment pills, content column, title header and bottom bar.

use crate::a11y::{
    button_semantic_node, nav_semantic_node, semantic_node, switch_node, toggle_semantic_node,
};
use crate::app::shell_bidi::{ShellDirectionRoot, ShellDirectionRow};
use crate::app::shell_focus::ShellFocusable;
use crate::app::{
    BOTTOM_NAV_HEIGHT_PX, BottomNavActive, BottomNavBar, BottomNavItem, ContentColumn, ContentSlot,
    ContentTitleLabel, DensityToggle, GlobalModeCapsule, GlobalStatusDot, HistoryBackButton,
    HistoryForwardButton, IDENTITY_TILE_PX, NavSpacer, RailNavTooltip, SIDEBAR_WIDTH_PX,
    ShellHeader, ShellRoot, SidebarActiveProfileCard, SidebarExpandedOnly, SidebarFoot,
    SidebarFooterRow, SidebarIdentityText, SidebarModeSegment, SidebarNavItem, SidebarPanel,
    SidebarScriptModePill, SidebarShortcutMatrix, SidebarShortcutTile, SidebarSpeedFooter,
    SidebarSystemProxyCard, SidebarSystemProxyToggle, SidebarTunCard, SidebarTunToggle,
    ThemeToggle,
};
use crate::chrome::chrome_bar_scene;
use crate::localized_widgets::localized_pill_scene;
use crate::pages::connections_confirm::confirmation_scene;
use crate::pages::overview::OverviewModePill;
use crate::pages::overview_speedtest::overview_speedtest_detail_modal_scene;
use crate::route::{Route, page_id_for_route};
use crate::shell_mode_issue;
use crate::shell_readout::{ShellQuotaFill, ShellQuotaTrack, ShellReadoutText};
use crate::shell_waveform::ShellWaveform;
use bevy::color::Color;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_application::proxy_mode_projection::mode_copy_key;
use infiltrator_application::system_toggle_projection::compact_label;
use infiltrator_bevy_widgets::button::{ButtonDisabled, pill_caption_scene};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::nav::{NavActive, NavItem, NavLabel, nav_fill, nav_label_ink};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_bevy_widgets::tooltip::TooltipBubble;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::system_toggle::SystemToggleSnapshot;

/// The root shell scene.
pub fn shell_scene(title: String, palette: &UiPalette) -> impl Scene + use<> {
    shell_scene_with_toggles(title, &SystemToggleSnapshot::default(), palette)
}

/// Root shell scene with the initial shared system-toggle projection.
pub fn shell_scene_with_toggles(
    title: String,
    toggles: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
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
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                border: UiRect::all(Val::Px(1.0)),
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
                    @{ sidebar_scene_with_toggles(toggles, palette) }
                    --
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
            ]
    }
}

/// The bottom navigation bar for Compact mode (<600px).
pub fn bottom_nav_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let overview_node = nav_semantic_node(&Route::Overview.label(), false);
    let proxies_node = nav_semantic_node(&Route::Proxies.label(), false);
    let profiles_node = nav_semantic_node(&Route::Profiles.label(), false);
    let settings_node = nav_semantic_node(&Route::Settings.label(), false);
    let edge = palette.border;

    Box::new(bsn! {
            Node {
                width: percent(100),
                height: px(BOTTOM_NAV_HEIGHT_PX),
                min_height: px(BOTTOM_NAV_HEIGHT_PX),
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
                @{ bottom_nav_item_scene(Route::Overview, IconId::Activity, true, palette) }
                overview_node LocalizedLabel::plain(Route::Overview.label_key())
                --
                @{ bottom_nav_item_scene(Route::Proxies, IconId::Globe, false, palette) }
                proxies_node LocalizedLabel::plain(Route::Proxies.label_key())
                --
                @{ bottom_nav_item_scene(Route::Profiles, IconId::FileText, false, palette) }
                profiles_node LocalizedLabel::plain(Route::Profiles.label_key())
                --
                @{ bottom_nav_item_scene(Route::Settings, IconId::Settings, false, palette) }
                settings_node LocalizedLabel::plain(Route::Settings.label_key())
            ]
    })
}

fn bottom_nav_item_scene(
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
            ShellFocusable
            Children [
                @{ icon_scene(icon, 20.0, ink) }
                --
                LocalizedText::plain(route.label_key()) TextRole(role)
            ]
    })
}

/// The title row with page heading, history navigation, and status indicators.
pub fn content_title_row(_title: &str, palette: &UiPalette) -> impl Scene + use<> {
    let header_node = semantic_node(ShellA11yNode::ShellHeader);
    let status_node = semantic_node(ShellA11yNode::GlobalStatusDot);
    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                column_gap: Val::Px(space::S8),
                flex_wrap: FlexWrap::Wrap,
                row_gap: Val::Px(space::S4),
            }
            ShellHeader
            header_node
            ShellDirectionRow
            Children [
                @{ pill_caption_scene("‹".to_owned(), false, palette) }
                HistoryBackButton
                ShellFocusable
                --
                @{ pill_caption_scene("›".to_owned(), false, palette) }
                HistoryForwardButton
                ShellFocusable
                --
                LocalizedText::plain("nav_overview") TextRole(Role::Heading) ContentTitleLabel
                --
                Node { flex_grow: 1.0 }
                --
                Node {
                    width: px(8.0),
                    height: px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.ink_dim })
                GlobalStatusDot
                status_node
                --
                @{ pill_caption_scene(String::new(), false, palette) }
                ButtonDisabled(true)
                GlobalModeCapsule
                --
                @{ shell_mode_issue::scene(palette) }
            ]
    }
}

/// Sidebar scene supporting Standard (240px) and polymorphic Rail/Wide modes.
pub fn sidebar_scene(palette: &UiPalette) -> impl Scene + use<> {
    sidebar_scene_with_toggles(&SystemToggleSnapshot::default(), palette)
}

pub fn sidebar_scene_with_toggles(
    toggles: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let pill_node = semantic_node(ShellA11yNode::ThemeToggle);
    let density_node = toggle_semantic_node("Toggle layout density");
    bsn! {
            Node {
                width: px(SIDEBAR_WIDTH_PX),
                height: percent(100),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(space::S12)),
                row_gap: Val::Px(space::S8),
                overflow: Overflow::visible(),
            }
            BackgroundColor({ palette.sidebar })
            SidebarPanel
            Children [
                @{ identity_scene(palette) }
                --
                @{ mode_segment_scene(None, palette) }
                --
                @{ sidebar_system_toggles_scene(toggles, palette) }
                --
                @{ sidebar_profile_card_scene(palette) }
                --
                @{ sidebar_shortcut_matrix_scene(palette) }
                --
                @{ sidebar_speed_footer_scene(palette) }
                --
                @{ nav_column_scene(palette) }
                --
                Node { flex_grow: 1.0 }
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                SidebarFooterRow
                SidebarExpandedOnly
                Children [
                    Text({ "0.30 demo".to_owned() }) TextRole(Role::Caption)
                    SidebarFoot
                    --
                    Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(space::S4),
                    }
                    Children [
                        @{ pill_caption_scene("Theme".to_owned(), false, palette) }
                        ThemeToggle
                        ShellFocusable
                        pill_node
                        --
                        @{ pill_caption_scene("Density".to_owned(), false, palette) }
                        DensityToggle
                        ShellFocusable
                        density_node
                    ]
                ]
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
                @{ icon_tile_scene(IconId::Network, IDENTITY_TILE_PX, palette) }
                --
                Node {
                    flex_direction: FlexDirection::Column,
                }
                SidebarIdentityText
                SidebarExpandedOnly
                Children [
                    Text({ "MusicFrog".to_owned() }) TextRole(Role::BodyStrong)
                    --
                    Text({ "v0.20.0".to_owned() }) TextRole(Role::Caption)
                ]
            ]
    }
}

/// Proxy-mode segment control pills (Rule, Global, Direct, Script).
pub fn mode_segment_scene(mode: Option<ProxyMode>, palette: &UiPalette) -> impl Scene + use<> {
    let segment_node = semantic_node(ShellA11yNode::ModeSegment);
    let rule_node = button_semantic_node("");
    let global_node = button_semantic_node("");
    let direct_node = button_semantic_node("");
    let script_node = button_semantic_node("");
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S4),
            }
            SidebarModeSegment
            SidebarExpandedOnly
            segment_node
            Children [
                @{ localized_pill_scene(LocalizedText::plain(mode_copy_key(ProxyMode::Rule)), mode == Some(ProxyMode::Rule), palette) }
                ButtonDisabled(true)
                OverviewModePill(ProxyMode::Rule)
                rule_node
                --
                @{ localized_pill_scene(LocalizedText::plain(mode_copy_key(ProxyMode::Global)), mode == Some(ProxyMode::Global), palette) }
                ButtonDisabled(true)
                OverviewModePill(ProxyMode::Global)
                global_node
                --
                @{ localized_pill_scene(LocalizedText::plain(mode_copy_key(ProxyMode::Direct)), mode == Some(ProxyMode::Direct), palette) }
                ButtonDisabled(true)
                OverviewModePill(ProxyMode::Direct)
                direct_node
                --
                @{ localized_pill_scene(LocalizedText::plain(mode_copy_key(ProxyMode::Script)), false, palette) }
                ButtonDisabled(true)
                SidebarScriptModePill
                script_node
            ]
    }
}

/// Double system toggle cards in sidebar: 系统代理 and TUN 模式.
pub fn sidebar_system_toggles_scene(
    toggles: &SystemToggleSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let proxy_selected = toggles.system_proxy.is_enabled();
    let proxy_label = compact_label(&toggles.system_proxy, UiLocale::default().code());
    let proxy_node = switch_node(ShellA11yNode::SystemProxySwitch, proxy_selected);
    let tun_selected = toggles.tun.is_enabled();
    let tun_label = compact_label(&toggles.tun, UiLocale::default().code());
    let tun_node = switch_node(ShellA11yNode::TunSwitch, tun_selected);
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(space::S6),
            }
            Children [
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
                SidebarExpandedOnly
                Children [
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        @{ icon_scene(IconId::Network, 14.0, palette.accent) }
                        --
                        @{ pill_caption_scene(proxy_label, proxy_selected, palette) }
                        SidebarSystemProxyToggle
                        ShellFocusable
                        proxy_node
                    ]
                    --
                    LocalizedText::plain("settings_sys_proxy")
                    TextRole(Role::Caption)
                    TextColor({ palette.ink })
                ]
                --
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
                SidebarExpandedOnly
                Children [
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        @{ icon_scene(IconId::Zap, 14.0, palette.warning) }
                        --
                        @{ pill_caption_scene(tun_label, tun_selected, palette) }
                        SidebarTunToggle
                        ShellFocusable
                        tun_node
                    ]
                    --
                    LocalizedText::plain("tun_mode")
                    TextRole(Role::Caption)
                    TextColor({ palette.ink })
                ]
            ]
    }
}

/// Active profile card in sidebar showing subscription name, usage progress bar and percentage.
pub fn sidebar_profile_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let semantic = button_semantic_node("nav_profiles");
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
            Button SidebarShortcutTile(Route::Profiles)
            ShellFocusable
            semantic LocalizedLabel::plain("nav_profiles")
            SidebarExpandedOnly
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S6),
                    }
                    Children [
                        @{ icon_scene(IconId::FileText, 14.0, palette.accent) }
                        --
                        Text({ String::new() }) ShellReadoutText::ProfileName TextRole(Role::BodyStrong)
                    ]
                    --
                    Text({ String::new() }) ShellReadoutText::ProfileKind TextRole(Role::Caption)
                ]
                --
                Node {
                    width: percent(100),
                    height: px(4.0),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    overflow: Overflow::clip(),
                }
                BackgroundColor({ palette.accent_container }) ShellQuotaTrack
                Children [
                    Node {
                        width: percent(0),
                        height: percent(100),
                        border_radius: BorderRadius::all(Val::Px(2.0)),
                    }
                    BackgroundColor({ palette.accent }) ShellQuotaFill
                ]
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Text({ String::new() }) ShellReadoutText::ProfileUsage
                    TextRole(Role::Caption)
                    TextColor({ palette.ink_dim })
                    --
                    Text({ String::new() }) ShellReadoutText::ProfilePercent
                    TextRole(Role::Caption)
                    TextColor({ palette.ink_dim })
                ]
            ]
    }
}

/// Navigation counts replay the complete application observation, independent of list windows.
pub fn sidebar_shortcut_matrix_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S6),
            }
            SidebarShortcutMatrix
            SidebarExpandedOnly
            Children [
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(space::S6),
                }
                Children [
                    @{ shortcut_tile_scene(IconId::Globe, "nav_proxies", Route::Proxies, palette) }
                    --
                    @{ shortcut_tile_scene(IconId::FileText, "nav_rules", Route::Rules, palette) }
                ]
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(space::S6),
                }
                Children [
                    @{ shortcut_tile_scene(IconId::Activity, "nav_connections", Route::Connections, palette) }
                    --
                    @{ shortcut_tile_scene(IconId::Network, "nav_dns", Route::Dns, palette) }
                ]
            ]
    }
}

fn shortcut_tile_scene(
    icon: IconId,
    label: &'static str,
    route: Route,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let semantic = button_semantic_node(label);
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
            ShellFocusable
            semantic LocalizedLabel::plain(label)
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    @{ icon_scene(icon, 14.0, palette.accent) }
                    --
                    Node {
                        padding: UiRect::horizontal(Val::Px(space::S4)),
                        border_radius: BorderRadius::all(Val::Px(space::S4)),
                    }
                    BackgroundColor({ palette.accent_container })
                    Children [
                        Text({ String::new() }) ShellReadoutText::Count(page_id_for_route(route))
                        TextRole(Role::Caption)
                        TextColor({ palette.accent })
                    ]
                ]
                --
                LocalizedText::plain(label)
                TextRole(Role::Caption)
                TextColor({ palette.ink })
            ]
    }
}

/// Live speed footer in sidebar with throughput and mini trend indicator.
pub fn sidebar_speed_footer_scene(palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let rate_node = semantic_node(ShellA11yNode::TrafficReadout);
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
            SidebarExpandedOnly
            rate_node
            Children [
                Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(2.0) }
                Children [
                Node { width: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween }
                Children [
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S2),
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S4),
                    }
                    Children [
                        @{ icon_scene(IconId::ArrowUp, 10.0, palette.success) }
                        --
                        Text({ String::new() }) ShellReadoutText::Upload
                        TextRole(Role::Caption)
                        TextColor({ palette.success })
                    ]
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S4),
                    }
                    Children [
                        @{ icon_scene(IconId::ArrowDown, 10.0, palette.accent) }
                        --
                        Text({ String::new() }) ShellReadoutText::Download
                        TextRole(Role::Caption)
                        TextColor({ palette.accent })
                    ]
                ]
                --
                Node { width: px(60.0), height: px(24.0) }
                ShellWaveform

                ]
                --
                Text({ String::new() }) ShellReadoutText::RateStatus TextRole(Role::Caption) TextColor({ palette.danger })
                ]
            ]
    }
}

/// Sidebar navigation item scene.
pub fn sidebar_nav_item_scene(route: Route, active: bool, palette: &UiPalette) -> Box<dyn Scene> {
    let semantic = nav_semantic_node(&route.label(), false);
    let ink = nav_label_ink(active, palette);
    let edge = palette.border;
    Box::new(bsn! {
            Node {
                width: percent(100),
                min_height: px(palette.control_height_px),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexStart,
                padding: UiRect::horizontal(Val::Px(space::S12)),
                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                position_type: PositionType::Relative,
            }
            BackgroundColor({ nav_fill(active, palette) })
            Button
            SidebarNavItem(route)
            NavItem
            NavActive(active)
            ShellFocusable
            semantic LocalizedLabel::plain(route.label_key())
            Children [
                @{ icon_scene(route.icon(), 18.0, ink) }
                --
                Node { width: px(space::S8) } NavSpacer SidebarExpandedOnly
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
                Node {
                    position_type: PositionType::Absolute,
                    left: px(68.0),
                    display: Display::None,
                    padding: UiRect::new(Val::Px(space::S8), Val::Px(space::S8), Val::Px(space::S4), Val::Px(space::S4)),
                    border: UiRect::all(Val::Px(palette.hairline_px)),
                    border_radius: BorderRadius::all(Val::Px(radius::XS)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor({ palette.surface_elevated })
                BorderColor { top: edge, right: edge, bottom: edge, left: edge }
                TooltipBubble
                RailNavTooltip(route)
                Children [
                    LocalizedText::plain(route.label_key()) TextRole(Role::Caption)
                ]
            ]
    })
}

/// Sidebar nav column rendering all 11 routes in Route::ALL.
pub fn nav_column_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let nav_node = semantic_node(ShellA11yNode::SidebarNav);
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
            nav_node
            Children [
                { nav_items }
            ]
    })
}
