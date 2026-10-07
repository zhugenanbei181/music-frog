//! The Connections page (连接审计): active connections tracker, host/process
//! inspection, matched rule tracer, throughput rates, and disconnect actions.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ConnectionsLine`],
//! [`ConnSpeedText`], [`ConnHostText`], [`ConnProcessText`],
//! [`ConnChainHopText`],
//! [`CloseConnectionButton`]). [`ConnectionsPagePlugin`] registers [`apply_connections_projection`]
//! and action observers once at product assembly.
//!
//! Row-level rendering ([`connection_chain_scenes`], [`connection_pulse_scene`])
//! and route chain derivation ([`connection_view::route_chain`]) are organized
//! under `connections_row` to support swipe-to-action within the line budget.

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::connections_clipboard::copy_connection_host;
use crate::pages::connections_confirm::on_confirmation_activated;
use crate::pages::connections_drawer::{connection_drawer_scene, on_connections_drawer_activated};
use crate::pages::connections_groups::{ConnectionGroupsRoot, ConnectionsGroupsPlugin};
use crate::pages::connections_idle;
use crate::pages::connections_idle::{ConnectionsIdleState, current_unix_secs};
use crate::pages::connections_row::connection_row_scene;
use crate::pages::connections_rows::{ConnectionRowText, ConnectionsRowsPlugin};
use crate::pages::connections_search::{
    ClearConnectionsSearch, ConnectionsSearchEmpty, ConnectionsSearchPlugin,
    ConnectionsSearchSummary,
};
use crate::pages::connections_view::{
    CloseAllConnectionsLabel, CloseFilteredConnectionsButton, ConnAggregationSummary,
    ConnAggregationSummaryContainer, ConnRowsContainer, ConnSearchField, ConnSortPill,
    ConnectionsViewState, on_connections_sort_activated, restamp_aggregation_pills,
    restamp_sort_pills, search_field_text, sort_pills_scene,
};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent,
    Node, Overflow, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, ScrollArea};
use infiltrator_application::byte_format::format_bytes;
use infiltrator_application::connection_grouping::grouping_label_key;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{
    LocalizedLabel, LocalizedPlaceholder, LocalizedText, UiLocale,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;
use infiltrator_domain::connection_view::ConnectionGroupingMode;

/// Root marker on the Connections page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ConnectionsPageRoot;

#[derive(Component, Clone, Copy, Default)]
pub struct ConnectionsScrollArea;

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
#[require(ConnectionRowText)]
pub struct ConnSpeedText(pub usize);

/// Marker for a connection row's host display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnHostText(pub usize);

/// Marker for a connection row's process/rule display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnProcessText(pub usize);

/// Marker for a connection row's route-chain hop display (DUAL-13-06).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
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
#[require(ButtonDisabled)]
pub struct CloseConnectionButton {
    pub connection_id: String,
    pub connection_idx: usize,
}

/// Marker component for the connection aggregation segmented control pills.
/// The payload is the shared domain grouping mode (DUAL-13-02), so Bevy and
/// Iced switch the same modes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnAggregationPill(pub ConnectionGroupingMode);

/// Snapshot of the Connections domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ConnectionsProjection {
    pub total_connections: usize,
    pub total_upload_bytes: u64,
    pub total_download_bytes: u64,
    /// DUAL-13-01: lifecycle phase of the connections telemetry feed.
    pub stream_phase: ConnectionStreamPhase,
    pub connections: Vec<ConnectionSnapshot>,
}

/// The typed event dispatched when connection data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct ConnectionsProjectionUpdated(pub ConnectionsProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastConnectionsProjection(pub Option<ConnectionsProjection>);

// ---- Scene constructors ---------------------------------------------------

fn stream_phase_key(phase: ConnectionStreamPhase) -> &'static str {
    match phase {
        ConnectionStreamPhase::Idle => "connections_stream_idle",
        ConnectionStreamPhase::Connecting => "connections_stream_connecting",
        ConnectionStreamPhase::Live => "connections_stream_live",
        ConnectionStreamPhase::Reconnecting => "connections_stream_reconnecting",
        ConnectionStreamPhase::Unavailable => "connections_stream_unavailable",
    }
}

