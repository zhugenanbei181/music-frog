//! The Connections page (连接审计): active connections tracker, host/process
//! inspection, matched rule tracer, throughput rates, and disconnect actions.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ConnectionsLine`],
//! [`ConnSpeedText`], [`ConnHostText`], [`ConnProcessText`], [`ConnChainText`],
//! [`CloseConnectionButton`]). The page self-registers [`apply_connections_projection`]
//! and action observers once per world via [`ConnectionsPageRoot`].

use bevy::a11y::AccessibilityNode;
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
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::overview::{format_byte_count, format_rate};
use crate::route::{PageRoot, Route};

/// Root marker on the Connections page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_connections_page)]
pub struct ConnectionsPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct ConnectionsPageBound;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectionsLine(pub ConnectionsLineKind);

/// Different text lines on the connections page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectionsLineKind {
    /// Overview summary: active connections count.
    #[default]
    Summary,
    /// Total traffic uploaded & downloaded.
    TrafficSummary,
}

/// Marker for a connection row's rate display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnSpeedText(pub usize);

/// Marker for a connection row's host display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnHostText(pub usize);

/// Marker for a connection row's process/rule display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnProcessText(pub usize);

/// Marker for a connection row's chain display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnChainText(pub usize);

/// Marker for the "Close All Connections" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseAllConnectionsButton;

/// Marker and target information for a single connection close button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct CloseConnectionButton {
    pub connection_id: String,
    pub connection_idx: usize,
}

/// Connection aggregation / grouping mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnGroupingMode {
    /// Flat list of all active connections.
    #[default]
    Flat,
    /// Group connections by application process.
    ByProcess,
    /// Group connections by destination host.
    ByHost,
}

/// Marker component for the connection aggregation segmented control pills.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnAggregationPill(pub ConnGroupingMode);

/// A single active connection entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionItem {
    pub id: String,
    pub host: String,
    pub process: String,
    pub rule: String,
    pub chain: String,
    pub upload_bps: f64,
    pub download_bps: f64,
    pub upload_total: u64,
    pub download_total: u64,
}

/// Snapshot of the Connections domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionsProjection {
    pub total_connections: usize,
    pub total_upload_bytes: u64,
    pub total_download_bytes: u64,
    pub connections: Vec<ConnectionItem>,
}

impl ConnectionsProjection {
    /// Believable demo fixture for the Connections page.
    pub fn demo() -> Self {
        Self {
            total_connections: 4,
            total_upload_bytes: 14_200_000,
            total_download_bytes: 88_900_000,
            connections: vec![
                ConnectionItem {
                    id: "c-1".to_owned(),
                    host: "api.github.com:443".to_owned(),
                    process: "git (pid: 14238)".to_owned(),
                    rule: "DOMAIN-SUFFIX github.com".to_owned(),
                    chain: "节点选择 -> 🇭🇰 香港 01".to_owned(),
                    upload_bps: 24_000.0,
                    download_bps: 180_000.0,
                    upload_total: 1_200_000,
                    download_total: 12_400_000,
                },
                ConnectionItem {
                    id: "c-2".to_owned(),
                    host: "manifest.googlevideo.com:443".to_owned(),
                    process: "chrome (pid: 8912)".to_owned(),
                    rule: "GEOSITE youtube".to_owned(),
                    chain: "国外媒体 -> 🇸🇬 新加坡 01".to_owned(),
                    upload_bps: 8_500.0,
                    download_bps: 2_450_000.0,
                    upload_total: 450_000,
                    download_total: 68_000_000,
                },
                ConnectionItem {
                    id: "c-3".to_owned(),
                    host: "gateway.discord.gg:443".to_owned(),
                    process: "Discord (pid: 11024)".to_owned(),
                    rule: "DOMAIN-SUFFIX discord.gg".to_owned(),
                    chain: "节点选择 -> 🇭🇰 香港 01".to_owned(),
                    upload_bps: 1_200.0,
                    download_bps: 3_400.0,
                    upload_total: 890_000,
                    download_total: 4_200_000,
                },
                ConnectionItem {
                    id: "c-4".to_owned(),
                    host: "119.29.29.29:53".to_owned(),
                    process: "systemd-resolved".to_owned(),
                    rule: "GEOIP CN".to_owned(),
                    chain: "DIRECT".to_owned(),
                    upload_bps: 0.0,
                    download_bps: 0.0,
                    upload_total: 12_000,
                    download_total: 34_000,
                },
            ],
        }
    }
}

