//! The Proxies page (代理策略): proxy groups and nodes, latency testing,
//! group selection, and active outbound routing.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProxiesLine`],
//! [`ProxyNodeMarker`], [`LatencyText`]). The page self-registers
//! [`apply_proxies_projection`] once per world via the [`ProxiesPageRoot`]
//! `on_insert` bind hook. When [`ProxiesProjectionUpdated`] fires, texts,
//! latency inks, and selection states restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
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
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

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

/// Marker for a proxy node's selection state text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeNameText {
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

/// Pure helper to classify and format latency in milliseconds.
pub fn format_latency(delay_ms: Option<u32>) -> (String, LatencyTier) {
    match delay_ms {
        Some(0) => ("超时".to_owned(), LatencyTier::Timeout),
        Some(ms) if ms < 150 => (format!("{ms} ms"), LatencyTier::Fast),
        Some(ms) if ms < 500 => (format!("{ms} ms"), LatencyTier::Medium),
        Some(ms) => (format!("{ms} ms"), LatencyTier::Slow),
        None => ("未测速".to_owned(), LatencyTier::Timeout),
    }
}

/// Resolve latency text color from tier and palette tokens.
pub fn latency_color(tier: LatencyTier, palette: &UiPalette) -> Color {
    match tier {
        LatencyTier::Fast => palette.success,
        LatencyTier::Medium => palette.warning,
        LatencyTier::Slow | LatencyTier::Timeout => palette.ink_dim,
    }
}

/// A single proxy node snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyNode {
    pub name: String,
    pub node_type: String,
    pub delay_ms: Option<u32>,
    pub selected: bool,
}

/// A proxy group snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProxyGroup {
    pub name: String,
    pub group_type: String,
    pub current: String,
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
                    proxies: vec![
                        ProxyNode {
                            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            selected: true,
                        },
                        ProxyNode {
                            name: "🇯🇵 日本东京 02 · 极速".to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            selected: false,
                        },
                        ProxyNode {
                            name: "🇸🇬 新加坡 01 · Anycast".to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            selected: false,
                        },
                        ProxyNode {
                            name: "🇺🇸 美国硅谷 01 · 4K".to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            selected: false,
                        },
                    ],
                },
                ProxyGroup {
                    name: "自动选择 (AUTO)".to_owned(),
                    group_type: "URLTest".to_owned(),
                    current: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                    proxies: vec![
                        ProxyNode {
                            name: "🇭🇰 香港 01 · BGP 专线".to_owned(),
                            node_type: "Shadowsocks".to_owned(),
                            delay_ms: Some(38),
                            selected: true,
                        },
                        ProxyNode {
                            name: "🇯🇵 日本东京 02 · 极速".to_owned(),
                            node_type: "Vmess".to_owned(),
                            delay_ms: Some(65),
                            selected: false,
                        },
                    ],
                },
                ProxyGroup {
                    name: "国外媒体 (STREAMING)".to_owned(),
                    group_type: "Selector".to_owned(),
                    current: "🇸🇬 新加坡 01 · Anycast".to_owned(),
                    proxies: vec![
                        ProxyNode {
                            name: "🇸🇬 新加坡 01 · Anycast".to_owned(),
                            node_type: "Trojan".to_owned(),
                            delay_ms: Some(72),
                            selected: true,
                        },
                        ProxyNode {
                            name: "🇺🇸 美国硅谷 01 · 4K".to_owned(),
                            node_type: "Hysteria2".to_owned(),
                            delay_ms: Some(152),
                            selected: false,
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
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Proxies)
        ProxiesPageRoot
        Children [
            ( { header_card_scene(summary, active_exit, test_status, palette) } ),
            { group_scenes },
        ]
    }
}

fn header_card_scene(
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

fn group_card_scene(g_idx: usize, group: &ProxyGroup, palette: &UiPalette) -> impl Scene + use<> {
    let title = group.name.clone();
    let type_badge = format!("[{}]", group.group_type);
    let current_label = format!("选中: {}", group.current);

    let node_scenes: Vec<Box<dyn Scene>> = group
        .proxies
        .iter()
        .enumerate()
        .map(|(n_idx, node)| {
            Box::new(proxy_node_scene(g_idx, n_idx, node, palette)) as Box<dyn Scene>
        })
        .collect();

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
                            ( Text(type_badge) TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text(current_label) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { node_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn proxy_node_scene(
    g_idx: usize,
    n_idx: usize,
    node: &ProxyNode,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = node.name.clone();
    let (delay_str, tier) = format_latency(node.delay_ms);
    let delay_color_val = latency_color(tier, palette);
    let proto_tag = node.node_type.clone();

    let bg = if node.selected {
        palette.accent_container
    } else {
        palette.surface_elevated
    };

    bsn! {
        Node {
            width: percent(100),
            min_height: px(palette.control_height_px),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::horizontal(Val::Px(space::S12)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ bg })
        ControlVisual({ node.selected })
        Button
        Children [
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(name) NodeNameText { group_idx: g_idx, node_idx: n_idx } TextRole(Role::Body) ),
                    ( Text(proto_tag) TextRole(Role::Caption) ),
                ]
            ),
            (
                Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
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

// ---- Observer & Update Hook -----------------------------------------------

fn bind_proxies_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<ProxiesPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(ProxiesPageBound);
    commands.add_observer(apply_proxies_projection);
}

#[allow(clippy::type_complexity)]
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
        ),
    >,
    mut latencies: Query<
        (&mut Text, &mut TextColor, &LatencyText),
        (
            With<LatencyText>,
            Without<ProxiesLine>,
            Without<NodeNameText>,
        ),
    >,
    mut node_names: Query<
        (&mut Text, &NodeNameText),
        (
            With<NodeNameText>,
            Without<ProxiesLine>,
            Without<LatencyText>,
        ),
    >,
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
        assert!(!proj.active_exit.is_empty());
    }
}
