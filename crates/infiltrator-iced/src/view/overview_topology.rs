//! Network topology flow card and its five stage nodes, badges and
//! navigation routing.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::component_forms::row_card_surface;
use crate::view::components::{BadgeKind, badge, card_surface};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_SEMIBOLD, R_CHIP, R_CONTROL, tokens};
use crate::view::topology::topology_flow_canvas;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_application::traffic_topology_navigation_application::TrafficTopologyNavigationApplication;
use infiltrator_contract::traffic_topology::{
    TrafficTopologySnapshot, TrafficTopologyStage, TrafficTopologyStatus,
};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn topology_card<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
    _is_en: bool,
) -> Element<'a, Message> {
    let topology = &state.runtime.traffic_topology;

    let card_header = row![
        row![
            icon_themed(Icon::Network, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_topology_title").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        badge(
            topology_badge(topology, lang),
            topology_badge_kind(topology),
        ),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let node_inbound = topology_stage_node(
        topology,
        TrafficTopologyStage::Inbound,
        Icon::Server,
        "Client / Inbound",
        BadgeKind::Neutral,
        |t| tokens(t).text_secondary,
    );
    let node_sniffer = topology_stage_node(
        topology,
        TrafficTopologyStage::Sniffer,
        Icon::Search,
        "Sniffer",
        BadgeKind::Accent,
        |t| tokens(t).accent,
    );
    let node_ruleset = topology_stage_node(
        topology,
        TrafficTopologyStage::RuleSet,
        Icon::Shield,
        "RuleSet",
        BadgeKind::Accent,
        |t| tokens(t).accent,
    );
    let node_group = topology_stage_node(
        topology,
        TrafficTopologyStage::ProxyGroup,
        Icon::LayoutGrid,
        "Proxy Group",
        BadgeKind::Warning,
        |t| tokens(t).warning,
    );
    let node_outbound = topology_stage_node(
        topology,
        TrafficTopologyStage::Outbound,
        Icon::Globe,
        "Outbound Node",
        BadgeKind::Success,
        |t| tokens(t).success,
    );

    let flow_row = row![
        node_inbound,
        arrow_connector(),
        node_sniffer,
        arrow_connector(),
        node_ruleset,
        arrow_connector(),
        node_group,
        arrow_connector(),
        node_outbound,
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    container(
        column![
            card_header,
            Space::new().height(theme::SP_SM),
            topology_flow_canvas(topology, state.diag.topology_flow_phase),
            flow_row,
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::Fill)
    .padding(theme::SP_XXL)
    .style(card_surface)
    .into()
}

fn topology_stage_node<'a>(
    snapshot: &TrafficTopologySnapshot,
    stage: TrafficTopologyStage,
    glyph: Icon,
    fallback_label: &'static str,
    badge_kind: BadgeKind,
    color_fn: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let (label, detail, badge_label) = snapshot
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
                fallback_label.to_owned(),
                snapshot
                    .failure
                    .clone()
                    .unwrap_or_else(|| "not available".to_owned()),
                "—".to_owned(),
            )
        });
    let node = topology_node_box(glyph, label, detail, badge_label, badge_kind, color_fn);
    let Some(route) = topology_route_for_stage(stage) else {
        return node;
    };
    if !snapshot.is_drawable() {
        return node;
    }
    button(node)
        .width(Length::Fill)
        .padding(0)
        .on_press(Message::Navigate(route))
        .into()
}

pub fn topology_route_for_stage(stage: TrafficTopologyStage) -> Option<Route> {
    match TrafficTopologyNavigationApplication::page_for_stage(stage)? {
        infiltrator_contract::surface_snapshot::PageId::Settings => Some(Route::Settings),
        infiltrator_contract::surface_snapshot::PageId::Rules => Some(Route::Rules),
        infiltrator_contract::surface_snapshot::PageId::Proxies => Some(Route::Proxies),
        _ => None,
    }
}

pub fn topology_badge(snapshot: &TrafficTopologySnapshot, lang: &Lang<'_>) -> String {
    match snapshot.status {
        TrafficTopologyStatus::Ready => format!(
            "{} {} · flowing",
            snapshot.active_connections,
            lang.tr("overview_conn_unit")
        ),
        TrafficTopologyStatus::Empty => format!("0 {} · idle", lang.tr("overview_conn_unit")),
        TrafficTopologyStatus::Unknown => "topology pending".to_owned(),
        TrafficTopologyStatus::Unsupported => "topology unavailable".to_owned(),
        TrafficTopologyStatus::Failed => "topology read failed".to_owned(),
    }
}

pub fn topology_badge_kind(snapshot: &TrafficTopologySnapshot) -> BadgeKind {
    match snapshot.status {
        TrafficTopologyStatus::Ready => BadgeKind::Success,
        TrafficTopologyStatus::Empty | TrafficTopologyStatus::Unknown => BadgeKind::Neutral,
        TrafficTopologyStatus::Unsupported => BadgeKind::Warning,
        TrafficTopologyStatus::Failed => BadgeKind::Danger,
    }
}

fn topology_node_box<'a>(
    glyph: Icon,
    stage_name: String,
    chip_label: String,
    badge_label: String,
    badge_kind: BadgeKind,
    color_fn: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let icon_part = container(icon_themed(glyph, 14.0, color_fn))
        .width(26)
        .height(26)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |t: &Theme| {
            let c = color_fn(t);
            container::Style {
                background: Some(Color { a: 0.14, ..c }.into()),
                border: Border {
                    radius: border::Radius::from(R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    let top_row = row![
        icon_part,
        Space::new().width(theme::SP_XS),
        text(stage_name)
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(Length::Fill),
        badge(badge_label, badge_kind),
    ]
    .align_y(Alignment::Center);

    let bottom_row = row![colored_flow_chip(chip_label, color_fn)].align_y(Alignment::Center);

    container(
        column![top_row, Space::new().height(theme::SP_XS), bottom_row]
            .spacing(theme::SP_XS)
            .width(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_MD)
    .style(row_card_surface)
    .into()
}

fn colored_flow_chip<'a>(
    label: String,
    color_fn: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    container(
        text(label)
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(move |t: &Theme| text::Style {
                color: Some(color_fn(t)),
            }),
    )
    .padding([3, 10])
    .style(move |t: &Theme| {
        let c = color_fn(t);
        container::Style {
            background: Some(Color { a: 0.12, ..c }.into()),
            border: Border {
                radius: border::Radius::from(R_CHIP),
                width: theme::HAIRLINE,
                color: Color { a: 0.25, ..c },
            },
            ..Default::default()
        }
    })
    .into()
}

fn arrow_connector<'a>() -> Element<'a, Message> {
    container(icon_themed(Icon::ChevronRight, 16.0, |t: &Theme| {
        tokens(t).text_tertiary
    }))
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}
