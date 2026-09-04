//! The Proxies page (代理策略): proxy groups and nodes, latency testing,
//! group selection, collapsible strategy groups, and active outbound routing.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProxiesLine`],
//! [`NodeNameText`], [`NodeFlagText`], [`NodeProtoText`], [`LatencyText`],
//! [`GroupCurrentText`], [`GroupFoldText`]).
//! The page self-registers [`apply_proxies_projection`] and action observers
//! once per world via [`ProxiesPageRoot`]. When [`ProxiesProjectionUpdated`] fires,
//! texts, latency inks, group expansion, and selection states restamp in place
//! without tree rebuilds.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, Display, FlexDirection, Node, Overflow, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::route::{PageRoot, Route};

/// Root marker on the Proxies page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_proxies_page)]
pub struct ProxiesPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct ProxiesPageBound;

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

/// Marker for the delay test URL indicator ("测试地址").
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DelayTestUrlIndicator;

/// Marker for favorite pin icon / button on proxy node cards.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
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

/// Classification of latency for color coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LatencyTier {
    Fast,
    Medium,
    Slow,
    Timeout,
}

/// Convenience helper to return the flag emoji for a node name, or 🌐 if unrecognized.
pub fn node_flag(name: &str) -> &'static str {
    crate::pages::proxies_filter::node_flag(name)
}

/// Canonical display name for proxy protocols (Shadowsocks, Vless, VMess, Trojan, Hysteria2).
pub fn format_protocol_chip(raw_type: &str) -> String {
    crate::pages::proxies_filter::format_protocol_chip(raw_type)
}

/// Multi-mode fuzzy search and pinyin/abbreviation filter (BEVY-GAP-032).
pub fn matches_proxy_filter(node: &ProxyNode, query: &str) -> bool {
    crate::pages::proxies_filter::matches_proxy_filter(node, query)
}

pub fn format_latency(delay_ms: Option<u32>) -> (String, LatencyTier) {
    match delay_ms {
        Some(0) => ("超时".to_owned(), LatencyTier::Timeout),
        Some(ms) if ms < 100 => (format!("{ms} ms"), LatencyTier::Fast),
        Some(ms) if ms < 250 => (format!("{ms} ms"), LatencyTier::Medium),
        Some(ms) => (format!("{ms} ms"), LatencyTier::Slow),
        None => ("未测速".to_owned(), LatencyTier::Timeout),
    }
}

/// Resolve latency text color from tier and palette tokens.
pub fn latency_color(tier: LatencyTier, palette: &UiPalette) -> Color {
    match tier {
        LatencyTier::Fast => palette.success,
        LatencyTier::Medium => palette.warning,
        LatencyTier::Slow => palette.danger,
        LatencyTier::Timeout => palette.danger,
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
    pub current: String,
    pub expanded: bool,
    pub proxies: Vec<ProxyNode>,
}

/// Snapshot of the entire Proxies domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxiesProjection {
    pub groups: Vec<ProxyGroup>,
    pub testing: bool,
    pub active_exit: String,
}

