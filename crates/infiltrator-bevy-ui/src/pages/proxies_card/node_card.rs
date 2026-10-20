//! The single proxy node card scene: flag, title, protocol & UDP chips,
//! feature chips, favorite pin, detail affordance and the latency trend tile.
//!
//! The same card vocabulary backs the full-mount path and the virtual-list
//! recycler, so the sized variant takes an explicit column width.

use crate::pages::proxies::{
    LatencySkeletonPulse, LatencyText, LatencyTrendIcon, NodeDetailButton, NodeFlagText,
    NodeNameText, NodePinButton, NodeProtoText, NodeUdpTag, ProxyNode, ProxyNodeButton,
    TestProxyGroupButton, latency_color,
};
use crate::pages::proxies_preferences::ProxyFavoriteStar;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, Display, FlexDirection, FlexWrap,
    JustifyContent, Node, Overflow, PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::gesture::{
    SwipeActionDrawer, SwipeContentContainer, SwipeToActionItem, SwipeToActionSpring,
};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_presentation::latency_projection::project_proxy_latency;
use infiltrator_presentation::proxies_page_model::proxy_protocol_chip;
use infiltrator_shared::country_flags::node_flag_emoji;
use lurker_core_contract::outbound::identity::OutboundId;

/// The selected proxy card's accent border width in baseline pixels.
const SELECTED_CARD_BORDER_WIDTH_PX: f32 = 1.5;

/// The proxy node card's minimum height in baseline pixels.
const PROXY_NODE_CARD_MIN_HEIGHT_PX: f32 = 58.0;

/// The swipe action drawer's fixed width in baseline pixels.
const SWIPE_ACTION_DRAWER_WIDTH_PX: f32 = 88.0;

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
        let proto_tag = proxy_protocol_chip(&node.node_type);
        vec![Box::new(bsn! {
            Node {
                padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                border_radius: BorderRadius::all(rem(radius::XS)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
            }
            ThemeBackgroundColor({ Token::Surface })
            Children [ Text(proto_tag) NodeProtoText { group_idx: g_idx, node_idx: n_idx } TextRole(Role::Caption) ]
        })]
    };

    let (bg, border_color, border_width) = if node.selected {
        // Selected cards carry a 1.5px accent border; no token carries that
        // width, so the baseline magnitude travels through the rem channel.
        (
            palette.accent_container,
            palette.accent,
            rem(SELECTED_CARD_BORDER_WIDTH_PX),
        )
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
                    // 3px chip corner: no radius token carries 3px.
                    border_radius: BorderRadius::MAX,
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
                        // 3px chip corner: no radius token carries 3px.
                        border_radius: BorderRadius::MAX,
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
                min_height: rem(PROXY_NODE_CARD_MIN_HEIGHT_PX),
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
                    min_height: rem(PROXY_NODE_CARD_MIN_HEIGHT_PX),
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
                        border_radius: BorderRadius::all(rem(radius::XS)),
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
                    width: rem(SWIPE_ACTION_DRAWER_WIDTH_PX),
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
