//! The Proxies page (代理策略): proxy groups and nodes, latency testing,
//! group selection, collapsible strategy groups, and active outbound routing.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProxiesLine`],
//! [`NodeNameText`], [`NodeFlagText`], [`NodeProtoText`], [`LatencyText`],
//! [`GroupCurrentText`], [`GroupFoldText`]).
//! [`ProxiesPagePlugin`] registers [`apply_proxies_projection`] and action observers
//! once at product assembly. When [`ProxiesProjectionUpdated`] fires,
//! texts, latency inks, group expansion, and selection states restamp in place
//! without tree rebuilds.

#[path = "proxies_query_access.rs"]
pub mod query_access;
use self::query_access::ProxyActionControls;
use infiltrator_composition::demo_identities::{AUTO, HK_PRIMARY, JP, PROXIES, SG, STREAMING, US};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::proxies_custom::{on_custom_node_action_activated, sync_custom_node_studio};
use crate::pages::proxies_filter;
use crate::pages::proxies_refresh::apply_proxies_projection;
use bevy::app::{App, Plugin};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::ui::prelude::{ComputedNode, Node, Val};
/// Root marker on the Proxies page scene.
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::fluid_grid::{FluidCardGrid, compute_ideal_column_layout};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::theme::{Breakpoint, space};
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;
use infiltrator_contract::proxies::{ProxyGroupClassification, ProxySortOrder};
use infiltrator_contract::search_text::SearchTextRun;
use std::collections::BTreeMap;

pub mod scene;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ProxiesPageRoot;

/// Native vertical scroll surface for all proxy groups and controls.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxiesScrollArea;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxiesLine(pub ProxiesLineKind);

/// Different text lines on the proxies page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxiesLineKind {
    /// Overview summary: total groups & total nodes count.
    #[default]
    Summary,
    /// Active exit node name.
    ActiveExit,
    /// Latency test status text.
    TestStatus,
}

/// Marker for a proxy node's latency display text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatencyText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy node's name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeNameText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy node's country flag text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeFlagText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy node's protocol badge text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeProtoText {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for a proxy group's current selection label text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupCurrentText(pub usize);

/// Marker for a proxy group's fold/expand toggle button text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupFoldText(pub usize);

/// Marker for a proxy group's nodes container.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupNodesContainer(pub usize);

/// Marker for the "Test All Groups" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TestAllProxiesButton;

/// Marker for a proxy group speed test button ("组测速").
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct TestProxyGroupButton {
    pub group_idx: usize,
    pub group_name: String,
}

/// Marker for a proxy group fold toggle button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupFoldButton {
    pub group_idx: usize,
    pub group_name: String,
}

/// Marker and target information for a proxy node selection button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyNodeButton {
    pub group_idx: usize,
    pub node_idx: usize,
    pub group_name: String,
    pub node_name: String,
}

/// Marker for the "Add Node" button (+ 添加节点).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AddCustomNodeButton;

/// Marker for the view mode toggle button (网格/列表视图).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToggleViewModeButton;
/// Marker for the "Filter Alive" toggle switch (只看可用).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FilterAliveToggle;

/// Marker for the sort mode pills.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProxySortMode {
    #[default]
    LatencyAsc,
    LatencyDesc,
    NameAsc,
    NameDesc,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProxySortPill(pub ProxySortMode);

impl From<ProxySortMode> for ProxySortOrder {
    fn from(mode: ProxySortMode) -> Self {
        match mode {
            ProxySortMode::LatencyAsc => Self::LatencyAsc,
            ProxySortMode::LatencyDesc => Self::LatencyDesc,
            ProxySortMode::NameAsc => Self::NameAsc,
            ProxySortMode::NameDesc => Self::NameDesc,
        }
    }
}

/// Marker for favorite pin icon / button on proxy node cards.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
#[require(Button)]
pub struct NodePinButton {
    pub group_idx: usize,
    pub node_idx: usize,
    pub node_name: String,
}

/// Marker for latency trend / waveform icon on proxy node cards.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatencyTrendIcon {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for capability UDP tag on proxy node cards.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeUdpTag {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Marker for the node detail button (DUAL-04-11).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeDetailButton {
    pub group_idx: usize,
    pub node_idx: usize,
    pub node_name: String,
}

/// Marker for proxy group move-up button (DUAL-04-12).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupMoveUpButton {
    pub group_idx: usize,
    pub group_name: String,
}

/// Marker for proxy group move-down button (DUAL-04-12).
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyGroupMoveDownButton {
    pub group_idx: usize,
    pub group_name: String,
}

/// Marker for resetting custom group order (DUAL-04-12).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResetProxyGroupOrderButton;

