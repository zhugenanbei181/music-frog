//! Telemetry and topology cards for the Overview page.
//!
//! Subtree scenes composing the topology chain, subscription quota,
//! active exit node card (BEVY-GAP-019), and system proxy / TUN master cards (BEVY-GAP-021).

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::button::pill_caption_scene;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::pages::overview::{
    AccentContainerFill, AccentFill, BorderFill, MiddleTopologyArrow, SubscriptionQuotaCard,
    SurfaceElevatedFill, SurfaceFill, TopologyChainCard,
};

/// Marker on active exit node card (BEVY-GAP-019).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveExitNodeCard;

/// Marker on system proxy master switch card (BEVY-GAP-021).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SystemProxyMasterCard;

/// Marker on TUN master switch card (BEVY-GAP-021).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunMasterCard;

/// The traffic topology chain card: 4 linked stage chips with connecting arrows (">").
pub fn topology_chain_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Region);
    header_a11y.set_label("分流网络拓扑");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S12),
            }
            template_value(AccessibilityNode(header_a11y))
            TopologyChainCard
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
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(IconId::Network, 16.0, palette.accent) } ),
                                ( Text({ "分流网络拓扑 (Traffic Topology)".to_owned() }) TextRole(Role::Heading) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ "12 连接".to_owned() }) TextRole(Role::Caption) TextColor({ palette.success }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        min_width: px(0.0),
                        max_width: percent(100),
                        flex_direction: FlexDirection::Row,
                        flex_wrap: FlexWrap::Wrap,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        row_gap: Val::Px(space::S8),
                        column_gap: Val::Px(space::S6),
                    }
                    Children [
                        (
                            Node {
                                flex_grow: 1.0,
                                flex_shrink: 1.0,
                                flex_basis: px(280.0),
                                min_width: px(240.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                column_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( { topology_stage_chip_scene(IconId::Activity, "Client / Inbound".to_owned(), "12 conns".to_owned(), palette.success, "Mixed: 7890".to_owned(), palette) } ),
                                ( { topology_arrow_scene(palette) } ),
                                ( { topology_stage_chip_scene(IconId::FileText, "RuleSet".to_owned(), "Active".to_owned(), palette.accent, "MRS / GeoIP".to_owned(), palette) } ),
                            ]
                        ),
                        (
                            { topology_arrow_scene(palette) }
                            MiddleTopologyArrow
                        ),
                        (
                            Node {
                                flex_grow: 1.0,
                                flex_shrink: 1.0,
                                flex_basis: px(280.0),
                                min_width: px(240.0),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                column_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( { topology_stage_chip_scene(IconId::Settings, "Proxy Group".to_owned(), "Selector".to_owned(), palette.warning, "GLOBAL / PROXIES".to_owned(), palette) } ),
                                ( { topology_arrow_scene(palette) } ),
                                ( { topology_stage_chip_scene(IconId::Globe, "Outbound Node".to_owned(), "38 ms".to_owned(), palette.success, "香港 01 · BGP 专线".to_owned(), palette) } ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn topology_stage_chip_scene(
    icon: IconId,
    stage_name: String,
    badge_text: String,
    badge_fg: Color,
    detail: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            flex_basis: px(120.0),
            min_width: px(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S6),
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        SurfaceElevatedFill
        Children [
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(space::S6),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S6),
                        }
                        Children [
                            ( { icon_scene(icon, 14.0, palette.ink_dim) } ),
                            ( Text({ stage_name }) TextRole(Role::Caption) ),
                        ]
                    ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S6), Val::Px(space::S2)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.accent_container })
                        AccentContainerFill
                        Children [
                            ( Text({ badge_text }) TextRole(Role::Caption) TextColor({ badge_fg }) ),
                        ]
                    ),
                ]
            ),
            (
                Node {
                    width: percent(100),
                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ palette.surface })
                SurfaceFill
                Children [
                    ( Text({ detail }) TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    }
}

fn topology_arrow_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_shrink: 0.0,
            padding: UiRect::horizontal(Val::Px(space::S2)),
        }
        Children [
            ( Text({ ">".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.ink_dim }) ),
        ]
    }
}

