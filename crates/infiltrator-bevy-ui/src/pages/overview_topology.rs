//! The Overview page's traffic-topology sub-feature: the five-stage chain
//! card, the shared flow graph, the click-through navigation and the
//! responsive connector visibility.

use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent,
    Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::traffic_topology_navigation_application::TrafficTopologyNavigationApplication;
use infiltrator_bevy_widgets::chart::topology::{
    NodeCategory, TopologyLink, TopologyNode, TopologySpec, topology_scene,
};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::traffic_topology::{
    TRAFFIC_TOPOLOGY_STAGE_COUNT, TrafficTopologySnapshot, TrafficTopologyStage,
    TrafficTopologyStatus,
};

use crate::pages::overview::{AccentContainerFill, SurfaceElevatedFill, SurfaceFill};
use crate::route::Route;

/// Explicit fixture adapter retained for deterministic demo/screenshot hosts.
pub fn topology_chain_scene(palette: &UiPalette) -> impl Scene + use<> {
    topology_chain_scene_with_snapshot(&TrafficTopologySnapshot::demo_fixture(), palette)
}

/// The production topology card: five shared stages and a widget-only flow
/// strip. All displayed facts are supplied by the application snapshot.
pub fn topology_chain_scene_with_snapshot(
    snapshot: &TrafficTopologySnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Region);
    header_a11y.set_label("分流网络拓扑");
    let header = topology_badge(snapshot);

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
                                ( Text({ header }) TopologyText { stage: TrafficTopologyStage::Inbound, kind: TopologyTextKind::HeaderBadge } TextRole(Role::Caption) TextColor({ palette.success }) ),
                            ]
                        ),
                    ]
                ),
                ( { topology_flow_scene(snapshot) } ),
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
                        ( { topology_stage_chip_scene(snapshot, TrafficTopologyStage::Inbound, IconId::Activity, palette.success, palette) } ),
                        ( { topology_arrow_scene(palette) } ),
                        ( { topology_stage_chip_scene(snapshot, TrafficTopologyStage::Sniffer, IconId::Activity, palette.accent, palette) } ),
                        ( { topology_arrow_scene(palette) } ),
                        ( { topology_stage_chip_scene(snapshot, TrafficTopologyStage::RuleSet, IconId::FileText, palette.accent, palette) } ),
                        ( { topology_arrow_scene(palette) } ),
                        ( { topology_stage_chip_scene(snapshot, TrafficTopologyStage::ProxyGroup, IconId::Settings, palette.warning, palette) } ),
                        ( { topology_arrow_scene(palette) } ),
                        ( { topology_stage_chip_scene(snapshot, TrafficTopologyStage::Outbound, IconId::Globe, palette.success, palette) } ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn topology_flow_scene(snapshot: &TrafficTopologySnapshot) -> impl Scene + use<> {
    topology_scene(topology_spec(snapshot))
}

/// Convert the shared contract into the generic widget graph. The widget
/// sees only stage/link geometry and aggregate flow metadata; it does not
/// know Mihomo, connections, or proxy groups.
pub(crate) fn topology_spec(snapshot: &TrafficTopologySnapshot) -> TopologySpec {
    let nodes = TrafficTopologyStage::ALL
        .into_iter()
        .enumerate()
        .map(|(index, stage)| {
            let node = snapshot.node(stage);
            let mut mapped = TopologyNode::new(
                stage.as_str(),
                node.map_or_else(|| stage_label(stage), |node| node.label.clone()),
                match stage {
                    TrafficTopologyStage::Inbound => NodeCategory::Inbound,
                    TrafficTopologyStage::Outbound => NodeCategory::Outbound,
                    TrafficTopologyStage::ProxyGroup => NodeCategory::Direct,
                    TrafficTopologyStage::Sniffer | TrafficTopologyStage::RuleSet => {
                        NodeCategory::Rule
                    }
                },
                index as f32 / (TRAFFIC_TOPOLOGY_STAGE_COUNT - 1) as f32,
                0.5,
            );
            mapped.subtext = node.map_or_else(String::new, |node| node.detail.clone());
            mapped
        })
        .collect();

    let links = if snapshot.is_drawable() {
        snapshot
            .links
            .iter()
            .map(|link| TopologyLink {
                source_id: link.from.as_str().to_owned(),
                target_id: link.to.as_str().to_owned(),
                bandwidth_bps: if link.flow_bps.is_finite() {
                    link.flow_bps.max(0.0)
                } else {
                    0.0
                },
                active_conns: link.active_connections,
                highlighted: link.active && link.flow_bps.is_finite() && link.flow_bps > 0.0,
            })
            .collect()
    } else {
        Vec::new()
    };
    TopologySpec::new(nodes, links, 860, 52).with_flow(0.0, snapshot.flow_speed_hz())
}

fn topology_stage_chip_scene(
    snapshot: &TrafficTopologySnapshot,
    stage: TrafficTopologyStage,
    icon: IconId,
    badge_fg: Color,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (label, detail, badge) = snapshot
        .node(stage)
        .map(|node| {
            let badge = if stage == TrafficTopologyStage::Sniffer {
                match snapshot.sniffer_enabled {
                    Some(true) => "On".to_owned(),
                    Some(false) => "Off".to_owned(),
                    None => "—".to_owned(),
                }
            } else if node.active_connections > 0 {
                format!("{} conns", node.active_connections)
            } else {
                "idle".to_owned()
            };
            (node.label.clone(), node.detail.clone(), badge)
        })
        .unwrap_or_else(|| {
            (
                stage_label(stage),
                snapshot
                    .failure
                    .clone()
                    .unwrap_or_else(|| "not available".to_owned()),
                "—".to_owned(),
            )
        });
    bsn! {
        Node {
            flex_grow: 1.0,
            flex_shrink: 1.0,
            flex_basis: px(140.0),
            min_width: px(110.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S6),
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        TopologyStageButton { stage, enabled: { snapshot.is_drawable() } }
        Button
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
                            ( Text({ label }) TopologyText { stage, kind: TopologyTextKind::Label } TextRole(Role::Caption) ),
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
                            ( Text({ badge }) TopologyText { stage, kind: TopologyTextKind::Badge } TextRole(Role::Caption) TextColor({ badge_fg }) ),
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
                    ( Text({ detail }) TopologyText { stage, kind: TopologyTextKind::Detail } TextRole(Role::BodyStrong) ),
                ]
            ),
        ]
    }
}

