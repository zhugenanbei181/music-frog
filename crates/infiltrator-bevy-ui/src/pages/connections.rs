//! The Connections page (连接审计): active connections tracker, host/process
//! inspection, matched rule tracer, throughput rates, and disconnect actions.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ConnectionsLine`],
//! [`ConnSpeedText`], [`ConnHostText`], [`ConnProcessText`],
//! [`ConnChainHopText`],
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
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, JustifyContent, Node,
    Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_domain::connection_view;
use infiltrator_domain::connection_view::ConnectionGroupingMode;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections_idle::{ConnectionsIdleState, current_unix_secs};
use crate::pages::connections_view::{
    CloseAllConnectionsLabel, CloseFilteredConnectionsButton, ConnAggregationSummary,
    ConnAggregationSummaryContainer, ConnRowsContainer, ConnSearchField, ConnectionRow,
    ConnectionsCloseAllState, ConnectionsViewState, restamp_aggregation_pills,
    restamp_aggregation_summary, search_field_text,
};
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
    /// DUAL-13-01: connections telemetry stream-phase badge.
    Stream,
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

/// Marker for a connection row's route-chain hop display (DUAL-13-06).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnChainHopText {
    /// Owning flat row index.
    pub row: usize,
    /// Hop ordinal within the parsed route chain.
    pub hop: usize,
}

/// Marker for the inspect button of one flat row (DUAL-13-03).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnInspectButton(pub usize);

/// Marker for the "Close All Connections" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseAllConnectionsButton;

/// Marker and target information for a single connection close button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct CloseConnectionButton {
    pub connection_id: String,
    pub connection_idx: usize,
}

/// Marker component for the connection aggregation segmented control pills.
/// The payload is the shared domain grouping mode (DUAL-13-02), so Bevy and
/// Iced switch the same modes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnAggregationPill(pub ConnectionGroupingMode);

/// A single active connection entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionItem {
    pub id: String,
    pub host: String,
    pub process: String,
    pub rule: String,
    pub chain: String,
    /// DUAL-13-06: parsed route-chain hops, one per stage.
    pub chains: Vec<String>,
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
    /// DUAL-13-01: lifecycle phase of the connections telemetry feed.
    pub stream_phase: infiltrator_contract::connection::ConnectionStreamPhase,
    pub connections: Vec<ConnectionItem>,
}