/// The subscription quota card: displays subscription name, expiry date,
/// used / total data with an accent progress bar.
pub fn subscription_quota_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mut quota_a11y = accesskit::Node::new(accesskit::Role::Region);
    quota_a11y.set_label("订阅配额");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            template_value(AccessibilityNode(quota_a11y))
            SubscriptionQuotaCard
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        ( { icon_scene(IconId::FileText, 16.0, palette.accent) } ),
                        ( Text({ "订阅配额".to_owned() }) TextRole(Role::Caption) ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        ( Text({ "主力高速订阅 (Primary VIP)".to_owned() }) TextRole(Role::Heading) ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ "2026-10-01 到期".to_owned() }) TextRole(Role::Caption) TextColor({ palette.accent }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                    }
                    Children [
                        ( Text({ "已用: 46.43 GB / 总计: 186.26 GB (24.9%)".to_owned() }) TextRole(Role::Caption) ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        height: px(8.0),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        overflow: Overflow::clip(),
                    }
                    BackgroundColor({ palette.border })
                    BorderFill
                    Children [
                        (
                            Node {
                                width: percent(25),
                                height: percent(100),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                            }
                            BackgroundColor({ palette.accent })
                            AccentFill
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

/// Active exit node card with flag, protocol and latency (BEVY-GAP-019).
pub fn active_exit_node_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mut a11y = accesskit::Node::new(accesskit::Role::Region);
    a11y.set_label("当前主出口节点");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            template_value(AccessibilityNode(a11y))
            ActiveExitNodeCard
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
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(IconId::Globe, 16.0, palette.accent) } ),
                                ( Text({ "当前主出口节点 (Active Exit Node)".to_owned() }) TextRole(Role::Heading) ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ "38 ms".to_owned() }) TextRole(Role::Caption) TextColor({ palette.success }) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::all(Val::Px(space::S8)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    SurfaceElevatedFill
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( Text({ "🇭🇰".to_owned() }) TextRole(Role::BodyStrong) ),
                                ( Text({ "香港 IPLC 01 (BGP 专线)".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S6),
                            }
                            Children [
                                ( Text({ "VLESS · Reality".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                                ( { pill_caption_scene("切换节点".to_owned(), false, palette) } ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

/// Dual system proxy and TUN master switch cards (BEVY-GAP-021).
pub fn master_switches_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(space::S12),
            flex_wrap: FlexWrap::Wrap,
            row_gap: Val::Px(space::S8),
        }
        Children [
            (
                { single_master_card_scene("系统代理 (System Proxy)", "接管系统 HTTP/SOCKS 端口", IconId::Settings, true, palette) }
                SystemProxyMasterCard
            ),
            (
                { single_master_card_scene("TUN 模式 (TUN Virtual Interface)", "gVisor 虚拟网卡全量接管", IconId::Network, false, palette) }
                TunMasterCard
            ),
        ]
    }
}

fn single_master_card_scene(
    title: &'static str,
    desc: &'static str,
    icon: IconId,
    enabled: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let status_text = if enabled { "已开启" } else { "已关闭" };
    let status_color = if enabled {
        palette.success
    } else {
        palette.ink_dim
    };
    let dot_color = if enabled {
        palette.success
    } else {
        palette.border
    };

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                flex_grow: 1.0,
                flex_basis: px(280.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
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
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(icon, 16.0, palette.accent) } ),
                                ( Text({ title.to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S4),
                            }
                            Children [
                                (
                                    Node {
                                        width: px(6.0),
                                        height: px(6.0),
                                        border_radius: BorderRadius::all(Val::Px(3.0)),
                                    }
                                    BackgroundColor({ dot_color })
                                ),
                                ( Text({ status_text.to_owned() }) TextRole(Role::Caption) TextColor({ status_color }) ),
                            ]
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
                        ( Text({ desc.to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                        ( { pill_caption_scene(if enabled { "关闭".to_owned() } else { "开启".to_owned() }, enabled, palette) } ),
                    ]
                ),
            ]
        })],
        palette,
    )
}