fn stage_label(stage: TrafficTopologyStage) -> String {
    match stage {
        TrafficTopologyStage::Inbound => "Client / Inbound",
        TrafficTopologyStage::Sniffer => "Sniffer",
        TrafficTopologyStage::RuleSet => "RuleSet",
        TrafficTopologyStage::ProxyGroup => "Proxy Group",
        TrafficTopologyStage::Outbound => "Outbound Node",
    }
    .to_owned()
}

fn topology_badge(snapshot: &TrafficTopologySnapshot) -> String {
    match snapshot.status {
        TrafficTopologyStatus::Ready => {
            format!("{} 连接 · flowing", snapshot.active_connections)
        }
        TrafficTopologyStatus::Empty => "0 连接 · idle".to_owned(),
        TrafficTopologyStatus::Unknown => "topology pending".to_owned(),
        TrafficTopologyStatus::Unsupported => "topology unavailable".to_owned(),
        TrafficTopologyStatus::Failed => "topology read failed".to_owned(),
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
        TopologyArrow
        Children [
            ( Text({ ">".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.ink_dim }) ),
        ]
    }
}

/// Marker on the traffic topology chain card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyChainCard;

/// Marker on the middle connecting arrow between stage pairs in the topology chain.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MiddleTopologyArrow;

/// Marker on every topology connector so compact mobile layouts can hide all
/// connectors without changing the shared five-stage data model.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyArrow;

/// Marker on mutable topology text projected from the shared snapshot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyText {
    pub stage: infiltrator_contract::traffic_topology::TrafficTopologyStage,
    pub kind: TopologyTextKind,
}

/// Click target on one topology stage. The button is intentionally gated by
/// the shared snapshot's drawable status in the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyStageButton {
    pub stage: infiltrator_contract::traffic_topology::TrafficTopologyStage,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TopologyTextKind {
    #[default]
    HeaderBadge,
    Label,
    Detail,
    Badge,
}

