//! Proxy node card rendering with feature chips, favorite pin, trend waveforms, and toolbar controls.
//!
//! Subtree scenes composing the individual proxy node card (BEVY-GAP-031),
//! favorite pinning toggle and trend wave (BEVY-GAP-033), protocol & UDP chips (BEVY-GAP-035),
//! proxies toolbar controls ("只看可用", 4-sort pills, "测试地址"), and group cards.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::pages::proxies::{
    AddCustomNodeButton, DelayTestUrlIndicator, FilterAliveToggle, GroupCurrentText, GroupFoldText,
    GroupNodesContainer, LatencyText, LatencyTrendIcon, NodeFlagText, NodeNameText, NodePinButton,
    NodeProtoText, NodeUdpTag, ProxiesLine, ProxiesLineKind, ProxyGroup, ProxyGroupFoldButton,
    ProxyNode, ProxyNodeButton, ProxySortMode, ProxySortPill, TestAllProxiesButton,
    TestProxyGroupButton, ToggleViewModeButton, format_latency, latency_color,
};
use crate::pages::proxies_filter::{format_protocol_chip, node_flag};

/// Renders the toolbar card containing search box, "只看可用" toggle switch, 4 sort mode pills, and "测试地址" indicator.
pub fn search_bar_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mut search_a11y = accesskit::Node::new(accesskit::Role::Search);
    search_a11y.set_label("搜索代理或节点");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                row_gap: Val::Px(space::S8),
                column_gap: Val::Px(space::S12),
            }
            template_value(AccessibilityNode(search_a11y))
            Children [
                (
                    Node {
                        flex_grow: 1.0,
                        min_width: px(220.0),
                        height: px(palette.control_height_px),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(space::S12)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        column_gap: Val::Px(space::S8),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    BorderColor {
                        top: { palette.border },
                        right: { palette.border },
                        bottom: { palette.border },
                        left: { palette.border },
                    }
                    Children [
                        ( { icon_scene(IconId::Globe, 16.0, palette.ink_dim) } ),
                        (
                            Text({ "搜索代理或节点 (Search Proxies)...".to_owned() })
                            TextRole(Role::Caption)
                        ),
                    ]
                ),
                (
                    Node {
                        height: px(palette.control_height_px),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(space::S12)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        column_gap: Val::Px(space::S8),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    BorderColor {
                        top: { palette.border },
                        right: { palette.border },
                        bottom: { palette.border },
                        left: { palette.border },
                    }
                    FilterAliveToggle
                    Button
                    Children [
                        ( Text({ "只看可用".to_owned() }) TextRole(Role::Caption) ),
                        (
                            Node {
                                width: px(28.0),
                                height: px(16.0),
                                border_radius: BorderRadius::all(Val::Px(8.0)),
                                align_items: AlignItems::Center,
                                padding: UiRect::horizontal(Val::Px(2.0)),
                            }
                            BackgroundColor({ palette.accent })
                            Children [
                                (
                                    Node {
                                        width: px(12.0),
                                        height: px(12.0),
                                        border_radius: BorderRadius::all(Val::Px(6.0)),
                                    }
                                    BackgroundColor({ palette.surface })
                                ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(2.0)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        column_gap: Val::Px(space::S4),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    BorderColor {
                        top: { palette.border },
                        right: { palette.border },
                        bottom: { palette.border },
                        left: { palette.border },
                    }
                    Children [
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            BackgroundColor({ palette.accent_container })
                            ProxySortPill(ProxySortMode::LatencyAsc)
                            Button
                            Children [
                                ( Text({ "延迟升序".to_owned() }) TextRole(Role::Caption) TextColor({ palette.accent }) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            BackgroundColor({ palette.surface })
                            ProxySortPill(ProxySortMode::LatencyDesc)
                            Button
                            Children [
                                ( Text({ "延迟降序".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            BackgroundColor({ palette.surface })
                            ProxySortPill(ProxySortMode::NameAsc)
                            Button
                            Children [
                                ( Text({ "名称升序".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                align_items: AlignItems::Center,
                            }
                            BackgroundColor({ palette.surface })
                            ProxySortPill(ProxySortMode::NameDesc)
                            Button
                            Children [
                                ( Text({ "名称降序".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        height: px(palette.control_height_px),
                        align_items: AlignItems::Center,
                        padding: UiRect::horizontal(Val::Px(space::S12)),
                        border: UiRect::all(Val::Px(palette.hairline_px)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                        column_gap: Val::Px(space::S6),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    BorderColor {
                        top: { palette.border },
                        right: { palette.border },
                        bottom: { palette.border },
                        left: { palette.border },
                    }
                    DelayTestUrlIndicator
                    Children [
                        ( Text({ "测试地址".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                        ( Text({ "http://cp.cloudflare.com/generate_204".to_owned() }) TextRole(Role::Mono) ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

/// Renders the header summary card for proxy strategies with exit node and action triggers.
pub fn header_card_scene(
    summary: String,
    active_exit: String,
    test_status: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("代理策略概览");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
            }
            template_value(AccessibilityNode(header_a11y))
            Children [
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( { icon_tile_scene(IconId::Globe, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) ProxiesLine(ProxiesLineKind::Summary) TextRole(Role::Heading) ),
                                (
                                    Node {
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(space::S8),
                                    }
                                    Children [
                                        ( Text({ "当前出口:".to_owned() }) TextRole(Role::Caption) ),
                                        ( Text(active_exit) ProxiesLine(ProxiesLineKind::ActiveExit) TextRole(Role::BodyStrong) ),
                                    ]
                                ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                column_gap: Val::Px(space::S4),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            AddCustomNodeButton
                            Button
                            Children [
                                ( { icon_scene(IconId::Plus, 14.0, palette.ink) } ),
                                ( Text({ "+ 添加节点".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                column_gap: Val::Px(space::S4),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            ToggleViewModeButton
                            Button
                            Children [
                                ( { icon_scene(IconId::Activity, 14.0, palette.ink) } ),
                                ( Text({ "网格视图".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        ( Text(test_status) ProxiesLine(ProxiesLineKind::TestStatus) TextRole(Role::Caption) ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            TestAllProxiesButton
                            Button
                            Children [
                                ( Text({ "全部测速".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                    ]
                ),
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
    node: &ProxyNode,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = node.name.clone();
    let flag = node_flag(&node.name);
    let (delay_str, tier) = format_latency(node.delay_ms);
    let delay_color_val = latency_color(tier, palette);
    let proto_tag = format_protocol_chip(&node.node_type);

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
                    padding: UiRect::axes(Val::Px(space::S4), Val::Px(space::S2)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                }
                BackgroundColor({ palette.surface })
                Children [
                    ( Text({ f_str }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    let udp_chip: Vec<Box<dyn Scene>> = if has_udp {
        vec![Box::new(bsn! {
            Node {
                padding: UiRect::axes(Val::Px(space::S4), Val::Px(space::S2)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            BackgroundColor({ palette.surface })
            Children [
                (
                    Text({ "udp".to_owned() })
                    NodeUdpTag { group_idx: g_idx, node_idx: n_idx }
                    TextRole(Role::Caption)
                    TextColor({ palette.ink_dim })
                ),
            ]
        }) as Box<dyn Scene>]
    } else {
        Vec::new()
    };

    bsn! {
        Node {
            width: percent(49),
            min_height: px(58.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(space::S12), Val::Px(space::S8)),
            border: UiRect::all(border_width),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
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
        }
        Button
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    flex_grow: 1.0,
                    overflow: Overflow::clip(),
                }
                Children [
                    (
                        Text({ star.to_owned() })
                        TextRole(Role::BodyStrong)
                        TextColor({ star_color })
                    ),
                    (
                        Text({ "📌".to_owned() })
                        NodePinButton {
                            group_idx: g_idx,
                            node_idx: n_idx,
                            node_name: { name.clone() },
                        }
                        TextRole(Role::Caption)
                        TextColor({ if node.favorite { palette.warning } else { palette.ink_dim } })
                    ),
                    (
                        Text({ flag.to_owned() })
                        NodeFlagText { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::BodyStrong)
                    ),
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S2),
                            overflow: Overflow::clip(),
                        }
                        Children [
                            (
                                Text(name)
                                NodeNameText { group_idx: g_idx, node_idx: n_idx }
                                TextRole(Role::BodyStrong)
                            ),
                            (
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S4),
                                    flex_wrap: FlexWrap::Wrap,
                                }
                                Children [
                                    (
                                        Node {
                                            padding: UiRect::axes(Val::Px(space::S6), Val::Px(space::S2)),
                                            border_radius: BorderRadius::all(Val::Px(4.0)),
                                            align_items: AlignItems::Center,
                                            justify_content: JustifyContent::Center,
                                        }
                                        BackgroundColor({ palette.surface })
                                        Children [
                                            (
                                                Text(proto_tag)
                                                NodeProtoText { group_idx: g_idx, node_idx: n_idx }
                                                TextRole(Role::Caption)
                                            ),
                                        ]
                                    ),
                                    { udp_chip },
                                    { feature_chips },
                                ]
                            ),
                        ]
                    ),
                ]
            ),
            (
                Node {
                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(space::S4),
                }
                BackgroundColor({ palette.surface })
                Children [
                    (
                        Text({ "📈".to_owned() })
                        LatencyTrendIcon { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::Caption)
                    ),
                    (
                        Text(delay_str)
                        LatencyText { group_idx: g_idx, node_idx: n_idx }
                        TextRole(Role::Mono)
                        TextColor(delay_color_val)
                    ),
                ]
            ),
        ]
    }
}

/// Renders a strategy group card with collapsible nodes and group test trigger.
pub fn group_card_scene(
    g_idx: usize,
    group: &ProxyGroup,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let title = group.name.clone();
    let type_badge = format!("[{}]", group.group_type);
    let current_label = format!("选中: {}", group.current);
    let fold_label = if group.expanded {
        "折叠 ▼"
    } else {
        "展开 ▶"
    }
    .to_owned();
    let group_name = group.name.clone();

    let mut sorted_proxies: Vec<(usize, &ProxyNode)> = group.proxies.iter().enumerate().collect();
    sorted_proxies.sort_by_key(|(_, a)| std::cmp::Reverse(a.favorite));

    let node_scenes: Vec<Box<dyn Scene>> = sorted_proxies
        .into_iter()
        .map(|(n_idx, node)| {
            Box::new(proxy_node_scene(g_idx, n_idx, &group_name, node, palette)) as Box<dyn Scene>
        })
        .collect();

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
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text(title) TextRole(Role::BodyStrong) ),
                            (
                                Node {
                                    padding: UiRect::axes(Val::Px(space::S6), Val::Px(space::S2)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    ( Text(type_badge) TextRole(Role::Caption) ),
                                ]
                            ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text(current_label) GroupCurrentText(g_idx) TextRole(Role::Caption) ),
                            (
                                Node {
                                    min_height: px(28.0),
                                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                TestProxyGroupButton {
                                    group_idx: { g_idx },
                                    group_name: { group_name.clone() },
                                }
                                Button
                                Children [
                                    ( Text({ "组测速".to_owned() }) TextRole(Role::Caption) ),
                                ]
                            ),
                            (
                                Node {
                                    min_height: px(28.0),
                                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                ProxyGroupFoldButton {
                                    group_idx: { g_idx },
                                    group_name: { group.name.clone() },
                                }
                                Button
                                Children [
                                    ( Text(fold_label) GroupFoldText(g_idx) TextRole(Role::Caption) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Row,
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::SpaceBetween,
                    row_gap: Val::Px(space::S8),
                    column_gap: Val::Px(space::S8),
                    display: { nodes_display },
                }
                GroupNodesContainer(g_idx)
                Children [
                    { node_scenes },
                ]
            }),
        ],
        palette,
    )
}
