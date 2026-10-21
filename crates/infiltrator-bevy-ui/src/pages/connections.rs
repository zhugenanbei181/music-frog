//! The Connections page (连接审计): active connections tracker, host/process
//! inspection, matched rule tracer, throughput rates, and disconnect actions.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ConnectionsLine`],
//! [`ConnSpeedText`], [`ConnHostText`], [`ConnPortText`],
//! [`ConnProcessTagText`], [`ConnRuleText`], [`ConnChainHopText`],
//! [`CloseConnectionButton`]). [`ConnectionsPagePlugin`] registers
//! [`apply_connections_projection`] and action observers once at product
//! assembly.
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
    CloseFilteredConnectionsButton, ConnAggregationSummary, ConnAggregationSummaryContainer,
    ConnRowsContainer, ConnSearchField, ConnectionsViewState, on_connections_strip_changed,
    search_field_text, seed_connections_strips, sort_strip_scene, sync_connections_strips,
    view_filter_strip_scene,
};
use crate::pages::connections_virtual::{CONNECTIONS_VIRTUAL_THRESHOLD, ConnectionsVirtualNodes};
use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, Update};
use bevy::color::{Alpha, Color};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut, SystemParam};
use bevy::picking::Pickable;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, Display, FlexDirection, FlexWrap, JustifyContent,
    Node, Overflow, PositionType, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_bevy_widgets::button::{
    ButtonDisabled, ButtonSize, ButtonVariant, button_with_label_scene,
};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{
    LocalizedLabel, LocalizedPlaceholder, LocalizedText, UiLocale,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::connection::ConnectionStreamPhase;
use infiltrator_contract::surface_snapshot::ConnectionSnapshot;
use infiltrator_domain::connection_view;
use infiltrator_domain::connection_view::{ConnectionGroupingMode, ConnectionSortKey};
use infiltrator_shared::byte_units::format_bytes;
use infiltrator_shared::locales::{Lang, Localizer};

/// Diameter of the stream-phase status dot (baseline pixels).
const STREAM_PHASE_DOT_PX: f32 = 6.0;

/// Height of one strip of the scroll-edge feather (baseline pixels).
const SCROLL_FADE_STRIP_PX: f32 = 3.0;

/// Number of strips the scroll-edge feather is built from.
const SCROLL_FADE_STRIPS: usize = 8;

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
}

/// Marker for a connection row's rate display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnSpeedText(pub usize);

/// Marker for a connection row's host display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnHostText(pub usize);

/// Marker for a connection row's destination port display. The port is its
/// own text so it can carry its own ink (the endpoint is not one flat string).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnPortText(pub usize);

/// Marker for a connection row's process tag. The tag is the process name in
/// its own chip, so the rule beside it is not a flat text pile.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnProcessTagText(pub usize);

/// Marker for a connection row's matched-rule display.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[require(ConnectionRowText)]
pub struct ConnRuleText(pub usize);

/// Marker for the connections telemetry stream-phase label.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnStreamPhaseLabel;

/// Marker for the connections telemetry stream-phase status dot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnStreamPhaseDot;

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
#[require(ButtonDisabled)]
pub struct CloseConnectionButton {
    pub connection_id: String,
    pub connection_idx: usize,
}

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

/// The shared phase copy carries a leading bullet glyph; the page draws the
/// live marker as its own aligned dot, so the glyph leaves the label.
pub fn strip_phase_marker(label: &str) -> &str {
    label.trim_start_matches('●').trim_start()
}

/// The status dot's ink for one telemetry phase.
fn stream_phase_color(phase: ConnectionStreamPhase, palette: &UiPalette) -> Color {
    match phase {
        ConnectionStreamPhase::Live => palette.success,
        ConnectionStreamPhase::Connecting | ConnectionStreamPhase::Reconnecting => palette.warning,
        ConnectionStreamPhase::Idle => palette.ink_dim,
        ConnectionStreamPhase::Unavailable => palette.disabled_ink,
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

    // BANDROID-010: large projections mount through the recycler (bounded
    // window + spacers). Small projections keep the full-mount vocabulary and
    // its stable-identity reconcile contract.
    let virtualized = projection.connections.len() > CONNECTIONS_VIRTUAL_THRESHOLD;
    let connection_scenes: Vec<Box<dyn Scene>> = if virtualized {
        Vec::new()
    } else {
        projection
            .connections
            .iter()
            .enumerate()
            .map(|(idx, item)| Box::new(connection_row_scene(idx, item, palette)) as Box<dyn Scene>)
            .collect()
    };

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S16),
                overflow: Overflow::clip(),
            }
            PageRoot(Route::Connections)
            ConnectionsPageRoot
            Children [
                Node {
                    width: percent(100), height: percent(100),
                    min_height: px(0.0), flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S16), overflow: Overflow::scroll_y(),
                }
                ScrollArea ConnectionsScrollArea
                Children [
                @{ header_card_scene(summary, traffic, palette) }
                --
                @{ connections_table_scene(connection_scenes, virtualized, palette) }
                ]
                --
                @{ connection_drawer_scene(palette) }
                --
                @{ scroll_fade_scene(palette) }
            ]
    }
}