impl ConnectionsProjection {
    /// Believable demo fixture for the Connections page.
    pub fn demo() -> Self {
        Self {
            total_connections: 4,
            total_upload_bytes: 14_200_000,
            total_download_bytes: 88_900_000,
            stream_phase: infiltrator_contract::connection::ConnectionStreamPhase::Live,
            connections: vec![
                ConnectionItem {
                    id: "c-1".to_owned(),
                    host: "api.github.com:443".to_owned(),
                    process: "git (pid: 14238)".to_owned(),
                    rule: "DOMAIN-SUFFIX github.com".to_owned(),
                    chain: "节点选择 -> 🇭🇰 香港 01".to_owned(),
                    chains: vec!["节点选择".to_owned(), "🇭🇰 香港 01".to_owned()],
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
                    chains: vec!["国外媒体".to_owned(), "🇸🇬 新加坡 01".to_owned()],
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
                    chains: vec!["节点选择".to_owned(), "🇭🇰 香港 01".to_owned()],
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
                    chains: vec!["DIRECT".to_owned()],
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

/// Bare-Chinese label for the shared connections stream phase (DUAL-13-01).
fn stream_phase_label(
    phase: infiltrator_contract::connection::ConnectionStreamPhase,
) -> &'static str {
    use infiltrator_contract::connection::ConnectionStreamPhase;
    match phase {
        ConnectionStreamPhase::Idle => "未连接",
        ConnectionStreamPhase::Connecting => "连接中",
        ConnectionStreamPhase::Live => "实时",
        ConnectionStreamPhase::Reconnecting => "重连中",
        ConnectionStreamPhase::Unavailable => "不可用",
    }
}

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
        vec![
            Box::new(bsn! {
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
                                    ( Text({ "● 连接流 · 未连接".to_owned() }) ConnectionsLine(ConnectionsLineKind::Stream) TextRole(Role::Caption) ),
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
                                    ( { conn_aggregation_pill(ConnectionGroupingMode::Flat, "全部连接 (Flat)", true, palette) } ),
                                    ( { conn_aggregation_pill(ConnectionGroupingMode::ByProcess, "按应用进程聚合 (By Process)", false, palette) } ),
                                    ( { conn_aggregation_pill(ConnectionGroupingMode::ByHost, "按目标域名聚合 (By Host)", false, palette) } ),
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
                                    ( Text({ "关闭全部连接".to_owned() }) CloseAllConnectionsLabel TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    (
                        Node {
                            flex_grow: 1.0,
                            min_width: px(0.0),
                        }
                        ConnSearchField
                        Children [
                            ( { text_field_with_placeholder_scene(
                                String::new(),
                                "按域名/IP/进程即时搜索连接".to_owned(),
                                palette,
                            ) } ),
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
                        BackgroundColor({ palette.surface_elevated })
                        Button
                        CloseFilteredConnectionsButton
                        Children [
                            ( Text({ "断开筛选结果".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            }),
            Box::new(conn_idle_controls_scene(palette)),
        ],
        palette,
    )
}

/// DUAL-13-11: shared idle-timeout choices + manual sweep + honest status.
fn conn_idle_controls_scene(palette: &UiPalette) -> impl Scene + use<> {
    crate::pages::connections_idle::conn_idle_controls_scene(palette)
}

fn conn_aggregation_pill(
    mode: ConnectionGroupingMode,
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
                    display: Display::None,
                    align_items: AlignItems::Center,
                }
                ConnAggregationSummaryContainer
                Children [
                    ( Text({ String::new() }) ConnAggregationSummary TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                }
                ConnRowsContainer
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
    let chain_scenes = connection_chain_scenes(idx, conn);
    let speed_info = format!(
        "↑ {}  ↓ {}",
        format_rate(conn.upload_bps),
        format_rate(conn.download_bps)
    );
    let conn_btn = CloseConnectionButton {
        connection_id: conn.id.clone(),
        connection_idx: idx,
    };
    let inspect_btn = ConnInspectButton(idx);

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
            }
            ConnectionRow(idx)
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
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S4),
                            }
                            Children [
                                { chain_scenes },
                            ]
                        ),
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
                            template_value(inspect_btn)
                            Children [
                                ( Text({ "详情".to_owned() }) TextRole(Role::Caption) ),
                            ]
                        ),
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

/// DUAL-13-06: render one flat row's route chain as one text per hop, through
/// the shared parsed chain model (no pre-joined string).
fn connection_chain_scenes(idx: usize, conn: &ConnectionItem) -> Vec<Box<dyn Scene>> {
    let chain = connection_view::route_chain(conn);
    let mut scenes: Vec<Box<dyn Scene>> = vec![Box::new(bsn! {
        ( Text({ "链路: ".to_owned() }) TextRole(Role::Caption) )
    }) as Box<dyn Scene>];
    if chain.is_empty() {
        scenes.push(Box::new(bsn! {
            ( Text({ "DIRECT".to_owned() }) TextRole(Role::Caption) )
        }) as Box<dyn Scene>);
        return scenes;
    }
    for (hop, label) in chain.hops().iter().enumerate() {
        if hop > 0 {
            scenes.push(Box::new(bsn! {
                ( Text({ "→".to_owned() }) TextRole(Role::Caption) )
            }) as Box<dyn Scene>);
        }
        let hop_label = label.clone();
        scenes.push(Box::new(bsn! {
            ( Text(hop_label) template_value(ConnChainHopText { row: idx, hop }) TextRole(Role::Caption) )
        }) as Box<dyn Scene>);
    }
    scenes
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
    commands.add_observer(on_connections_view_activated);
    commands.add_observer(crate::pages::connections_drawer::on_connections_drawer_activated);
    commands.add_observer(crate::pages::connections_idle::on_connections_idle_activated);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn on_connections_action_activated(
    activate: On<Activate>,
    close_all_buttons: Query<(), With<CloseAllConnectionsButton>>,
    close_row_buttons: Query<&CloseConnectionButton>,
    close_filtered_buttons: Query<(), With<CloseFilteredConnectionsButton>>,
    search_fields: Query<&Children, With<ConnSearchField>>,
    text_fields: Query<&TextField>,
    mut close_all_labels: Query<&mut Text, With<CloseAllConnectionsLabel>>,
    mut close_all_state: Option<ResMut<ConnectionsCloseAllState>>,
    last: Option<Res<LastConnectionsProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if close_all_buttons.contains(activate.entity) {
        // DUAL-13-08: destructive teardown is armed behind a second click;
        // only the armed click submits the shared command.
        let Some(state) = close_all_state.as_deref_mut() else {
            handle.submit(UiCommand::CloseAllConnections);
            return;
        };
        if !state.armed {
            state.armed = true;
            for mut text in &mut close_all_labels {
                text.0 = "确认关闭全部？再次点击执行".to_owned();
            }
        } else {
            state.armed = false;
            for mut text in &mut close_all_labels {
                text.0 = "关闭全部连接".to_owned();
            }
            handle.submit(UiCommand::CloseAllConnections);
        }
    } else if let Ok(btn) = close_row_buttons.get(activate.entity) {
        handle.submit(UiCommand::CloseConnection {
            id: btn.connection_id.clone(),
        });
    } else if close_filtered_buttons.contains(activate.entity) {
        // DUAL-13-07: range teardown reuses the shared keyword predicate
        // (DUAL-13-13); an empty filter never tears anything down.
        let query = search_field_text(&search_fields, &text_fields).unwrap_or_default();
        if query.trim().is_empty() {
            return;
        }
        if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
            for item in projection
                .connections
                .iter()
                .filter(|item| connection_view::matches_search(*item, &query))
            {
                handle.submit(UiCommand::CloseConnection {
                    id: item.id.clone(),
                });
            }
        }
    }
}

/// DUAL-13-02: the aggregation segmented control switches the shared grouping
/// mode and restamps the summary line + row visibility from the shared
/// reduction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn on_connections_view_activated(
    activate: On<Activate>,
    mut pills: Query<(&mut BackgroundColor, &ConnAggregationPill)>,
    mut summaries: Query<(&mut Text, &ConnAggregationSummary)>,
    mut summary_containers: Query<
        &mut Node,
        (
            With<ConnAggregationSummaryContainer>,
            Without<ConnRowsContainer>,
        ),
    >,
    mut rows_containers: Query<
        &mut Node,
        (
            With<ConnRowsContainer>,
            Without<ConnAggregationSummaryContainer>,
        ),
    >,
    palette: Res<UiPalette>,
    mut view_state: Option<ResMut<ConnectionsViewState>>,
    last: Option<Res<LastConnectionsProjection>>,
) {
    let Ok((_, pill)) = pills.get(activate.entity) else {
        return;
    };
    let mode = pill.0;
    if let Some(state) = view_state.as_deref_mut() {
        state.grouping = mode;
    }
    restamp_aggregation_pills(&palette, &mut pills, mode);
    if let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) {
        restamp_aggregation_summary(&mut summaries, projection, mode);
    }
    let flat = mode.is_flat();
    for mut node in &mut summary_containers {
        node.display = if flat { Display::None } else { Display::Flex };
    }
    for mut node in &mut rows_containers {
        node.display = if flat { Display::Flex } else { Display::None };
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
            Without<ConnChainHopText>,
            Without<ConnAggregationSummary>,
        ),
    >,
    mut speeds: Query<
        (&mut Text, &ConnSpeedText),
        (
            With<ConnSpeedText>,
            Without<ConnectionsLine>,
            Without<ConnHostText>,
            Without<ConnProcessText>,
            Without<ConnChainHopText>,
            Without<ConnAggregationSummary>,
        ),
    >,
    mut hosts: Query<
        (&mut Text, &ConnHostText),
        (
            With<ConnHostText>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnProcessText>,
            Without<ConnChainHopText>,
            Without<ConnAggregationSummary>,
        ),
    >,
    mut processes: Query<
        (&mut Text, &ConnProcessText),
        (
            With<ConnProcessText>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnHostText>,
            Without<ConnChainHopText>,
            Without<ConnAggregationSummary>,
        ),
    >,
    mut chains: Query<
        (&mut Text, &ConnChainHopText),
        (
            With<ConnChainHopText>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnHostText>,
            Without<ConnProcessText>,
            Without<ConnAggregationSummary>,
        ),
    >,
    mut buttons: Query<&mut CloseConnectionButton>,
    mut summaries: Query<
        (&mut Text, &ConnAggregationSummary),
        (
            With<ConnAggregationSummary>,
            Without<ConnectionsLine>,
            Without<ConnSpeedText>,
            Without<ConnHostText>,
            Without<ConnProcessText>,
            Without<ConnChainHopText>,
        ),
    >,
    mut pills: Query<(&mut BackgroundColor, &ConnAggregationPill)>,
    palette: Res<UiPalette>,
    view_state: Option<Res<ConnectionsViewState>>,
    mut idle: Option<ResMut<ConnectionsIdleState>>,
) {
    let projection = &update.0;

    // DUAL-13-11: record byte-change times for the idle sweeper.
    if let Some(idle) = idle.as_deref_mut() {
        idle.tracker
            .observe(&projection.connections, current_unix_secs());
    }

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
            ConnectionsLineKind::Stream => {
                text.0 = format!("● 连接流 · {}", stream_phase_label(projection.stream_phase));
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
        // DUAL-13-06: each hop slot restamps from the shared parsed chain.
        text.0 = projection
            .connections
            .get(marker.row)
            .map(connection_view::route_chain)
            .and_then(|chain| chain.hops().get(marker.hop).cloned())
            .unwrap_or_default();
    }

    for mut btn in &mut buttons {
        if let Some(conn) = projection.connections.get(btn.connection_idx) {
            btn.connection_id = conn.id.clone();
        }
    }

    // DUAL-13-02: a new projection restamps the aggregation view from the
    // shared reduction so grouped mode never shows stale buckets.
    let grouping = view_state.map(|state| state.grouping).unwrap_or_default();
    restamp_aggregation_summary(&mut summaries, projection, grouping);
    restamp_aggregation_pills(&palette, &mut pills, grouping);

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
