//! Proxy node card rendering with feature chips, favorite pin, trend waveforms, and toolbar controls.
//!
//! Subtree scenes composing the individual proxy node card (BEVY-GAP-031),
//! favorite pinning toggle and trend wave (BEVY-GAP-033), protocol & UDP chips (BEVY-GAP-035),
//! proxies toolbar controls ("只看可用", 4-sort pills, "测试地址"), and group cards.

use crate::pages::proxies::{
    AddCustomNodeButton, FilterAliveToggle, GroupCurrentText, GroupFoldText, GroupNodesContainer,
    LatencySkeletonPulse, LatencyText, LatencyTrendIcon, NodeDetailButton, NodeFlagText,
    NodeNameText, NodePinButton, NodeProtoText, NodeUdpTag, ProxiesLine, ProxiesLineKind,
    ProxyGroup, ProxyGroupFoldButton, ProxyGroupMoveDownButton, ProxyGroupMoveUpButton, ProxyNode,
    ProxyNodeButton, ProxySortMode, ProxySortPill, ResetProxyGroupOrderButton,
    TestAllProxiesButton, TestProxyGroupButton, ToggleViewModeButton, latency_color,
};
use crate::pages::proxies_filter::format_protocol_chip;
use crate::pages::proxies_identity::{ProxyGroupIdentity, ProxyGroupOutboundIdentity};
use crate::pages::proxies_preferences::{FilterAliveIndicator, ProxyFavoriteStar, ProxyViewLabel};
use crate::pages::proxies_search::search_input_scene;
use crate::pages::proxy_probe_settings::launcher;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_application::latency_projection::project_proxy_latency;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::gesture::{
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, SwipeToActionSpring,
};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::outbound::identity::OutboundId;
use infiltrator_shared::country_flags::node_flag_emoji;

/// Renders the toolbar card containing search box, "只看可用" toggle switch, 4 sort mode pills, and "测试地址" indicator.
pub fn search_bar_card_scene(query: &str, palette: &UiPalette) -> impl Scene + use<> {
    let search_a11y = accesskit::Node::new(accesskit::Role::Search);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: rem(space::S8),
                        column_gap: rem(space::S12),
                    }
                    AccessibilityNode(search_a11y)
                    LocalizedLabel::plain("proxies_search_placeholder")
                    Children [
                        @{ search_input_scene(query, palette) }
                        --
                        Node {
                            height: rem(palette.control_height_px),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(rem(space::S12)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            column_gap: rem(space::S8),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        FilterAliveToggle
                        Button
                        Children [
                            LocalizedText::plain("proxies_filter_alive") TextRole(Role::Caption)
                            --
                            Node {
                                width: rem(28.0),
                                height: rem(16.0),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(Val::Px(2.0)),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated }) FilterAliveIndicator
                            Children [
                                Node {
                                    width: rem(12.0),
                                    height: rem(12.0),
                                    border_radius: BorderRadius::all(Val::Px(6.0)),
                                }
                                ThemeBackgroundColor({ Token::Surface })
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(2.0)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                            border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            column_gap: rem(space::S4),
                        }
                        ThemeBackgroundColor({ Token::SurfaceElevated })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        Children [
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::AccentContainer })
                            ProxySortPill(ProxySortMode::LatencyAsc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_latency_ascending") TextRole(Role::Caption) ThemeTextColor({ Token::Accent })
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::Surface })
                            ProxySortPill(ProxySortMode::LatencyDesc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_latency_descending") TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::Surface })
                            ProxySortPill(ProxySortMode::NameAsc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_name_ascending") TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            ThemeBackgroundColor({ Token::Surface })
                            ProxySortPill(ProxySortMode::NameDesc)
                            Button
                            Children [
                                LocalizedText::plain("outbound_sort_name_descending") TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                            ]
                        ]
                        --
                        @{ launcher(palette) }
                    ]
        })],
        palette,
    )
}