/// Marker for latency test pulsing skeleton placeholder (DUAL-04-14).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LatencySkeletonPulse {
    pub group_idx: usize,
    pub node_idx: usize,
}

/// Canonical display name for proxy protocols (Shadowsocks, Vless, VMess, Trojan, Hysteria2).
pub fn format_protocol_chip(raw_type: &str) -> String {
    proxies_filter::format_protocol_chip(raw_type)
}

/// Resolve latency text color from tier and palette tokens.
pub fn latency_color(tier: LatencyBand, palette: &UiPalette) -> Color {
    match tier {
        LatencyBand::Fast => palette.success,
        LatencyBand::Medium => palette.warning,
        LatencyBand::Slow => palette.danger,
        LatencyBand::NotObserved | LatencyBand::UnconfirmedZero => palette.ink_dim,
    }
}

/// A single proxy node snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyNode {
    pub name: String,
    pub node_type: String,
    pub delay_ms: Option<u32>,
    pub selected: bool,
    pub favorite: bool,
    pub features: Vec<String>,
}

/// A proxy group snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyGroup {
    pub name: String,
    pub group_type: String,
    pub classification: ProxyGroupClassification,
    pub current: String,
    pub expanded: bool,
    pub proxies: Vec<ProxyNode>,
}

/// Snapshot of the entire Proxies domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxiesProjection {
    pub name_runs: BTreeMap<String, Vec<SearchTextRun>>,
    pub search_query: String,
    pub groups: Vec<ProxyGroup>,
    pub testing: bool,
    pub filter_alive: bool,
    pub compact_view: bool,
    pub active_exit: String,
    /// DUAL-05: the shared custom-node protocol studio. Both surfaces render
    /// this single projection; Bevy keeps no second protocol fact source.
    pub custom_node: ProtocolStudioSnapshot,
}

impl ProxiesProjection {
    /// Believable demo fixture for the Proxies page.
    pub fn demo() -> Self {
        Self {
            name_runs: BTreeMap::new(),
            search_query: String::new(),
            active_exit: HK_PRIMARY.to_owned(),
            testing: false,
            filter_alive: false,
            compact_view: false,
            custom_node: Default::default(),
            groups: vec![
                ProxyGroup {
                    name: PROXIES.to_owned(),
                    group_type: "Selector".to_owned(),
                    classification: ProxyGroupClassification::Selector,
                    current: HK_PRIMARY.to_owned(),
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            name: HK_PRIMARY.to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            selected: true,
                            favorite: true,
                            features: vec!["UDP".to_owned(), "TFO".to_owned()],
                        },
                        ProxyNode {
                            name: JP.to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            selected: false,
                            favorite: false,
                            features: vec!["Vision".to_owned()],
                        },
                        ProxyNode {
                            name: SG.to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            selected: false,
                            favorite: false,
                            features: vec!["Reality".to_owned()],
                        },
                        ProxyNode {
                            name: US.to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            selected: false,
                            favorite: false,
                            features: vec!["UDP".to_owned(), "Reality".to_owned()],
                        },
                    ],
                },
                ProxyGroup {
                    name: AUTO.to_owned(),
                    group_type: "URLTest".to_owned(),
                    classification: ProxyGroupClassification::UrlTest,
                    current: HK_PRIMARY.to_owned(),
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            name: HK_PRIMARY.to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            selected: true,
                            favorite: true,
                            features: vec!["UDP".to_owned(), "TFO".to_owned()],
                        },
                        ProxyNode {
                            name: JP.to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            selected: false,
                            favorite: false,
                            features: vec!["Vision".to_owned()],
                        },
                    ],
                },
                ProxyGroup {
                    name: STREAMING.to_owned(),
                    group_type: "Selector".to_owned(),
                    classification: ProxyGroupClassification::Selector,
                    current: SG.to_owned(),
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            name: SG.to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            selected: true,
                            favorite: false,
                            features: vec!["Reality".to_owned()],
                        },
                        ProxyNode {
                            name: US.to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            selected: false,
                            favorite: false,
                            features: vec!["UDP".to_owned(), "Reality".to_owned()],
                        },
                    ],
                },
            ],
        }
    }

    /// Total count of proxy nodes across all groups.
    pub fn total_nodes(&self) -> usize {
        self.groups.iter().map(|g| g.proxies.len()).sum()
    }
}

/// The typed event dispatched when proxy data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct ProxiesProjectionUpdated(pub ProxiesProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastProxiesProjection(pub Option<ProxiesProjection>);

// ---- Scene constructors ---------------------------------------------------

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct ProxiesPagePlugin;

