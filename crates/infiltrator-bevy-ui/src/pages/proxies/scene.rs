//! Scene owner.

use crate::pages::business_panel::{PanelKind, launcher_scene, panel_scene};
use crate::pages::proxies::{
    ProxiesPageRoot, ProxiesProjection, ProxiesScrollArea, ProxyGroup, ProxyNode,
};
use crate::pages::proxies_card;
use crate::pages::proxies_custom::custom_node_scene;
use crate::pages::proxies_identity::ProxyCardsContainer;
use crate::pages::proxies_virtual::{PROXIES_VIRTUAL_THRESHOLD, ProxiesVirtualNodes};
use crate::pages::proxy_inspection::inspection_scene;
use crate::pages::{proxy_group_order, proxy_probe_settings};
use crate::route::{PageRoot, Route};
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{FlexDirection, Node, Overflow, Val, percent, px};
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::gesture::{PullToRefreshState, pull_to_refresh_scene};
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::space;

/// The top-level Proxies page scene.
pub fn proxies_page(projection: &ProxiesProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = LocalizedText::new(
        "proxies_summary",
        vec![
            ("groups", projection.groups.len().to_string()),
            ("nodes", projection.total_nodes().to_string()),
        ],
    );
    let active_exit = projection.active_exit.clone();
    let test_status = if projection.testing {
        LocalizedText::plain("proxies_testing")
    } else {
        LocalizedText::plain("proxies_test_ready")
    };

    // BANDROID-009: large projections mount through the recycler (bounded
    // window + spacers). Small projections keep the full-mount vocabulary and
    // its stable-identity reconcile contract.
    let virtualized = projection.total_nodes() > PROXIES_VIRTUAL_THRESHOLD;

    let mut root_children: Vec<Box<dyn Scene>> = Vec::new();
    if virtualized {
        root_children.push(pull_to_refresh_scene(
            &PullToRefreshState::default(),
            palette,
        ));
        root_children.push(Box::new(header_card_scene(
            summary,
            active_exit,
            test_status,
            palette,
        )));
        root_children.push(Box::new(proxies_card::search_bar_card_scene(
            &projection.search_query,
            palette,
        )));
        root_children.push(Box::new(launcher_scene(PanelKind::CustomNode, palette)));
        root_children.push(Box::new(bsn! {
                Node {
                    width: percent(100),
                    min_height: px(0.0),
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(space::S16),
                    overflow: Overflow::scroll_y(),
                }
                ScrollArea ProxiesScrollArea
                Children [
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                    }
                    ProxiesVirtualNodes
                ]
        }) as Box<dyn Scene>);
    } else {
        let group_scenes: Vec<Box<dyn Scene>> = projection
            .groups
            .iter()
            .enumerate()
            .map(|(g_idx, group)| {
                Box::new(group_card_scene(g_idx, group, palette)) as Box<dyn Scene>
            })
            .collect();
        root_children.push(Box::new(bsn! {
                Node {
                    width: percent(100), height: percent(100),
                    min_height: px(0.0), flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(space::S16), overflow: Overflow::scroll_y(),
                }
                ScrollArea ProxiesScrollArea
                Children [
                @{ pull_to_refresh_scene(&PullToRefreshState::default(), palette) }
                --
                @{ header_card_scene(summary, active_exit, test_status, palette) }
                --
                @{ proxies_card::search_bar_card_scene(&projection.search_query, palette) }
                --
                @{ launcher_scene(PanelKind::CustomNode, palette) }
                --
                Node { width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(space::S16), flex_shrink: 0.0 }
                ProxyCardsContainer
                Children [ { group_scenes } ]
                ]
        }) as Box<dyn Scene>);
    }
    root_children.push(Box::new(panel_scene(
        PanelKind::CustomNode,
        Box::new(custom_node_scene(&projection.custom_node, palette)),
        palette,
    )) as Box<dyn Scene>);
    root_children.push(Box::new(inspection_scene(palette)) as Box<dyn Scene>);
    root_children.push(Box::new(proxy_probe_settings::modal(palette)) as Box<dyn Scene>);
    root_children.push(Box::new(proxy_group_order::modal(palette)) as Box<dyn Scene>);

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
                overflow: Overflow::clip(),
            }
            PageRoot(Route::Proxies)
            ProxiesPageRoot
            Children [ { root_children } ]
    }
}

pub(super) fn header_card_scene(
    summary: LocalizedText,
    active_exit: String,
    test_status: LocalizedText,
    palette: &UiPalette,
) -> impl Scene + use<> {
    proxies_card::header_card_scene(summary, active_exit, test_status, palette)
}

pub(super) fn group_card_scene(
    g_idx: usize,
    group: &ProxyGroup,
    palette: &UiPalette,
) -> impl Scene + use<> {
    proxies_card::group_card_scene(g_idx, group, palette)
}

pub fn proxy_node_scene(
    g_idx: usize,
    n_idx: usize,
    group_name: &str,
    node: &ProxyNode,
    palette: &UiPalette,
) -> impl Scene + use<> {
    proxies_card::proxy_node_scene(g_idx, n_idx, group_name, node, palette)
}