/// The scroll-edge feather: the content dissolves into the page background
/// instead of being cut off by the viewport edge.
///
/// The feather never blocks the pointer, so the wheel keeps scrolling the
/// list underneath it.
fn scroll_fade_scene(palette: &UiPalette) -> impl Scene + use<> {
    let strips: Vec<Box<dyn Scene>> = (0..SCROLL_FADE_STRIPS)
        .map(|index| {
            let alpha = (index + 1) as f32 / SCROLL_FADE_STRIPS as f32;
            Box::new(bsn! {
                    Node {
                        width: percent(100),
                        height: rem(SCROLL_FADE_STRIP_PX),
                    }
                    BackgroundColor({ palette.window_clear.with_alpha(alpha) })
            }) as Box<dyn Scene>
        })
        .collect();
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                right: px(0.0),
                bottom: px(0.0),
                height: rem(SCROLL_FADE_STRIP_PX * SCROLL_FADE_STRIPS as f32),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                overflow: Overflow::clip(),
            }
            Pickable { should_block_lower: false, is_hoverable: true }
            Children [
                { strips }
            ]
    }
}

/// The header card: the identity column, the view filter and the quiet
/// destructive trigger.
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
                                row_gap: rem(space::S12),
                                column_gap: rem(space::S16),
                            }
                            AccessibilityNode(header_a11y) LocalizedLabel::plain("nav_connections")
                            Children [
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: rem(space::S12),
                                    flex_grow: 1.0,
                                    min_width: px(0.0),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::Network, 36.0, palette) }
                                    --
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: rem(space::S4),
                                    }
                                    Children [
                                        LocalizedText { .. { summary } } ConnectionsLine(ConnectionsLineKind::Summary) TextRole(Role::Heading)
                                        --
                                        LocalizedText { .. { traffic } } ConnectionsLine(ConnectionsLineKind::TrafficSummary) TextRole(Role::Body)
                                        --
                                        @{ stream_phase_line_scene(palette) }
                                    ]
                                ]
                                --
                                @{ close_all_button_scene(palette) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: rem(space::S8),
                                flex_wrap: FlexWrap::Wrap,
                                row_gap: rem(space::S8),
                            }
                            Children [
                                @{ view_filter_strip_scene(ConnectionGroupingMode::default(), palette) }
                            ]
            }),
            Box::new(conn_idle_controls_scene(palette)),
        ],
        palette,
    )
}

/// The close-all trigger: a quiet outlined control, never a solid danger
/// slab.
///
/// The destructive moment is the CONFIRMATION modal (the official
/// `ModalDialog` layer with the danger confirm button), so the trigger only
/// asks for it — a red block would invite the very mis-click the modal
/// exists to prevent.
fn close_all_button_scene(palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain("connections_close_all_action") TextRole(Role::Body) },
                ButtonVariant::Outline,
                ButtonSize::Sm,
                palette,
            ) }
            CloseAllConnectionsButton
    }
}

/// The stream-phase line: the telemetry state as a status dot and a label.
///
/// The dot is its own element, so it is tinted by the phase and sits on the
/// label's optical centre instead of riding the baseline as a glyph.
fn stream_phase_line_scene(palette: &UiPalette) -> impl Scene + use<> {
    let language = UiLocale::default().code().to_string();
    let label = strip_phase_marker(&Lang(&language).tr("connections_stream_live")).to_owned();
    bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: rem(space::S6),
            }
            Children [
                Node {
                    width: rem(STREAM_PHASE_DOT_PX),
                    height: rem(STREAM_PHASE_DOT_PX),
                    border_radius: BorderRadius::MAX,
                    margin: UiRect::bottom(rem(space::S4)),
                }
                BackgroundColor({ palette.success })
                ConnStreamPhaseDot
                --
                Text({ label }) ConnStreamPhaseLabel TextRole(Role::Caption)
            ]
    }
}

/// DUAL-13-11: shared idle-timeout choices + manual sweep + honest status.
fn conn_idle_controls_scene(palette: &UiPalette) -> impl Scene + use<> {
    connections_idle::conn_idle_controls_scene(palette)
}