pub fn connections_page(
    projection: &ConnectionsProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let summary = LocalizedText::new(
        "connections_active_summary",
        vec![("count", projection.total_connections.to_string())],
    );
    let traffic = LocalizedText::new(
        "connections_traffic_summary",
        vec![
            ("upload", format_bytes(projection.total_upload_bytes)),
            ("download", format_bytes(projection.total_download_bytes)),
        ],
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
                overflow: Overflow::clip(),
            }
            PageRoot(Route::Connections)
            ConnectionsPageRoot
            Children [
                Node {
                    width: percent(100), height: percent(100),
                    min_height: px(0.0), flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(space::S16), overflow: Overflow::scroll_y(),
                }
                ScrollArea ConnectionsScrollArea
                Children [
                @{ header_card_scene(summary, traffic, palette) }
                --
                @{ connections_table_scene(connection_scenes, palette) }
                ]
                --
                @{ connection_drawer_scene(palette) }
            ]
    }
}

fn header_card_scene(
    summary: LocalizedText,
    traffic: LocalizedText,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("");

    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: px(space::S8),
                                column_gap: Val::Px(space::S16),
                            }
                            AccessibilityNode(header_a11y) LocalizedLabel::plain("nav_connections")
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S12),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::Network, 36.0, palette) }
                                    --
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(space::S4),
                                    }
                                    Children [
                                        LocalizedText { .. { summary } } ConnectionsLine(ConnectionsLineKind::Summary) TextRole(Role::Heading)
                                        --
                                        LocalizedText { .. { traffic } } ConnectionsLine(ConnectionsLineKind::TrafficSummary) TextRole(Role::Caption)
                                        --
                                        LocalizedText::plain("connections_stream_idle") ConnectionsLine(ConnectionsLineKind::Stream) TextRole(Role::Caption)
                                    ]
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                    flex_wrap: FlexWrap::Wrap,
                                    row_gap: px(space::S8),
                                }
                                Children [
                                    Node {
                                        align_items: AlignItems::Center,
                                        padding: UiRect::all(Val::Px(2.0)),
                                        border: UiRect::all(Val::Px(palette.hairline_px)),
                                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                        column_gap: Val::Px(space::S4),
                                        flex_wrap: FlexWrap::Wrap,
                                    }
                                    BackgroundColor({ palette.surface_elevated })
                                    BorderColor {
                                        top: { palette.border },
                                        right: { palette.border },
                                        bottom: { palette.border },
                                        left: { palette.border },
                                    }
                                    Children [
                                        @{ conn_aggregation_pill(ConnectionGroupingMode::Flat, true, palette) }
                                        --
                                        @{ conn_aggregation_pill(ConnectionGroupingMode::ByProcess, false, palette) }
                                        --
                                        @{ conn_aggregation_pill(ConnectionGroupingMode::ByHost, false, palette) }
                                    ]
                                    --
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
                                        LocalizedText::plain("connections_close_all_action") CloseAllConnectionsLabel TextRole(Role::BodyStrong)
                                    ]
                                ]
                            ]
            }),
            Box::new(conn_idle_controls_scene(palette)),
        ],
        palette,
    )
}

/// DUAL-13-11: shared idle-timeout choices + manual sweep + honest status.
fn conn_idle_controls_scene(palette: &UiPalette) -> impl Scene + use<> {
    connections_idle::conn_idle_controls_scene(palette)
}

