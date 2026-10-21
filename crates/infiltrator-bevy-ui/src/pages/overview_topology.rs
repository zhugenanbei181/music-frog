//! The Overview page's traffic-topology sub-feature: the five-stage chain
//! card, the shared flow graph, the click-through navigation and the
//! responsive connector visibility.

use crate::app::{LayoutMode, ShellLayoutState};
use crate::pages::overview::{
    AccentContainerFill, LastOverviewProjection, SurfaceElevatedFill, SurfaceFill,
};
use crate::pages::overview_topology_drilldown::drilldown_panel_scene;
use bevy::a11y::AccessibilityNode;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent, Node, UiRect,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::chart::topology::{
    NodeCategory, TopologyLink, TopologyNode, TopologyPlate, TopologySpec, topology_scene,
};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_contract::traffic_topology::{
    TRAFFIC_TOPOLOGY_STAGE_COUNT, TrafficTopologySnapshot, TrafficTopologyStage,
};
use infiltrator_presentation::traffic_topology_projection::status_label;
use infiltrator_shared::locales::Lang;

/// The topology stage card's flex basis in baseline pixels.
const TOPOLOGY_STAGE_FLEX_BASIS_PX: f32 = 140.0;

/// The topology stage card's minimum width in baseline pixels.
const TOPOLOGY_STAGE_MIN_WIDTH_PX: f32 = 110.0;

/// Explicit fixture adapter retained for deterministic demo/screenshot hosts.
#[cfg(any(feature = "capture-tools", test))]
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
    header_a11y.set_label(UiLocale::default().text("overview_topology_label"));
    let header = status_label(snapshot, &Lang(UiLocale::default().code()));

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S12),
                    }
                    AccessibilityNode(header_a11y) LocalizedLabel::plain("overview_topology_label")
                    TopologyChainCard
                    Children [
                        Node {
                            width: percent(100),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                        }
                        Children [
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: rem(space::S8),
                            }
                            Children [
                                @{ icon_scene(IconId::Network, 16.0, palette.accent) }
                                --
                                LocalizedText::plain("overview_topology_title") TextRole(Role::Heading)
                            ]
                            --
                            Node {
                                padding: UiRect::axes(rem(space::S8), rem(space::S2)),
                                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
                            }
                            ThemeBackgroundColor({ Token::AccentContainer })
                            AccentContainerFill
                            Children [
                                Text({ header }) TopologyText { stage: TrafficTopologyStage::Inbound, kind: TopologyTextKind::HeaderBadge } TextRole(Role::Caption) ThemeTextColor({ Token::Success })
                            ]
                        ]
                        --
                        @{ topology_flow_scene(snapshot) }
                        --
                        Node {
                            width: percent(100),
                            min_width: px(0.0),
                            max_width: percent(100),
                            flex_direction: FlexDirection::Row,
                            flex_wrap: FlexWrap::Wrap,
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::SpaceBetween,
                            row_gap: rem(space::S8),
                            column_gap: rem(space::S6),
                        }
                        Children [
                            @{ topology_stage_chip_scene(snapshot, TrafficTopologyStage::Inbound, IconId::Activity, palette.success, palette) }
                            --
                            @{ topology_arrow_scene(palette) }
                            --
                            @{ topology_stage_chip_scene(snapshot, TrafficTopologyStage::Sniffer, IconId::Activity, palette.accent, palette) }
                            --
                            @{ topology_arrow_scene(palette) }
                            --
                            @{ topology_stage_chip_scene(snapshot, TrafficTopologyStage::RuleSet, IconId::FileText, palette.accent, palette) }
                            --
                            @{ topology_arrow_scene(palette) }
                            --
                            @{ topology_stage_chip_scene(snapshot, TrafficTopologyStage::ProxyGroup, IconId::Settings, palette.warning, palette) }
                            --
                            @{ topology_arrow_scene(palette) }
                            --
                            @{ topology_stage_chip_scene(snapshot, TrafficTopologyStage::Outbound, IconId::Globe, palette.success, palette) }
                        ]
                        --
                        @{ drilldown_panel_scene(palette) }
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
    TopologySpec::new(nodes, links, 860, 52).with_flow(0.0, snapshot.linear_flow_speed_hz())
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
                flex_basis: rem(TOPOLOGY_STAGE_FLEX_BASIS_PX),
                min_width: rem(TOPOLOGY_STAGE_MIN_WIDTH_PX),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S6),
                padding: UiRect::all(rem(space::S8)),
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            TopologyStageButton { stage, enabled: { snapshot.is_drawable() } }
            Button
            ThemeBackgroundColor({ Token::SurfaceElevated })
            SurfaceElevatedFill
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: rem(space::S6),
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S6),
                    }
                    Children [
                        @{ icon_scene(icon, 14.0, palette.ink_dim) }
                        --
                        Text({ label }) TopologyText { stage, kind: TopologyTextKind::Label } TextRole(Role::Caption)
                    ]
                    --
                    Node {
                        padding: UiRect::axes(rem(space::S6), rem(space::S2)),
                        border_radius: BorderRadius::all(rem(radius::XS)),
                    }
                    ThemeBackgroundColor({ Token::AccentContainer })
                    AccentContainerFill
                    Children [
                        Text({ badge }) TopologyText { stage, kind: TopologyTextKind::Badge } TextRole(Role::Caption) TextColor({ badge_fg })
                    ]
                ]
                --
                Node {
                    width: percent(100),
                    padding: UiRect::axes(rem(space::S8), rem(space::S4)),
                    border_radius: BorderRadius::all(rem(radius::XS)),
                }
                ThemeBackgroundColor({ Token::Surface })
                SurfaceFill
                Children [
                    Text({ detail }) TopologyText { stage, kind: TopologyTextKind::Detail } TextRole(Role::BodyStrong)
                ]
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