/// Renders the header summary card for proxy strategies with exit node and action triggers.
pub fn header_card_scene(
    summary: LocalizedText,
    active_exit: String,
    test_status: LocalizedText,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let header_a11y = accesskit::Node::new(accesskit::Role::Header);

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: rem(space::S16),
                    }
                    AccessibilityNode(header_a11y)
                    LocalizedLabel::plain("proxies_title")
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::Globe, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S4),
                            }
                            Children [
                                LocalizedText { .. { summary } } ProxiesLine(ProxiesLineKind::Summary) TextRole(Role::Heading)
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S8),
                                }
                                Children [
                                    LocalizedText::plain("proxies_active_exit_label") TextRole(Role::Caption)
                                    --
                                    Text(active_exit) ProxiesLine(ProxiesLineKind::ActiveExit) TextRole(Role::BodyStrong)
                                ]
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: rem(space::S8),
                        }
                        Children [
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                column_gap: rem(space::S4),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            AddCustomNodeButton
                            Button
                            Children [
                                @{ icon_scene(IconId::Plus, 14.0, palette.ink) }
                                --
                                LocalizedText::plain("proxies_add_node_btn") TextRole(Role::BodyStrong)
                            ]
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                column_gap: rem(space::S4),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            ResetProxyGroupOrderButton
                            Button
                            Children [
                                LocalizedText::plain("proxies_reorder_reset") TextRole(Role::BodyStrong)
                            ]
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                column_gap: rem(space::S4),
                            }
                            ThemeBackgroundColor({ Token::SurfaceElevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            ToggleViewModeButton
                            Button
                            Children [
                                @{ icon_scene(IconId::Activity, 14.0, palette.ink) }
                                --
                                LocalizedText::plain("proxies_grid_view") ProxyViewLabel TextRole(Role::BodyStrong)
                            ]
                            --
                            LocalizedText { .. { test_status } } ProxiesLine(ProxiesLineKind::TestStatus) TextRole(Role::Caption)
                            --
                            Node {
                                min_height: rem(palette.control_height_px),
                                padding: UiRect::horizontal(rem(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::Accent })
                            TestAllProxiesButton
                            Button
                            Children [
                                LocalizedText::plain("runtime_delay_test_all") TextRole(Role::BodyStrong)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

/// Renders a single proxy node card with flag, title, protocol, features, and latency.
pub fn proxy_node_scene(
    g_idx: usize,
    n_idx: usize,
    group_name: &str,
    collection_id: Option<OutboundId>,
    node: &ProxyNode,
    selection_available: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    proxy_node_scene_sized(
        ProxyNodeSceneContext {
            group_idx: g_idx,
            node_idx: n_idx,
            group_name,
            collection_id,
            selection_available,
        },
        node,
        percent(31),
        palette,
    )
}

pub struct ProxyNodeSceneContext<'a> {
    pub group_idx: usize,
    pub node_idx: usize,
    pub group_name: &'a str,
    pub collection_id: Option<OutboundId>,
    pub selection_available: bool,
}

/// Renders a single proxy node card at an explicit column width. The
/// virtual-list node rows pass the shared wrapped-item percent so a recycled
/// row reuses the exact same card vocabulary as the full-mount path.
pub fn proxy_node_scene_sized(
    context: ProxyNodeSceneContext<'_>,
    node: &ProxyNode,
    width: Val,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let ProxyNodeSceneContext {
        group_idx: g_idx,
        node_idx: n_idx,
        group_name,
        collection_id,
        selection_available,
    } = context;
    let name = node.name.clone();
    let flag = if node.node_type.is_empty() {
        String::new()
    } else {
        node_flag_emoji(&node.name).into()
    };
    let latency = project_proxy_latency(node.delay_ms);
    let delay_color_val = latency_color(latency.band, palette);
    let delay_copy = LocalizedText::new(latency.key, latency.params);
    let protocol_chip: Vec<Box<dyn Scene>> = if node.node_type.is_empty() {
        Vec::new()
    } else {
        let proto_tag = format_protocol_chip(&node.node_type);
        vec![Box::new(bsn! {
            Node {
                padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            ThemeBackgroundColor({ Token::Surface })
            Children [ Text(proto_tag) NodeProtoText { group_idx: g_idx, node_idx: n_idx } TextRole(Role::Caption) ]
        })]
    };

    let (bg, border_color, border_width) = if node.selected {
        (palette.accent_container, palette.accent, Val::Px(1.5))
    } else {
        (
            palette.surface_elevated,
            palette.border,
            Val::Px(palette.hairline_px),
        )
    };

    let star = if node.favorite { "★" } else { "☆" };
    let star_color = if node.favorite {
        palette.warning
    } else {
        palette.ink_dim
    };

    let has_udp = node.features.iter().any(|f| f.eq_ignore_ascii_case("udp"));

    let feature_chips: Vec<_> = node
        .features
        .iter()
        .map(|f| {
            let f_str = f.clone();
            Box::new(bsn! {
                Node {
                    padding: UiRect::axes(rem(space::S4), rem(space::S2)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                ThemeBackgroundColor({ Token::Surface })
                Children [
                    Text({ f_str }) TextRole(Role::Caption) ThemeTextColor({ Token::InkDim })
                ]
}) as Box<dyn Scene>
        })
        .collect();

    let udp_chip: Vec<Box<dyn Scene>> = if has_udp {
        vec![Box::new(bsn! {
                    Node {
                        padding: UiRect::axes(rem(space::S4), rem(space::S2)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Surface })
                    Children [
                        Text({ "udp".to_owned() })
                        NodeUdpTag { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::Caption)
                        ThemeTextColor({ Token::InkDim })
                    ]
        }) as Box<dyn Scene>]
    } else {
        Vec::new()
    };

    bsn! {
            Node {
                width: { width },
                min_height: rem(58.0),
                border: UiRect::all(border_width),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                overflow: Overflow::clip(),
            }
            BackgroundColor({ bg })
            BorderColor {
                top: { border_color },
                right: { border_color },
                bottom: { border_color },
                left: { border_color },
            }
            ControlVisual({ node.selected })
            ProxyNodeButton {
                group_idx: { g_idx },
                node_idx: { n_idx },
                group_name: { group_name.to_owned() },
                node_name: { node.name.clone() },
                collection_id: { collection_id.clone() },
                outbound_id: { node.outbound_id.clone() },
            }
            Button
            ButtonDisabled({ !selection_available })
            SwipeToActionItem {
                offset_x: 0.0,
                max_action_width: 88.0,
            }
            SwipeToActionSpring::default()
            Children [
                Node {
                    width: percent(100),
                    min_height: rem(58.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(rem(space::S12), rem(space::S8)),
                }
                SwipeContentContainer
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                        flex_grow: 1.0,
                        overflow: Overflow::clip(),
                    }
                    Children [
                        Text({ star.to_owned() })
                        ProxyFavoriteStar
                        TextRole(Role::BodyStrong)
                        TextColor({ star_color })
                        --
                        Text({ "📌".to_owned() })
                        NodePinButton {
                            group_idx: g_idx,
                            node_idx: n_idx,
                            node_name: { name.clone() },
                            outbound_id: { node.outbound_id.clone() },
                        }
                        TextRole(Role::Caption)
                        ThemeTextColor({ if node.favorite { Token::Warning } else { Token::InkDim } })
                        --
                        Text({ "ℹ️".to_owned() })
                        NodeDetailButton {
                            group_idx: g_idx,
                            node_idx: n_idx,
                            node_name: { name.clone() },
                            outbound_id: { node.outbound_id.clone() },
                        }
                        Button
                        TextRole(Role::Caption)
                        ThemeTextColor({ Token::InkDim })
                        --
                        Text({ flag.to_owned() })
                        NodeFlagText { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::BodyStrong)
                        --
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: rem(space::S2),
                            overflow: Overflow::clip(),
                        }
                        Children [
                            Text({ name.clone() })
                            NodeNameText { group_idx: g_idx, node_idx: n_idx }
                            TextRole(Role::BodyStrong)
                            --
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: rem(space::S4),
                                flex_wrap: FlexWrap::Wrap,
                            }
                            Children [
                                { protocol_chip }
                                --
                                { udp_chip }
                                --
                                { feature_chips }
                            ]
                        ]
                    ]
                    --
                    Node {
                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        column_gap: rem(space::S4),
                    }
                    ThemeBackgroundColor({ Token::Surface })
                    LatencySkeletonPulse { group_idx: g_idx, node_idx: n_idx }
                    Children [
                        Text({ "📈".to_owned() })
                        LatencyTrendIcon { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::Caption)
                        --
                        LocalizedText::new(delay_copy.key,delay_copy.params)
                        LatencyText { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::Mono)
                        TextColor(delay_color_val)
                    ]
                ]
                --
                Node {
                    position_type: PositionType::Absolute,
                    right: px(0.0),
                    top: px(0.0),
                    bottom: px(0.0),
                    width: rem(88.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: rem(space::S4),
                    display: Display::None,
                }
                SwipeActionDrawer
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Children [
                    Node {
                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Warning })
                    Button
                    NodePinButton {
                        group_idx: g_idx,
                        node_idx: n_idx,
                        node_name: { name },
                        outbound_id: { node.outbound_id.clone() },
                    }
                    Children [
                        Text({ "★".to_owned() }) TextRole(Role::BodyStrong) ThemeTextColor({ Token::Surface })
                    ]
                    --
                    Node {
                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                    }
                    ThemeBackgroundColor({ Token::Accent })
                    Button
                    TestProxyGroupButton {
                        group_idx: { g_idx },
                        collection_id: { collection_id.clone() },
                    }
                    Children [
                        Text({ "⚡".to_owned() }) TextRole(Role::BodyStrong) ThemeTextColor({ Token::Surface })
                    ]
                ]
            ]
    }
}

/// Renders a strategy group card with collapsible nodes and group test trigger.
pub fn group_card_scene(
    g_idx: usize,
    group: &ProxyGroup,
    palette: &UiPalette,
) -> impl Scene + use<> {
    group_card_scene_impl(g_idx, group, palette, true)
}

/// Renders a strategy group card header only: the same identity, controls and
/// fold affordances, but no node children. The virtual node list owns node
/// mounting, so its group rows reuse this header vocabulary.
pub fn group_header_scene(
    g_idx: usize,
    group: &ProxyGroup,
    palette: &UiPalette,
) -> impl Scene + use<> {
    group_card_scene_impl(g_idx, group, palette, false)
}

fn group_card_scene_impl(
    g_idx: usize,
    group: &ProxyGroup,
    palette: &UiPalette,
    with_nodes: bool,
) -> impl Scene + use<> {
    let title = group.name.clone();
    let type_badge = format!("[{}]", group.group_type);
    let current_label = LocalizedText::new(
        "proxies_group_current",
        vec![("node", group.current.clone())],
    );
    let fold_label = if group.expanded {
        LocalizedText::plain("rules_collapse")
    } else {
        LocalizedText::plain("rules_expand")
    };
    let group_name = group.name.clone();

    let node_scenes: Vec<Box<dyn Scene>> = if with_nodes && group.expanded {
        let sorted_proxies: Vec<(usize, &ProxyNode)> = group.proxies.iter().enumerate().collect();
        sorted_proxies
            .into_iter()
            .map(|(n_idx, node)| {
                Box::new(proxy_node_scene(
                    g_idx,
                    n_idx,
                    &group_name,
                    group.collection_id.clone(),
                    node,
                    group.outbound_selection_available,
                    palette,
                )) as Box<dyn Scene>
            })
            .collect()
    } else {
        Vec::new()
    };

    let nodes_display = if group.expanded {
        Display::Flex
    } else {
        Display::None
    };

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            ProxyGroupIdentity({ group.name.clone() })
                            ProxyGroupOutboundIdentity({ group.collection_id.clone() })
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S8),
                                }
                                Children [
                                    Text(title) TextRole(Role::BodyStrong)
                                    --
                                    Node {
                                        padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                                        border_radius: BorderRadius::all(Val::Px(4.0)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                    }
                                    ThemeBackgroundColor({ Token::SurfaceElevated })
                                    Children [
                                        Text(type_badge) TextRole(Role::Caption)
                                    ]
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S8),
                                }
                                Children [
                                    LocalizedText { .. { current_label } } GroupCurrentText(g_idx) TextRole(Role::Caption)
                                    --
                                    Node {
                                        min_height: rem(28.0),
                                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                    }
                                    ThemeBackgroundColor({ Token::SurfaceElevated })
                                    TestProxyGroupButton {
                                        group_idx: { g_idx },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    Button
                                    Children [
                                        LocalizedText::plain("proxies_test_group") TextRole(Role::Caption)
                                    ]
                                    --
                                    Node {
                                        min_height: rem(28.0),
                                        padding: UiRect::axes(rem(space::S6), rem(space::S4)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                    }
                                    ThemeBackgroundColor({ Token::SurfaceElevated })
                                    ProxyGroupMoveUpButton {
                                        group_idx: { g_idx },
                                        group_name: { group_name.clone() },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    Button
                                    Children [
                                        Text({ "▲".to_owned() }) TextRole(Role::Caption)
                                    ]
                                    --
                                    Node {
                                        min_height: rem(28.0),
                                        padding: UiRect::axes(rem(space::S6), rem(space::S4)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                    }
                                    ThemeBackgroundColor({ Token::SurfaceElevated })
                                    ProxyGroupMoveDownButton {
                                        group_idx: { g_idx },
                                        group_name: { group_name.clone() },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    Button
                                    Children [
                                        Text({ "▼".to_owned() }) TextRole(Role::Caption)
                                    ]
                                    --
                                    Node {
                                        min_height: rem(28.0),
                                        padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                                    }
                                    ThemeBackgroundColor({ Token::SurfaceElevated })
                                    ProxyGroupFoldButton {
                                        group_idx: { g_idx },
                                        group_name: { group.name.clone() },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    Button
                                    Children [
                                        LocalizedText { .. { fold_label } } GroupFoldText(g_idx) TextRole(Role::Caption)
                                    ]
                                ]
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Row,
                                flex_wrap: FlexWrap::Wrap,
                                justify_content: JustifyContent::SpaceBetween,
                                row_gap: rem(space::S8),
                                column_gap: rem(space::S8),
                                display: { nodes_display },
                            }
                            GroupNodesContainer(g_idx)
                            ProxyGroupIdentity({ group.name.clone() })
                            ProxyGroupOutboundIdentity({ group.collection_id.clone() })
                            Children [
                                { node_scenes }
                            ]
            }),
        ],
        palette,
    )
}