/// Restamp one topology text from the shared snapshot. The scene remains
/// mounted while the controller refreshes; only these marked values change.
pub(crate) fn topology_text_value(
    snapshot: &infiltrator_contract::traffic_topology::TrafficTopologySnapshot,
    marker: &TopologyText,
) -> String {
    if marker.kind == TopologyTextKind::HeaderBadge {
        return match snapshot.status {
            infiltrator_contract::traffic_topology::TrafficTopologyStatus::Ready => {
                format!("{} 连接 · flowing", snapshot.active_connections)
            }
            infiltrator_contract::traffic_topology::TrafficTopologyStatus::Empty => {
                "0 连接 · idle".to_owned()
            }
            infiltrator_contract::traffic_topology::TrafficTopologyStatus::Unknown => {
                "topology pending".to_owned()
            }
            infiltrator_contract::traffic_topology::TrafficTopologyStatus::Unsupported => {
                "topology unavailable".to_owned()
            }
            infiltrator_contract::traffic_topology::TrafficTopologyStatus::Failed => {
                "topology read failed".to_owned()
            }
        };
    }

    let Some(node) = snapshot.node(marker.stage) else {
        return match marker.kind {
            TopologyTextKind::Label => topology_stage_label(marker.stage).to_owned(),
            TopologyTextKind::Detail => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "not available".to_owned()),
            TopologyTextKind::Badge => "—".to_owned(),
            TopologyTextKind::HeaderBadge => unreachable!(),
        };
    };
    match marker.kind {
        TopologyTextKind::Label => node.label.clone(),
        TopologyTextKind::Detail => node.detail.clone(),
        TopologyTextKind::Badge => {
            if marker.stage == infiltrator_contract::traffic_topology::TrafficTopologyStage::Sniffer
            {
                match snapshot.sniffer_enabled {
                    Some(true) => "On".to_owned(),
                    Some(false) => "Off".to_owned(),
                    None => "—".to_owned(),
                }
            } else if node.active_connections > 0 {
                format!("{} conns", node.active_connections)
            } else {
                "idle".to_owned()
            }
        }
        TopologyTextKind::HeaderBadge => unreachable!(),
    }
}

fn topology_stage_label(
    stage: infiltrator_contract::traffic_topology::TrafficTopologyStage,
) -> &'static str {
    match stage {
        infiltrator_contract::traffic_topology::TrafficTopologyStage::Inbound => "Client / Inbound",
        infiltrator_contract::traffic_topology::TrafficTopologyStage::Sniffer => "Sniffer",
        infiltrator_contract::traffic_topology::TrafficTopologyStage::RuleSet => "RuleSet",
        infiltrator_contract::traffic_topology::TrafficTopologyStage::ProxyGroup => "Proxy Group",
        infiltrator_contract::traffic_topology::TrafficTopologyStage::Outbound => "Outbound Node",
    }
}

/// Translate a Bevy `Activate` gesture through the shared application
/// topology-navigation policy and into the shell's typed route event.
pub(crate) fn on_topology_stage_activated(
    activate: On<Activate>,
    buttons: Query<&TopologyStageButton>,
    mut commands: Commands,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    if !button.enabled {
        return;
    }
    let Some(page) = TrafficTopologyNavigationApplication::page_for_stage(button.stage) else {
        return;
    };
    let route = match page {
        infiltrator_contract::surface_snapshot::PageId::Settings => Route::Settings,
        infiltrator_contract::surface_snapshot::PageId::Rules => Route::Rules,
        infiltrator_contract::surface_snapshot::PageId::Proxies => Route::Proxies,
        _ => return,
    };
    commands.trigger(crate::route::RouteChanged(route));
}

/// Sync overview topology chain responsive layout according to layout mode.
pub fn sync_overview_responsive(
    layout: Option<Res<crate::app::ShellLayoutState>>,
    mut arrows: Query<&mut Node, With<TopologyArrow>>,
) {
    let Some(layout) = layout else {
        return;
    };
    let is_compact = layout.mode == crate::app::LayoutMode::BottomNav;
    let target_display = if is_compact {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut arrows {
        if node.display != target_display {
            node.display = target_display;
        }
    }
}