fn topology_arrow_scene(_palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                padding: UiRect::horizontal(rem(space::S2)),
            }
            TopologyArrow
            Children [
                Text({ ">".to_owned() }) TextRole(Role::BodyStrong) ThemeTextColor({ Token::InkDim })
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
    pub stage: TrafficTopologyStage,
    pub kind: TopologyTextKind,
}

/// Click target on one topology stage. The button is intentionally gated by
/// the shared snapshot's drawable status in the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologyStageButton {
    pub stage: TrafficTopologyStage,
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
    snapshot: &TrafficTopologySnapshot,
    marker: &TopologyText,
    locale: &str,
) -> String {
    if marker.kind == TopologyTextKind::HeaderBadge {
        return status_label(snapshot, &Lang(locale));
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
            if marker.stage == TrafficTopologyStage::Sniffer {
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

fn topology_stage_label(stage: TrafficTopologyStage) -> &'static str {
    match stage {
        TrafficTopologyStage::Inbound => "Client / Inbound",
        TrafficTopologyStage::Sniffer => "Sniffer",
        TrafficTopologyStage::RuleSet => "RuleSet",
        TrafficTopologyStage::ProxyGroup => "Proxy Group",
        TrafficTopologyStage::Outbound => "Outbound Node",
    }
}

/// Global navigation filter intent injected during topology node drilldown (UI-04-05).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct TopologyDrilldownFilter {
    pub stage: Option<TrafficTopologyStage>,
    pub filter_query: Option<String>,
}

/// Sync topology plate full-chain highlight when any stage card is hovered (UI-04-05).
pub fn sync_topology_hover_highlight(
    stage_buttons: Query<(&TopologyStageButton, Option<&PickingInteraction>)>,
    mut plates: Query<&mut TopologyPlate>,
) {
    let hovered_stage = stage_buttons.iter().find_map(|(btn, interaction)| {
        if matches!(
            interaction,
            Some(PickingInteraction::Hovered | PickingInteraction::Pressed)
        ) {
            Some(btn.stage.as_str().to_string())
        } else {
            None
        }
    });

    for mut plate in &mut plates {
        if plate.0.hovered_stage != hovered_stage {
            plate.0.hovered_stage = hovered_stage.clone();
        }
    }
}

/// Translate a Bevy `Activate` gesture on a topology stage into the shared
/// application drilldown.
///
/// The activation opens the Overview page's own drilldown panel: the stage's
/// result is rendered on the page that owns the topology, and the shared
/// navigation target is offered *inside* that panel. The business therefore
/// never leaves its own page to show its outcome.
pub(crate) fn on_topology_stage_activated(
    activate: On<Activate>,
    buttons: Query<&TopologyStageButton>,
    last_overview: Option<Res<LastOverviewProjection>>,
    mut drilldown: Option<ResMut<TopologyDrilldownFilter>>,
    mut commands: Commands,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    if !button.enabled {
        return;
    }

    // Extract filter keyword from the topology stage node detail
    let filter_query = last_overview
        .as_ref()
        .and_then(|lo| lo.0.as_ref())
        .and_then(|proj| {
            proj.traffic_topology.node(button.stage).and_then(|node| {
                if node.detail.is_empty() {
                    None
                } else {
                    Some(node.detail.clone())
                }
            })
        });

    if let Some(ref mut drilldown) = drilldown {
        drilldown.stage = Some(button.stage);
        drilldown.filter_query = filter_query;
    } else {
        commands.insert_resource(TopologyDrilldownFilter {
            stage: Some(button.stage),
            filter_query,
        });
    }
}

/// Sync overview topology chain responsive layout according to layout mode.
pub fn sync_overview_responsive(
    layout: Option<Res<ShellLayoutState>>,
    mut arrows: Query<&mut Node, With<TopologyArrow>>,
) {
    let Some(layout) = layout else {
        return;
    };
    let is_compact = layout.mode == LayoutMode::BottomNav;
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