impl Plugin for ProxiesPagePlugin {
    fn build(&self, app: &mut App) {
        // DUAL-05: the listeners read the last projection (e.g. the custom-node
        // save submits the shared draft it carries), so the resource must exist
        // the moment the page is mounted instead of being read as `None` forever.
        app.init_resource::<LastProxiesProjection>();
        app.init_resource::<crate::pages::proxies_virtual::ProxiesVirtualState>();
        app.add_observer(apply_proxies_projection);
        app.add_observer(on_proxies_action_activated);
        app.add_observer(on_custom_node_action_activated);
        // DUAL-05: the custom-node studio is re-covered from the same event in a
        // separate observer so it never shares the page restamp query set.
        app.add_observer(sync_custom_node_studio);
    }
}

/// Reflow proxy node cards to the shared tier column count (1 / 2 / 3 / 4).
/// When container dimensions are measured via [`ComputedNode`], the exact card
/// width is computed via [`compute_ideal_column_layout`] to fill 100% of the
/// container width symmetrically. Otherwise, it falls back to the authoritative
/// percentage basis.
pub fn sync_proxies_node_columns(
    ctx: Option<Res<ResponsiveContext>>,
    last: Option<Res<LastProxiesProjection>>,
    containers: Query<(Option<&ComputedNode>, &Children), With<GroupNodesContainer>>,
    mut nodes: Query<&mut Node, With<ProxyNodeButton>>,
) {
    let Some(ctx) = ctx else {
        return;
    };
    let columns = if last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .is_some_and(|projection| projection.compact_view)
    {
        1
    } else {
        match ctx.breakpoint {
            Breakpoint::Compact => 1,
            Breakpoint::Medium => 2,
            Breakpoint::Expanded => 3,
            Breakpoint::Ultra => 4,
        }
    };
    let fallback_width = Val::Percent(FluidCardGrid::wrapped_item_percent(columns));

    if containers.is_empty() {
        for mut node in &mut nodes {
            if node.width != fallback_width {
                node.width = fallback_width;
            }
        }
    } else {
        for (computed, children) in &containers {
            let target_width = if let Some(computed) = computed {
                let measured_w = computed.size().x * computed.inverse_scale_factor();
                if measured_w > 100.0 {
                    let layout = compute_ideal_column_layout(measured_w, 220.0, space::S8, columns);
                    Val::Px(layout.item_width_px)
                } else {
                    fallback_width
                }
            } else {
                fallback_width
            };

            for child in children.iter() {
                if let Ok(mut node) = nodes.get_mut(*child)
                    && node.width != target_width
                {
                    node.width = target_width;
                }
            }
        }
    }
}

