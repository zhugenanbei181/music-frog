//! The strategy group card scene: group identity, current node, group test
//! trigger, reorder/fold actions and the collapsible node rows.

use super::node_card::{ProxyNodeSceneContext, proxy_node_scene};
use crate::pages::proxies::{
    GroupCurrentText, GroupFoldText, GroupNodesContainer, ProxyGroup, ProxyGroupFoldButton,
    ProxyGroupMoveDownButton, ProxyGroupMoveUpButton, ProxyNode, TestProxyGroupButton,
};
use crate::pages::proxies_identity::{ProxyGroupIdentity, ProxyGroupOutboundIdentity};
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::button::{ButtonSize, ButtonVariant, button_with_label_scene};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_contract::design_resources;
use infiltrator_contract::proxies::ProxyGroupClassification;

/// The group header action button's minimum height in baseline pixels.
/// The group header's reorder icon plate size, in baseline pixels.
const GROUP_HEADER_ICON_SIZE_PX: f32 = 14.0;

/// The group's product label.
///
/// Every kind is worded by the shared copy table, so the label is the
/// reader's language: the contract's own product label is a Chinese
/// literal and would leak into the English surface.
fn group_kind_label(classification: ProxyGroupClassification) -> LocalizedText {
    match classification {
        ProxyGroupClassification::Selector => {
            LocalizedText::plain("outbound_availability_selectable")
        }
        ProxyGroupClassification::UrlTest => {
            LocalizedText::plain("outbound_availability_automatic")
        }
        ProxyGroupClassification::Fallback => LocalizedText::plain("proxies_group_kind_fallback"),
        ProxyGroupClassification::LoadBalance => {
            LocalizedText::plain("proxies_group_kind_load_balance")
        }
        ProxyGroupClassification::Relay => LocalizedText::plain("proxies_group_kind_relay"),
    }
}

/// Renders a strategy group card with collapsible nodes and group test trigger.
pub fn group_card_scene(
    g_idx: usize,
    group: &ProxyGroup,
    width: Val,
    palette: &UiPalette,
) -> impl Scene + use<> {
    group_card_scene_impl(g_idx, group, width, palette, true)
}

/// Renders a strategy group card header only: the same identity, controls and
/// fold affordances, but no node children. The virtual node list owns node
/// mounting, so its group rows reuse this header vocabulary.
pub fn group_header_scene(
    g_idx: usize,
    group: &ProxyGroup,
    palette: &UiPalette,
) -> impl Scene + use<> {
    group_card_scene_impl(g_idx, group, percent(100), palette, false)
}

fn group_card_scene_impl(
    g_idx: usize,
    group: &ProxyGroup,
    width: Val,
    palette: &UiPalette,
    with_nodes: bool,
) -> impl Scene + use<> {
    let title = group.name.clone();
    let kind_label = group_kind_label(group.classification);
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
                    ProxyNodeSceneContext {
                        group_idx: g_idx,
                        node_idx: n_idx,
                        group_name: &group_name,
                        collection_id: group.collection_id.clone(),
                        selection_available: group.outbound_selection_available,
                    },
                    node,
                    width,
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
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: rem(space::S8),
                            }
                            ProxyGroupIdentity({ group.name.clone() })
                            ProxyGroupOutboundIdentity({ group.collection_id.clone() })
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S8),
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                }
                                Children [
                                    Text(title) TextRole(design_resources::TextRole::BodyStrong)
                                    --
                                    Node {
                                        padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                                        border_radius: BorderRadius::all(rem(radius::XS)),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Center,
                                        flex_shrink: 0.0,
                                    }
                                    ThemeBackgroundColor({ Token::AccentContainer })
                                    Children [
                                        LocalizedText { .. { kind_label } } TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::Ink })
                                    ]
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S8),
                                }
                                Children [
                                    LocalizedText { .. { current_label } } GroupCurrentText(g_idx) TextRole(design_resources::TextRole::Caption)
                                    --
                                    @{ button_with_label_scene(
                                        bsn! { LocalizedText::plain("proxies_test_group") TextRole(design_resources::TextRole::Caption) },
                                        ButtonVariant::Default,
                                        ButtonSize::Sm,
                                        palette,
                                    ) }
                                    TestProxyGroupButton {
                                        group_idx: { g_idx },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    --
                                    @{ button_with_label_scene(
                                        icon_scene(IconId::ArrowUp, GROUP_HEADER_ICON_SIZE_PX, palette.ink_dim),
                                        ButtonVariant::Default,
                                        ButtonSize::Sm,
                                        palette,
                                    ) }
                                    ProxyGroupMoveUpButton {
                                        group_idx: { g_idx },
                                        group_name: { group_name.clone() },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    --
                                    @{ button_with_label_scene(
                                        icon_scene(IconId::ArrowDown, GROUP_HEADER_ICON_SIZE_PX, palette.ink_dim),
                                        ButtonVariant::Default,
                                        ButtonSize::Sm,
                                        palette,
                                    ) }
                                    ProxyGroupMoveDownButton {
                                        group_idx: { g_idx },
                                        group_name: { group_name.clone() },
                                        collection_id: { group.collection_id.clone() },
                                    }
                                    --
                                    @{ button_with_label_scene(
                                        bsn! { LocalizedText { .. { fold_label } } GroupFoldText(g_idx) TextRole(design_resources::TextRole::Caption) },
                                        ButtonVariant::Default,
                                        ButtonSize::Sm,
                                        palette,
                                    ) }
                                    ProxyGroupFoldButton {
                                        group_idx: { g_idx },
                                        group_name: { group.name.clone() },
                                        collection_id: { group.collection_id.clone() },
                                    }
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