/// The connection table card: the title, the row-order strip, the search row
/// and the rows themselves.
fn connections_table_scene(
    connection_scenes: Vec<Box<dyn Scene>>,
    virtualized: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let rows_scene: Box<dyn Scene> = if virtualized {
        // The recycler owns the row children; the container keeps its
        // `ConnRowsContainer` marker so the shared search/grouping display
        // reductions still address it.
        Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
            }
            ConnRowsContainer
            ConnectionsVirtualNodes
        })
    } else {
        Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
            }
            ConnRowsContainer
            Children [
                { connection_scenes }
            ]
        })
    };
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::FlexStart,
                                justify_content: JustifyContent::SpaceBetween,
                                flex_direction: FlexDirection::Column,
                                column_gap: rem(space::S8),
                            }
                            Children [
                                Node {
                                    width: percent(100),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::SpaceBetween,
                                    column_gap: rem(space::S8),
                                }
                                Children [
                                    LocalizedText::plain("connections_sessions_title") TextRole(Role::BodyStrong)
                                    --
                                    LocalizedText::plain("connections_sessions_hint") TextRole(Role::Caption)
                                ]
                                --
                                @{ sort_strip_scene(ConnectionSortKey::default(), palette) }
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: rem(space::S8),
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
                                @{ quiet_button_scene("common_clear", ClearConnectionsSearch, palette) }
                                --
                                @{ quiet_button_scene("conn_close_filtered_btn", CloseFilteredConnectionsButton, palette) }
                            ]
            }),
            Box::new(bsn! {
                LocalizedText::plain("conn_search_results") ConnectionsSearchSummary TextRole(Role::Caption)
            }),
            Box::new(bsn! {
                Node { width: percent(100), display: Display::None, padding: UiRect::all(rem(space::S16)) } ConnectionsSearchEmpty
                Children [ LocalizedText::plain("runtime_no_matching_connections") TextRole(Role::Body) ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                display: Display::None,
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Stretch,
                            }
                            ConnAggregationSummaryContainer
                            Children [
                                LocalizedText::plain("conn_aggregate_empty") ConnAggregationSummary TextRole(Role::Caption)
                                --
                                Node {width: percent(100), flex_direction: FlexDirection::Column, row_gap: rem(space::S8)} ConnectionGroupsRoot
                            ]
            }),
            rows_scene,
        ],
        palette,
    )
}

/// A quiet inline action: no fill, no frame — the row's own actions must not
/// read as a wall of buttons.
fn quiet_button_scene<M: Component + Clone + Unpin>(
    key: &'static str,
    marker: M,
    palette: &UiPalette,
) -> impl Scene + use<M> {
    bsn! {
            @{ button_with_label_scene(
                bsn! { LocalizedText::plain(key) TextRole(Role::Caption) },
                ButtonVariant::Ghost,
                ButtonSize::Sm,
                palette,
            ) }
            marker
    }
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
        app.add_systems(
            Update,
            (
                seed_connections_strips.before(sync_connections_strips),
                sync_connections_strips,
                sync_connections_phase_line,
            ),
        );
        app.add_observer(on_connections_strip_changed);
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
    if !view.page.source_current() {
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

#[derive(SystemParam)]
pub(crate) struct ConnectionPresentation<'w, 's> {
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
}
pub(crate) fn apply_connections_projection(
    update: On<ConnectionsProjectionUpdated>,
    presentation: ConnectionPresentation,
) {
    let ConnectionPresentation {
        locale,
        mut view_state,
        mut idle,
        mut last,
        mut lines,
    } = presentation;
    let projection = &update.0;
    if projection.stream_phase != ConnectionStreamPhase::Live {
        if let Some(state) = view_state.as_deref_mut() {
            state.page.mark_unavailable();
        }
        if let Some(last) = last.as_deref_mut()
            && let Some(previous) = last.0.as_mut()
        {
            previous.stream_phase = projection.stream_phase;
        }
        return;
    }
    if let Some(state) = view_state.as_deref_mut() {
        state.page.observe(&projection.connections);
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
        };
        if *copy != value {
            *copy = value;
        }
        text.0 = copy.render(&locale);
    }

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

/// The telemetry phase line: the dot's ink and the label follow the shared
/// phase, and the label follows the locale.
pub(crate) fn sync_connections_phase_line(
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    last: Option<Res<LastConnectionsProjection>>,
    mut labels: Query<&mut Text, With<ConnStreamPhaseLabel>>,
    mut dots: Query<&mut BackgroundColor, With<ConnStreamPhaseDot>>,
) {
    let phase = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .map(|projection| projection.stream_phase)
        .unwrap_or(ConnectionStreamPhase::Unavailable);
    let copy = Lang(locale.code()).tr(stream_phase_key(phase));
    let label = strip_phase_marker(&copy);
    for mut text in &mut labels {
        if text.0 != label {
            text.0 = label.to_owned();
        }
    }
    let color = stream_phase_color(phase, &palette);
    for mut dot in &mut dots {
        if dot.0 != color {
            dot.0 = color;
        }
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

    #[test]
    fn the_phase_marker_leaves_the_label() {
        assert_eq!(strip_phase_marker("● 连接流 · 实时"), "连接流 · 实时");
        assert_eq!(
            strip_phase_marker("● Connection stream · live"),
            "Connection stream · live"
        );
    }
}