pub(crate) fn on_proxies_action_activated(
    activate: On<Activate>,
    handle: Option<Res<CommandSinkHandle>>,
    last: Option<Res<LastProxiesProjection>>,
    targets: ProxyActionControls,
) {
    let ProxyActionControls {
        test_all_buttons,
        test_group_buttons,
        fold_buttons,
        node_buttons,
        filter_alive_toggles,
        sort_pills,
        pin_buttons,
        toggle_view_buttons,
    } = targets;

    let Some(handle) = handle else {
        return;
    };
    if test_all_buttons.contains(activate.entity) {
        handle.submit(UiCommand::TestAllProxyGroups);
    } else if let Ok(btn) = test_group_buttons.get(activate.entity) {
        if !last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .is_some_and(|projection| {
                projection
                    .groups
                    .iter()
                    .any(|group| group.name == btn.group_name)
            })
        {
            return;
        }
        handle.submit(UiCommand::TestProxyGroup {
            group: btn.group_name.clone(),
        });
    } else if let Ok(btn) = fold_buttons.get(activate.entity) {
        if !last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .is_some_and(|projection| {
                projection
                    .groups
                    .iter()
                    .any(|group| group.name == btn.group_name)
            })
        {
            return;
        }
        handle.submit(UiCommand::ToggleProxyGroupExpand {
            group: btn.group_name.clone(),
        });
    } else if let Ok(btn) = node_buttons.get(activate.entity) {
        if !last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .is_some_and(|projection| {
                projection.groups.iter().any(|group| {
                    group.name == btn.group_name
                        && group.proxies.iter().any(|node| node.name == btn.node_name)
                })
            })
        {
            return;
        }
        handle.submit(UiCommand::SelectProxyNode {
            group: btn.group_name.clone(),
            node: btn.node_name.clone(),
        });
    } else if filter_alive_toggles.contains(activate.entity) {
        if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
            handle.submit(UiCommand::ToggleFilterAlive(!projection.filter_alive));
        }
    } else if let Ok(pill) = sort_pills.get(activate.entity) {
        handle.submit(UiCommand::SetProxySortOrder(pill.0.into()));
    } else if let Ok(pin) = pin_buttons.get(activate.entity) {
        if !last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .is_some_and(|projection| {
                projection
                    .groups
                    .iter()
                    .flat_map(|group| &group.proxies)
                    .any(|node| node.name == pin.node_name)
            })
        {
            return;
        }
        handle.submit(UiCommand::ToggleFavoriteProxy(pin.node_name.clone()));
    } else if toggle_view_buttons.contains(activate.entity)
        && let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref())
    {
        handle.submit(UiCommand::SetProxyCompactView(!projection.compact_view));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_application::latency_projection::project_proxy_latency;
    use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
    use infiltrator_shared::country_flags::node_flag_emoji;

    fn native_caption(delay: Option<u32>) -> (String, LatencyBand) {
        let caption = project_proxy_latency(delay);
        (
            LocalizedText::new(caption.key, caption.params).render(&UiLocale::new("zh-CN")),
            caption.band,
        )
    }

    #[test]
    fn format_latency_tiers() {
        let (s1, t1) = native_caption(Some(35));
        assert_eq!(s1, "35 ms");
        assert_eq!(t1, LatencyBand::Fast);

        let (s2, t2) = native_caption(Some(220));
        assert_eq!(s2, "220 ms");
        assert_eq!(t2, LatencyBand::Medium);

        let (s3, t3) = native_caption(Some(850));
        assert_eq!(s3, "850 ms");
        assert_eq!(t3, LatencyBand::Slow);

        let (s4, t4) = native_caption(Some(0));
        assert_eq!(s4, "0 ms（结果未区分）");
        assert_eq!(t4, LatencyBand::UnconfirmedZero);

        let (s5, t5) = native_caption(None);
        assert_eq!(s5, "尚未测速");
        assert_eq!(t5, LatencyBand::NotObserved);
    }

    #[test]
    fn demo_fixture_counts() {
        let proj = ProxiesProjection::demo();
        assert_eq!(proj.groups.len(), 3);
        assert_eq!(proj.total_nodes(), 8);
        assert_eq!(proj.active_exit, "HK-01");
        assert_eq!(proj.groups[0].name, "PROXIES");
        assert_eq!(proj.groups[0].group_type, "Selector");
        assert_eq!(proj.groups[0].proxies.len(), 4);
        assert_eq!(proj.groups[0].proxies[0].name, "HK-01");
        assert_eq!(proj.groups[0].proxies[0].delay_ms, Some(38));
        assert!(proj.groups[0].expanded);
    }

    #[test]
    fn test_node_flag_extraction() {
        assert_eq!(node_flag_emoji("🇭🇰 香港 01 · BGP 专线"), "🇭🇰");
        assert_eq!(node_flag_emoji("HK-IEPL-01"), "🇭🇰");
        assert_eq!(node_flag_emoji("🇯🇵 日本东京 02 · 极速"), "🇯🇵");
        assert_eq!(node_flag_emoji("JP-Tokyo-01"), "🇯🇵");
        assert_eq!(node_flag_emoji("🇸🇬 新加坡 01 · Anycast"), "🇸🇬");
        assert_eq!(node_flag_emoji("SG-01"), "🇸🇬");
        assert_eq!(node_flag_emoji("🇺🇸 美国硅谷 01 · 4K"), "🇺🇸");
        assert_eq!(node_flag_emoji("US-Silicon-Valley"), "🇺🇸");
        assert_eq!(node_flag_emoji("Taiwan Premium"), "🇹🇼");
        assert_eq!(node_flag_emoji("Korea Seoul 01"), "🇰🇷");
        assert_eq!(node_flag_emoji("Unknown Node"), "🌐");
    }

    #[test]
    fn test_matches_proxy_filter() {
        use crate::pages::proxies_filter::matches_proxy_filter;
        let node = ProxyNode {
            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
            node_type: "Shadowsocks".to_owned(),
            delay_ms: Some(38),
            selected: true,
            favorite: true,
            features: vec!["UDP".to_owned(), "TFO".to_owned()],
        };

        assert!(matches_proxy_filter(&node, ""));
        assert!(matches_proxy_filter(&node, "香港"));
        assert!(matches_proxy_filter(&node, "hk"));
        assert!(matches_proxy_filter(&node, "xg"));
        assert!(matches_proxy_filter(&node, "Shadowsocks"));
        assert!(matches_proxy_filter(&node, "ss"));
        assert!(matches_proxy_filter(&node, "<100"));
        assert!(!matches_proxy_filter(&node, "<30"));
        assert!(matches_proxy_filter(&node, ">20"));
        assert!(!matches_proxy_filter(&node, ">50"));
        assert!(!matches_proxy_filter(&node, "日本"));
        assert!(!matches_proxy_filter(&node, "jp"));
    }
}