fn conn_aggregation_pill(
    mode: ConnectionGroupingMode,
    active: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let (bg, text_color) = if active {
        (palette.accent_container, palette.accent)
    } else {
        (palette.surface, palette.ink_dim)
    };
    let label = LocalizedText::plain(grouping_label_key(mode));

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
                LocalizedText { .. { label } } TextRole(Role::Caption) TextColor({ text_color })
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
                                column_gap: Val::Px(space::S8),
                                padding: UiRect::bottom(Val::Px(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("connections_sessions_title") TextRole(Role::BodyStrong)
                                --
                                @{ sort_pills_scene(palette) }
                                --
                                LocalizedText::plain("connections_sessions_hint") TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                Node {
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                }
                                ConnSearchField
                                Children [
                                    @{ text_field_with_placeholder_scene(
                                            String::new(),
                                            LocalizedText::plain("connections_search_placeholder").render(&UiLocale::default()),
                                            palette,
                                    ) } LocalizedPlaceholder::plain("connections_search_placeholder") NativeTextField(10)
                                ]
                                --
                                Node { min_height: px(palette.control_height_px), padding: UiRect::horizontal(px(space::S8)), align_items: AlignItems::Center }
                                Button ClearConnectionsSearch
                                Children [ LocalizedText::plain("common_clear") TextRole(Role::Caption) ]
                                --
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
                                    LocalizedText::plain("conn_close_filtered_btn") TextRole(Role::Caption)
                                ]
                            ]
            }),
            Box::new(bsn! {
                LocalizedText::plain("conn_search_results") ConnectionsSearchSummary TextRole(Role::Caption)
            }),
            Box::new(bsn! {
                Node { width: percent(100), display: Display::None, padding: UiRect::all(px(space::S16)) } ConnectionsSearchEmpty
                Children [ LocalizedText::plain("runtime_no_matching_connections") TextRole(Role::Body) ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                display: Display::None,
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Stretch,
                                row_gap: px(space::S8),
                            }
                            ConnAggregationSummaryContainer
                            Children [
                                LocalizedText::plain("conn_aggregate_empty") ConnAggregationSummary TextRole(Role::Caption)
                                --
                                Node {width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(space::S8)} ConnectionGroupsRoot
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
                                { connection_scenes }
                            ]
            }),
        ],
        palette,
    )
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct ConnectionsPagePlugin;

impl Plugin for ConnectionsPagePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ConnectionsGroupsPlugin,
            ConnectionsRowsPlugin,
            ConnectionsSearchPlugin,
        ));
        app.add_observer(apply_connections_projection);
        app.add_observer(on_connections_action_activated);
        app.add_observer(on_confirmation_activated);
        app.add_observer(on_connections_view_activated);
        app.add_observer(on_connections_sort_activated);
        app.add_observer(on_connections_drawer_activated);
        app.add_observer(copy_connection_host);
        app.add_observer(connections_idle::on_connections_idle_activated);
    }
}