impl ProxiesProjection {
    /// Believable demo fixture for the Proxies page.
    pub fn demo() -> Self {
        Self {
            active_exit: "🇭🇰 香港 01 · BGP 专线".to_owned(),
            testing: false,
            groups: vec![
                ProxyGroup {
                    name: "节点选择 (PROXIES)".to_owned(),
                    group_type: "Selector".to_owned(),
                    current: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            selected: true,
                            favorite: true,
                            features: vec!["UDP".to_owned(), "TFO".to_owned()],
                        },
                        ProxyNode {
                            name: "🇯🇵 日本东京 02 · 极速".to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            selected: false,
                            favorite: false,
                            features: vec!["Vision".to_owned()],
                        },
                        ProxyNode {
                            name: "🇸🇬 新加坡 01 · Anycast".to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            selected: false,
                            favorite: false,
                            features: vec!["Reality".to_owned()],
                        },
                        ProxyNode {
                            name: "🇺🇸 美国硅谷 01 · 4K".to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            selected: false,
                            favorite: false,
                            features: vec!["UDP".to_owned(), "Reality".to_owned()],
                        },
                    ],
                },
                ProxyGroup {
                    name: "自动选择 (AUTO)".to_owned(),
                    group_type: "URLTest".to_owned(),
                    current: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            selected: true,
                            favorite: true,
                            features: vec!["UDP".to_owned(), "TFO".to_owned()],
                        },
                        ProxyNode {
                            name: "🇯🇵 日本东京 02 · 极速".to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            selected: false,
                            favorite: false,
                            features: vec!["Vision".to_owned()],
                        },
                    ],
                },
                ProxyGroup {
                    name: "国外媒体 (STREAMING)".to_owned(),
                    group_type: "Selector".to_owned(),
                    current: "🇸🇬 新加坡 01 · Anycast".to_owned(),
                    expanded: true,
                    proxies: vec![
                        ProxyNode {
                            name: "🇸🇬 新加坡 01 · Anycast".to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            selected: true,
                            favorite: false,
                            features: vec!["Reality".to_owned()],
                        },
                        ProxyNode {
                            name: "🇺🇸 美国硅谷 01 · 4K".to_owned(),
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

/// The top-level Proxies page scene.
pub fn proxies_page(projection: &ProxiesProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "代理策略 · 共 {} 个策略组 ({} 个节点)",
        projection.groups.len(),
        projection.total_nodes()
    );
    let active_exit = projection.active_exit.clone();
    let test_status = if projection.testing {
        "正在全面测速中...".to_owned()
    } else {
        "测速就绪".to_owned()
    };

    let group_scenes: Vec<Box<dyn Scene>> = projection
        .groups
        .iter()
        .enumerate()
        .map(|(g_idx, group)| Box::new(group_card_scene(g_idx, group, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            height: percent(100),
            min_height: px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Proxies)
        ProxiesPageRoot
        Children [
            ( { header_card_scene(summary, active_exit, test_status, palette) } ),
            ( { search_bar_card_scene(palette) } ),
            ( { crate::pages::proxies_custom::custom_node_scene(palette) } ),
            { group_scenes },
        ]
    }
}

fn search_bar_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    crate::pages::proxies_card::search_bar_card_scene(palette)
}

fn header_card_scene(
    summary: String,
    active_exit: String,
    test_status: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    crate::pages::proxies_card::header_card_scene(summary, active_exit, test_status, palette)
}

fn group_card_scene(g_idx: usize, group: &ProxyGroup, palette: &UiPalette) -> impl Scene + use<> {
    crate::pages::proxies_card::group_card_scene(g_idx, group, palette)
}

pub fn proxy_node_scene(
    g_idx: usize,
    n_idx: usize,
    group_name: &str,
    node: &ProxyNode,
    palette: &UiPalette,
) -> impl Scene + use<> {
    crate::pages::proxies_card::proxy_node_scene(g_idx, n_idx, group_name, node, palette)
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_proxies_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<ProxiesPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(ProxiesPageBound);
    commands.add_observer(apply_proxies_projection);
    commands.add_observer(on_proxies_action_activated);
}

pub(crate) fn on_proxies_action_activated(
    activate: On<Activate>,
    test_all_buttons: Query<(), With<TestAllProxiesButton>>,
    test_group_buttons: Query<&TestProxyGroupButton>,
    fold_buttons: Query<&ProxyGroupFoldButton>,
    node_buttons: Query<&ProxyNodeButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if test_all_buttons.contains(activate.entity) {
        handle.submit(UiCommand::TestAllProxyGroups);
    } else if let Ok(btn) = test_group_buttons.get(activate.entity) {
        handle.submit(UiCommand::TestProxyGroup {
            group: btn.group_name.clone(),
        });
    } else if let Ok(btn) = fold_buttons.get(activate.entity) {
        handle.submit(UiCommand::ToggleProxyGroupExpand {
            group: btn.group_name.clone(),
        });
    } else if let Ok(btn) = node_buttons.get(activate.entity) {
        handle.submit(UiCommand::SelectProxyNode {
            group: btn.group_name.clone(),
            node: btn.node_name.clone(),
        });
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_proxies_projection(
    update: On<ProxiesProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastProxiesProjection>>,
    mut lines: Query<
        (&mut Text, &ProxiesLine),
        (
            With<ProxiesLine>,
            Without<LatencyText>,
            Without<NodeNameText>,
            Without<GroupCurrentText>,
            Without<GroupFoldText>,
            Without<NodeFlagText>,
            Without<NodeProtoText>,
        ),
    >,
    mut group_currents: Query<
        (&mut Text, &GroupCurrentText),
        (
            With<GroupCurrentText>,
            Without<ProxiesLine>,
            Without<LatencyText>,
            Without<NodeNameText>,
            Without<GroupFoldText>,
            Without<NodeFlagText>,
            Without<NodeProtoText>,
        ),
    >,
    mut group_folds: Query<
        (&mut Text, &GroupFoldText),
        (
            With<GroupFoldText>,
            Without<ProxiesLine>,
            Without<LatencyText>,
            Without<NodeNameText>,
            Without<GroupCurrentText>,
            Without<NodeFlagText>,
            Without<NodeProtoText>,
        ),
    >,
    mut group_containers: Query<(&mut Node, &GroupNodesContainer)>,
    mut latencies: Query<
        (&mut Text, &mut TextColor, &LatencyText),
        (
            With<LatencyText>,
            Without<ProxiesLine>,
            Without<NodeNameText>,
            Without<GroupCurrentText>,
            Without<GroupFoldText>,
            Without<NodeFlagText>,
            Without<NodeProtoText>,
        ),
    >,
    mut node_names: Query<
        (&mut Text, &NodeNameText),
        (
            With<NodeNameText>,
            Without<ProxiesLine>,
            Without<LatencyText>,
            Without<GroupCurrentText>,
            Without<GroupFoldText>,
            Without<NodeFlagText>,
            Without<NodeProtoText>,
        ),
    >,
    mut node_flags: Query<
        (&mut Text, &NodeFlagText),
        (
            With<NodeFlagText>,
            Without<ProxiesLine>,
            Without<LatencyText>,
            Without<NodeNameText>,
            Without<GroupCurrentText>,
            Without<GroupFoldText>,
            Without<NodeProtoText>,
        ),
    >,
    mut node_protos: Query<
        (&mut Text, &NodeProtoText),
        (
            With<NodeProtoText>,
            Without<ProxiesLine>,
            Without<LatencyText>,
            Without<NodeNameText>,
            Without<GroupCurrentText>,
            Without<GroupFoldText>,
            Without<NodeFlagText>,
        ),
    >,
    mut node_buttons: Query<(
        &mut BackgroundColor,
        &mut BorderColor,
        &mut ControlVisual,
        &mut ProxyNodeButton,
    )>,
    mut test_group_buttons: Query<&mut TestProxyGroupButton>,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            ProxiesLineKind::Summary => {
                text.0 = format!(
                    "代理策略 · 共 {} 个策略组 ({} 个节点)",
                    projection.groups.len(),
                    projection.total_nodes()
                );
            }
            ProxiesLineKind::ActiveExit => {
                text.0 = projection.active_exit.clone();
            }
            ProxiesLineKind::TestStatus => {
                text.0 = if projection.testing {
                    "正在全面测速中...".to_owned()
                } else {
                    "测速就绪".to_owned()
                };
            }
        }
    }

    for (mut text, marker) in &mut group_currents {
        if let Some(group) = projection.groups.get(marker.0) {
            text.0 = format!("选中: {}", group.current);
        }
    }

    for (mut text, marker) in &mut group_folds {
        if let Some(group) = projection.groups.get(marker.0) {
            text.0 = if group.expanded {
                "折叠 ▼".to_owned()
            } else {
                "展开 ▶".to_owned()
            };
        }
    }

    for (mut node_layout, marker) in &mut group_containers {
        if let Some(group) = projection.groups.get(marker.0) {
            node_layout.display = if group.expanded {
                Display::Flex
            } else {
                Display::None
            };
        }
    }

    for (mut text, mut color, marker) in &mut latencies {
        if let Some(group) = projection.groups.get(marker.group_idx)
            && let Some(node) = group.proxies.get(marker.node_idx)
        {
            let (str_val, tier) = format_latency(node.delay_ms);
            text.0 = str_val;
            color.0 = latency_color(tier, &palette);
        }
    }

    for (mut text, marker) in &mut node_names {
        if let Some(group) = projection.groups.get(marker.group_idx)
            && let Some(node) = group.proxies.get(marker.node_idx)
        {
            text.0 = node.name.clone();
        }
    }

    for (mut text, marker) in &mut node_flags {
        if let Some(group) = projection.groups.get(marker.group_idx)
            && let Some(node) = group.proxies.get(marker.node_idx)
        {
            text.0 = node_flag(&node.name).to_owned();
        }
    }

    for (mut text, marker) in &mut node_protos {
        if let Some(group) = projection.groups.get(marker.group_idx)
            && let Some(node) = group.proxies.get(marker.node_idx)
        {
            text.0 = format_protocol_chip(&node.node_type);
        }
    }

    for (mut bg, mut border, mut visual, mut btn) in &mut node_buttons {
        if let Some(group) = projection.groups.get(btn.group_idx)
            && let Some(node) = group.proxies.get(btn.node_idx)
        {
            btn.group_name = group.name.clone();
            btn.node_name = node.name.clone();
            visual.0 = node.selected;
            bg.0 = if node.selected {
                palette.accent_container
            } else {
                palette.surface_elevated
            };
            let border_col = if node.selected {
                palette.accent
            } else {
                palette.border
            };
            border.top = border_col;
            border.right = border_col;
            border.bottom = border_col;
            border.left = border_col;
        }
    }

    for mut btn in &mut test_group_buttons {
        if let Some(group) = projection.groups.get(btn.group_idx) {
            btn.group_name = group.name.clone();
        }
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_latency_tiers() {
        let (s1, t1) = format_latency(Some(35));
        assert_eq!(s1, "35 ms");
        assert_eq!(t1, LatencyTier::Fast);

        let (s2, t2) = format_latency(Some(220));
        assert_eq!(s2, "220 ms");
        assert_eq!(t2, LatencyTier::Medium);

        let (s3, t3) = format_latency(Some(850));
        assert_eq!(s3, "850 ms");
        assert_eq!(t3, LatencyTier::Slow);

        let (s4, t4) = format_latency(Some(0));
        assert_eq!(s4, "超时");
        assert_eq!(t4, LatencyTier::Timeout);

        let (s5, t5) = format_latency(None);
        assert_eq!(s5, "未测速");
        assert_eq!(t5, LatencyTier::Timeout);
    }

    #[test]
    fn demo_fixture_counts() {
        let proj = ProxiesProjection::demo();
        assert_eq!(proj.groups.len(), 3);
        assert_eq!(proj.total_nodes(), 8);
        assert_eq!(proj.active_exit, "🇭🇰 香港 01 · BGP 专线");
        assert_eq!(proj.groups[0].name, "节点选择 (PROXIES)");
        assert_eq!(proj.groups[0].group_type, "Selector");
        assert_eq!(proj.groups[0].proxies.len(), 4);
        assert_eq!(proj.groups[0].proxies[0].name, "🇭🇰 香港 01 · BGP 专线");
        assert_eq!(proj.groups[0].proxies[0].delay_ms, Some(38));
        assert!(proj.groups[0].expanded);
    }

    #[test]
    fn test_node_flag_extraction() {
        assert_eq!(node_flag("🇭🇰 香港 01 · BGP 专线"), "🇭🇰");
        assert_eq!(node_flag("HK-IEPL-01"), "🇭🇰");
        assert_eq!(node_flag("🇯🇵 日本东京 02 · 极速"), "🇯🇵");
        assert_eq!(node_flag("JP-Tokyo-01"), "🇯🇵");
        assert_eq!(node_flag("🇸🇬 新加坡 01 · Anycast"), "🇸🇬");
        assert_eq!(node_flag("SG-01"), "🇸🇬");
        assert_eq!(node_flag("🇺🇸 美国硅谷 01 · 4K"), "🇺🇸");
        assert_eq!(node_flag("US-Silicon-Valley"), "🇺🇸");
        assert_eq!(node_flag("Taiwan Premium"), "🇹🇼");
        assert_eq!(node_flag("Korea Seoul 01"), "🇰🇷");
        assert_eq!(node_flag("Unknown Node"), "🌐");
    }

    #[test]
    fn test_matches_proxy_filter() {
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