/// The typed event dispatched when connection data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct ConnectionsProjectionUpdated(pub ConnectionsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastConnectionsProjection(pub Option<ConnectionsProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn connections_page(
    projection: &ConnectionsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let summary = format!(
        "活动连接 · 当前活跃 {} 个连接",
        projection.total_connections
    );
    let traffic = format!(
        "累积上传: {} | 累积下载: {}",
        format_byte_count(projection.total_upload_bytes),
        format_byte_count(projection.total_download_bytes)
    );

    let connection_scenes: Vec<Box<dyn Scene>> = projection
        .connections
        .iter()
        .enumerate()
        .map(|(idx, item)| Box::new(connection_row_scene(idx, item, palette)) as Box<dyn Scene>)
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
        PageRoot(Route::Connections)
        ConnectionsPageRoot
        Children [
            ( { header_card_scene(summary, traffic, palette) } ),
            ( { connections_table_scene(connection_scenes, palette) } ),
            ( { crate::pages::connections_drawer::connection_drawer_scene(palette) } ),
        ]
    }
}

fn header_card_scene(summary: String, traffic: String, palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("连接审计概览");

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
                        ( { icon_tile_scene(IconId::Network, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) ConnectionsLine(ConnectionsLineKind::Summary) TextRole(Role::Heading) ),
                                ( Text(traffic) ConnectionsLine(ConnectionsLineKind::TrafficSummary) TextRole(Role::Caption) ),
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
                        (
                            Node {
                                align_items: AlignItems::Center,
                                padding: UiRect::all(Val::Px(2.0)),
                                border: UiRect::all(Val::Px(palette.hairline_px)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                column_gap: Val::Px(space::S4),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            BorderColor {
                                top: { palette.border },
                                right: { palette.border },
                                bottom: { palette.border },
                                left: { palette.border },
                            }
                            Children [
                                ( { conn_aggregation_pill(ConnGroupingMode::Flat, "全部连接 (Flat)", true, palette) } ),
                                ( { conn_aggregation_pill(ConnGroupingMode::ByProcess, "按应用进程聚合 (By Process)", false, palette) } ),
                                ( { conn_aggregation_pill(ConnGroupingMode::ByHost, "按目标域名聚合 (By Host)", false, palette) } ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.danger })
                            Button
                            CloseAllConnectionsButton
                            Children [
                                ( Text({ "关闭全部连接".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn conn_aggregation_pill(
    mode: ConnGroupingMode,
    label: &str,
    active: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (bg, text_color) = if active {
        (palette.accent_container, palette.accent)
    } else {
        (palette.surface, palette.ink_dim)
    };
    let label_str = label.to_owned();

    bsn! {
        Node {
            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            align_items: AlignItems::Center,
        }
        BackgroundColor({ bg })
        ConnAggregationPill(mode)
        Button
        Children [
            ( Text(label_str) TextRole(Role::Caption) TextColor({ text_color }) ),
        ]
    }
}

fn connections_table_scene(
    connection_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
) -> impl Scene + use<> {
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
                    ( Text({ "实时连接表 (Active Sessions)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ "实时追踪链路与进程".to_owned() }) TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                Children [
                    { connection_scenes },
                ]
            }),
        ],
        palette,
    )
}

fn connection_row_scene(
    idx: usize,
    conn: &ConnectionItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let host = conn.host.clone();
    let process_info = format!("{} · {}", conn.process, conn.rule);
    let chain_info = format!("链路: {}", conn.chain);
    let speed_info = format!(
        "↑ {}  ↓ {}",
        format_rate(conn.upload_bps),
        format_rate(conn.download_bps)
    );
    let conn_btn = CloseConnectionButton {
        connection_id: conn.id.clone(),
        connection_idx: idx,
    };

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            Children [
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( Text(host) ConnHostText(idx) TextRole(Role::BodyStrong) ),
                                ( Text(process_info) ConnProcessText(idx) TextRole(Role::Caption) ),
                            ]
                        ),
                        ( Text(chain_info) ConnChainText(idx) TextRole(Role::Caption) ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( Text(speed_info) ConnSpeedText(idx) TextRole(Role::Mono) ),
                        (
                            Node {
                                min_height: px(palette.control_height_px * 0.8),
                                padding: UiRect::horizontal(Val::Px(space::S8)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            template_value(conn_btn)
                            Children [
                                ( Text({ "断开".to_owned() }) TextRole(Role::Caption) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_connections_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<ConnectionsPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(ConnectionsPageBound);
    commands.add_observer(apply_connections_projection);
    commands.add_observer(on_connections_action_activated);
}

pub(crate) fn on_connections_action_activated(
    activate: On<Activate>,
    close_all_buttons: Query<(), With<CloseAllConnectionsButton>>,
    close_row_buttons: Query<&CloseConnectionButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if close_all_buttons.contains(activate.entity) {
        handle.submit(UiCommand::CloseAllConnections);
    } else if let Ok(btn) = close_row_buttons.get(activate.entity) {
        handle.submit(UiCommand::CloseConnection {
            id: btn.connection_id.clone(),
        });
    }
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_connections_projection(
    update: On<ConnectionsProjectionUpdated>,
    mut last: Option<ResMut<LastConnectionsProjection>>,
    mut lines: Query<
        (&mut Text, &ConnectionsLine),
        (
            With<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnHostText>,
            Without<ConnProcessText>,
            Without<ConnChainText>,
        ),
    >,
    mut speeds: Query<
        (&mut Text, &ConnSpeedText),
        (
            With<ConnSpeedText>,
            Without<ConnectionsLine>,
            Without<ConnHostText>,
            Without<ConnProcessText>,
            Without<ConnChainText>,
        ),
    >,
    mut hosts: Query<
        (&mut Text, &ConnHostText),
        (
            With<ConnHostText>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnProcessText>,
            Without<ConnChainText>,
        ),
    >,
    mut processes: Query<
        (&mut Text, &ConnProcessText),
        (
            With<ConnProcessText>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnHostText>,
            Without<ConnChainText>,
        ),
    >,
    mut chains: Query<
        (&mut Text, &ConnChainText),
        (
            With<ConnChainText>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnHostText>,
            Without<ConnProcessText>,
        ),
    >,
    mut buttons: Query<&mut CloseConnectionButton>,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            ConnectionsLineKind::Summary => {
                text.0 = format!(
                    "活动连接 · 当前活跃 {} 个连接",
                    projection.total_connections
                );
            }
            ConnectionsLineKind::TrafficSummary => {
                text.0 = format!(
                    "累积上传: {} | 累积下载: {}",
                    format_byte_count(projection.total_upload_bytes),
                    format_byte_count(projection.total_download_bytes)
                );
            }
        }
    }

    for (mut text, marker) in &mut speeds {
        if let Some(conn) = projection.connections.get(marker.0) {
            text.0 = format!(
                "↑ {}  ↓ {}",
                format_rate(conn.upload_bps),
                format_rate(conn.download_bps)
            );
        }
    }

    for (mut text, marker) in &mut hosts {
        if let Some(conn) = projection.connections.get(marker.0) {
            text.0 = conn.host.clone();
        }
    }

    for (mut text, marker) in &mut processes {
        if let Some(conn) = projection.connections.get(marker.0) {
            text.0 = format!("{} · {}", conn.process, conn.rule);
        }
    }

    for (mut text, marker) in &mut chains {
        if let Some(conn) = projection.connections.get(marker.0) {
            text.0 = format!("链路: {}", conn.chain);
        }
    }

    for mut btn in &mut buttons {
        if let Some(conn) = projection.connections.get(btn.connection_idx) {
            btn.connection_id = conn.id.clone();
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
    fn demo_connections_fixture() {
        let proj = ConnectionsProjection::demo();
        assert_eq!(proj.total_connections, 4);
        assert_eq!(proj.connections.len(), 4);
        assert_eq!(proj.connections[0].id, "c-1");
        assert_eq!(proj.connections[0].host, "api.github.com:443");
        assert_eq!(proj.connections[0].process, "git (pid: 14238)");
        assert_eq!(proj.total_upload_bytes, 14_200_000);
        assert_eq!(proj.total_download_bytes, 88_900_000);
    }
}