#[derive(SystemParam)]
pub(crate) struct ConnectionActionControls<'w, 's> {
    close_row_buttons: Query<'w, 's, &'static CloseConnectionButton>,
    close_filtered_buttons: Query<'w, 's, (), With<CloseFilteredConnectionsButton>>,
    search_fields: Query<'w, 's, &'static Children, With<ConnSearchField>>,
    text_fields: Query<'w, 's, &'static TextField>,
    last: Option<Res<'w, LastConnectionsProjection>>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    view: Res<'w, ConnectionsViewState>,
}
pub(crate) fn on_connections_action_activated(
    activate: On<Activate>,
    controls: ConnectionActionControls,
) {
    let ConnectionActionControls {
        close_row_buttons,
        close_filtered_buttons,
        search_fields,
        text_fields,
        last,
        handle,
        view,
    } = controls;
    if !view.groups.source_current() {
        return;
    }
    let Some(handle) = handle else {
        return;
    };
    if let Ok(btn) = close_row_buttons.get(activate.entity) {
        if !last
            .as_ref()
            .and_then(|last| last.0.as_ref())
            .is_some_and(|projection| {
                projection
                    .connections
                    .iter()
                    .any(|row| row.id == btn.connection_id)
            })
        {
            return;
        }
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
pub(crate) fn on_connections_view_activated(
    activate: On<Activate>,
    mut pills: Query<(&mut BackgroundColor, &ConnAggregationPill)>,
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
) {
    let Ok((_, pill)) = pills.get(activate.entity) else {
        return;
    };
    let mode = pill.0;
    if let Some(state) = view_state.as_deref_mut() {
        state.groups.select(mode);
    }
    restamp_aggregation_pills(&palette, &mut pills, mode);
    let flat = mode.is_flat();
    for mut node in &mut summary_containers {
        node.display = if flat { Display::None } else { Display::Flex };
    }
    for mut node in &mut rows_containers {
        node.display = if flat { Display::Flex } else { Display::None };
    }
}

#[derive(SystemParam)]
pub(crate) struct ConnectionPresentation<'w, 's> {
    palette: Res<'w, UiPalette>,
    locale: Res<'w, UiLocale>,
    view_state: Option<ResMut<'w, ConnectionsViewState>>,
    idle: Option<ResMut<'w, ConnectionsIdleState>>,
    last: Option<ResMut<'w, LastConnectionsProjection>>,
    lines: Query<
        'w,
        's,
        (
            &'static mut Text,
            &'static ConnectionsLine,
            &'static mut LocalizedText,
        ),
        With<ConnectionsLine>,
    >,
    pills: Query<
        'w,
        's,
        (&'static mut BackgroundColor, &'static ConnAggregationPill),
        Without<ConnSortPill>,
    >,
    sort_pills: Query<
        'w,
        's,
        (&'static mut BackgroundColor, &'static ConnSortPill),
        Without<ConnAggregationPill>,
    >,
}
pub(crate) fn apply_connections_projection(
    update: On<ConnectionsProjectionUpdated>,
    presentation: ConnectionPresentation,
) {
    let ConnectionPresentation {
        palette,
        locale,
        mut view_state,
        mut idle,
        mut last,
        mut lines,
        mut pills,
        mut sort_pills,
    } = presentation;
    let projection = &update.0;
    if projection.stream_phase != ConnectionStreamPhase::Live {
        if let Some(state) = view_state.as_deref_mut() {
            state.groups.mark_unavailable();
        }
        if let Some(last) = last.as_deref_mut()
            && let Some(previous) = last.0.as_mut()
        {
            previous.stream_phase = projection.stream_phase;
        }
        for (mut text, line, mut copy) in &mut lines {
            if line.0 == ConnectionsLineKind::Stream {
                *copy = LocalizedText::plain(stream_phase_key(projection.stream_phase));
                text.0 = copy.render(&locale);
            }
        }
        return;
    }
    if let Some(state) = view_state.as_deref_mut() {
        state.groups.observe(&projection.connections);
    }

    // DUAL-13-11: record byte-change times for the idle sweeper.
    if let Some(idle) = idle.as_deref_mut() {
        idle.tracker
            .observe(&projection.connections, current_unix_secs());
    }

    for (mut text, line, mut copy) in &mut lines {
        let value = match line.0 {
            ConnectionsLineKind::Summary => LocalizedText::new(
                "connections_active_summary",
                vec![("count", projection.total_connections.to_string())],
            ),
            ConnectionsLineKind::TrafficSummary => LocalizedText::new(
                "connections_traffic_summary",
                vec![
                    ("upload", format_bytes(projection.total_upload_bytes)),
                    ("download", format_bytes(projection.total_download_bytes)),
                ],
            ),
            ConnectionsLineKind::Stream => {
                LocalizedText::plain(stream_phase_key(projection.stream_phase))
            }
        };
        if *copy != value {
            *copy = value;
        }
        text.0 = copy.render(&locale);
    }

    // DUAL-13-02: a new projection restamps the aggregation view from the
    // shared reduction so grouped mode never shows stale buckets.
    let grouping = view_state
        .as_ref()
        .map(|state| state.groups.mode())
        .unwrap_or_default();
    restamp_aggregation_pills(&palette, &mut pills, grouping);

    // DUAL-13-12: the flat rows follow the shared sort key of the view state;
    // a fresh snapshot is re-ordered without touching the row markers.
    let sort = view_state
        .as_ref()
        .map(|state| state.sort)
        .unwrap_or_default();
    restamp_sort_pills(&palette, &mut sort_pills, sort);
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
