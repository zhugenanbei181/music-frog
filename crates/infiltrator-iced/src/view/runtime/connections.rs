//! Runtime page connections section: header with traffic totals, sort
//! segmented control, live search filter, close-all button, sub-tabs
//! (Active / Closed), and windowed connection card rows with process names,
//! protocol chips, outbound target badges, instantaneous rates and the
//! high-throughput pulse, plus kill buttons.

use crate::state::AppState;
use crate::state::runtime::ConnectionTab;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::view::runtime::{
    connection_search, connection_view_controls, connections_controls, connections_table,
};
use crate::view_root::interaction_regions::InteractionRegion;
use infiltrator_shared::byte_units::format_bytes;

use crate::view::component_forms::{row_card_surface, style_danger, style_ghost, text_btn};
use crate::view::components::{
    BadgeKind, badge, chip, empty_state, icon_button, modern_scrollable, section_header,
    segmented_control, status_dot,
};
use crate::view::observation_layout::connection_rows_use_stacked_headline;
use crate::view::svg_icons::Icon;
use crate::view::theme::{SP_MD, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};
use infiltrator_contract::connection_view_preference::ConnectionViewMode;
use infiltrator_domain::connection_activity::{IDLE_TIMEOUT_CHOICES, idle_timeout_minutes_label};
use infiltrator_domain::connection_observation::ConnectionObservationInstant;
use infiltrator_domain::connection_rate::{ConnectionRate, ConnectionRates};
use infiltrator_domain::connection_view::{ConnectionGroupingMode, RatedConnection};
use infiltrator_domain::runtime::Connection;
use infiltrator_domain::{connection_rate, connection_view};
use infiltrator_presentation::connection_grouping::sort_label_key;
use infiltrator_presentation::connection_page_model::{CONNECTION_SORT_OPTIONS, sort_option_at};
use infiltrator_presentation::connection_view_table_projection::{
    ConnectionTableStatus, project_connection_view_table,
};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionListScrollOwner {
    RuntimePage,
    ConnectionsPage,
}

/// Native capture operations address this owner, so its identity lives with
/// the widget that owns the standalone page's scroll.
pub(crate) const CONNECTIONS_PAGE_SCROLL_ID: &str = "connections-list-scroll";

/// Extract clean executable/binary name from a system process path.
pub fn extract_process_name(path: &str) -> String {
    connection_view::process_display_name(path)
}

/// Determine outbound target label and badge semantic color.
pub fn outbound_target_info(conn: &Connection) -> (String, BadgeKind) {
    let target = conn
        .chains
        .first()
        .cloned()
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| {
            if !conn.rule_payload.is_empty() {
                conn.rule_payload.clone()
            } else if !conn.rule.is_empty() {
                conn.rule.clone()
            } else {
                "DIRECT".to_string()
            }
        });

    let target_upper = target.to_uppercase();
    let kind = if target_upper == "DIRECT"
        || target.contains("\u{76f4}\u{8fde}")
        || target_upper == "DIRECT"
    {
        BadgeKind::Success
    } else if target_upper == "REJECT"
        || target.contains("\u{62d2}\u{7edd}")
        || target_upper == "REJECT"
    {
        BadgeKind::Danger
    } else {
        BadgeKind::Accent
    };

    (target, kind)
}

/// Whether the grouping control is mounted for the given readback.
///
/// The table shows one row per connection, so the grouping buckets have no
/// place there: a grouping control in the table morphology would fold rows
/// the table cannot show.
pub(crate) const fn grouping_control_mounted(mode: ConnectionViewMode) -> bool {
    matches!(mode, ConnectionViewMode::List)
}

/// DUAL-13-10: the breathing glow of one row at the shared pulse phase; zero
/// until the connection really crosses the shared high-throughput threshold.
pub fn connection_pulse_intensity(conn: &Connection, rates: &ConnectionRates, phase: f32) -> f32 {
    let rate = rates.get(&conn.id);
    connection_rate::pulse_intensity(rate.upload_bps, rate.download_bps, phase)
}

/// Bytes-per-second display; the value is a real derived rate, never a guess.
pub fn format_rate(bps: f64) -> String {
    format!("{}/s", format_bytes(bps.max(0.0) as u64))
}

/// The instant the Core accepted the connection list the page renders.
///
/// A page without an accepted list has no instant to measure against, so the
/// age column keeps the honest unknown. The surface never reads a clock of
/// its own.
fn observed_instant(state: &AppState) -> ConnectionObservationInstant {
    state
        .diag
        .connections
        .as_ref()
        .map(|observed| observed.instant())
        .unwrap_or(ConnectionObservationInstant::unobserved())
}

pub(crate) fn connections_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    connections_section_with_scroll_owner(state, lang, ConnectionListScrollOwner::RuntimePage)
}

pub(crate) fn connections_page_section<'a>(
    state: &'a AppState,
    lang: Lang<'a>,
) -> Element<'a, Message> {
    connections_section_with_scroll_owner(state, lang, ConnectionListScrollOwner::ConnectionsPage)
}

fn connections_section_with_scroll_owner<'a>(
    state: &'a AppState,
    lang: Lang<'a>,
    scroll_owner: ConnectionListScrollOwner,
) -> Element<'a, Message> {
    let _is_zh = state.shell.lang.starts_with("zh");

    // 1. Sort segmented control: cumulative download / cumulative upload /
    // instantaneous download / instantaneous upload / latest / host. The
    // instantaneous entries are DUAL-13-12; every position maps to the shared
    // `ConnectionSortKey`, and the shared page model owns the selection.
    let sort_labels: Vec<String> = CONNECTION_SORT_OPTIONS
        .iter()
        .map(|key| lang.tr(sort_label_key(*key)).to_string())
        .collect();
    let sort_index = state.diag.connections_model.sort_index();
    let conn_sort_control = segmented_control(&sort_labels, sort_index, |index| {
        Message::UpdateRuntimeConnectionSort(sort_option_at(index))
    });

    // 2. Traffic totals and stream status
    let (upload_total, download_total) = match &state.diag.connections {
        Some(c) => (c.snapshot().upload_total, c.snapshot().download_total),
        None => (0, 0),
    };
    let upload_badge = badge(
        format!("↑ {}", format_bytes(upload_total)),
        BadgeKind::Success,
    );
    let download_badge = badge(
        format!("↓ {}", format_bytes(download_total)),
        BadgeKind::Accent,
    );

    // 3. Close all connections button (Icon::Trash2 + danger style)
    let close_all_btn = button(
        row![
            svg_icons::icon_themed(Icon::Trash2, 13.0, |t: &Theme| tokens(t).danger),
            Space::new().width(4),
            theme::role_body_text(lang.tr("btn_close_all").to_string()),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_danger)
    .on_press(Message::RequestConfirmation(
        ConfirmAction::CloseAllConnections,
    ));

    // Keep metrics, sorting and destructive actions on independent native rows.
    let header_trailing = row![
        icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
        close_all_btn,
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);
    let traffic_summary = row![
        stream_badge(&state.diag.connections_stream_state, &lang),
        upload_badge,
        download_badge,
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center);

    // 5. Sub-tabs: the typed destination is state, not a string prefix.
    let user_filter = state.runtime.connection_query.trim();
    let is_closed_tab = matches!(state.runtime.connection_tab, ConnectionTab::Closed);
    let total_active_conns = state.diag.connections_model.source_count();
    let total_closed_conns = state.diag.closed_connections_model.source_count();
    let tab_labels = vec![
        format!("{} ({total_active_conns})", lang.tr("conn_status_active")),
        format!("{} ({total_closed_conns})", lang.tr("conn_status_closed")),
    ];
    let tab_index = usize::from(is_closed_tab);
    let sub_tabs = segmented_control(&tab_labels, tab_index, |idx| {
        Message::SetConnectionTab(if idx == 1 {
            ConnectionTab::Closed
        } else {
            ConnectionTab::Active
        })
    });

    // The user's persisted morphology decides what the page body is: the
    // list of cards or the table of columns. The readback is the owner's, so
    // the page never keeps a mode of its own and never renders both.
    let view_mode = state.diag.connection_view.mode;
    let table_mode = view_mode == ConnectionViewMode::Table;

    // The grouping control folds the rows into buckets, which the table
    // cannot show: a table row is one connection. In the table morphology the
    // control is not mounted at all, so no control is ever inert.
    let group_control =
        grouping_control_mounted(view_mode).then(|| connections_controls::grouping(state));
    let search_control = connections_controls::search(state);
    let connection_view_control = connection_view_controls::view_mode_controls(state);

    let close_filtered_btn = button(
        row![
            svg_icons::icon_themed(Icon::X, 13.0, |t: &Theme| tokens(t).warning),
            Space::new().width(4),
            theme::role_body_text(lang.tr("conn_close_filtered_btn").to_string()),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press_maybe((!user_filter.is_empty()).then_some(Message::CloseFilteredConnections));

    let filter_bar: Element<'_, Message> = if state.shell.viewport.tier.is_narrow() {
        let mut items: Vec<Element<'_, Message>> = vec![
            row![sub_tabs, close_filtered_btn]
                .spacing(theme::SP_SM)
                .align_y(Alignment::Center)
                .into(),
        ];
        if let Some(group_control) = group_control {
            items.push(group_control);
        }
        items.push(search_control);
        items.push(connection_view_control);
        column(items)
            .spacing(theme::SP_SM)
            .width(Length::Fill)
            .into()
    } else {
        let mut items: Vec<Element<'_, Message>> = vec![sub_tabs];
        if let Some(group_control) = group_control {
            items.push(group_control);
        }
        items.push(search_control);
        items.push(connection_view_control);
        items.push(close_filtered_btn.into());
        row(items)
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into()
    };

    // DUAL-13-11: idle-timeout control + manual sweep with honest last-sweep
    // status. The timeout choices and the idle reduction are the shared domain
    // seam; this surface only selects and renders them.
    let idle_choices = IDLE_TIMEOUT_CHOICES;
    let idle_timeout_labels: Vec<String> = idle_choices
        .iter()
        .map(|secs| idle_timeout_minutes_label(*secs))
        .collect();
    let idle_timeout_index = idle_choices
        .iter()
        .position(|secs| *secs == state.diag.connection_idle_timeout_secs)
        .unwrap_or(1);
    let idle_timeout_control = segmented_control(&idle_timeout_labels, idle_timeout_index, |idx| {
        let choices = IDLE_TIMEOUT_CHOICES;
        Message::SetConnectionIdleTimeout(choices[idx.min(choices.len() - 1)])
    });
    let idle_status_label = match state.diag.last_idle_sweep {
        Some(count) => {
            let count_text = count.to_string();
            interpolate(
                &lang.tr("conn_idle_last_sweep"),
                &[("count", count_text.as_str())],
            )
        }
        None => lang.tr("conn_idle_last_sweep_none").to_string(),
    };
    let idle_sweep_btn = button(
        row![
            svg_icons::icon_themed(Icon::Activity, 13.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(4),
            theme::role_body_text(lang.tr("conn_idle_sweep_btn").to_string()),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press_maybe(
        (!state.diag.connection_activity.is_empty()).then_some(Message::SweepIdleConnections),
    );

    let idle_bar = row![
        theme::role_body_text(lang.tr("conn_idle_timeout_label").to_string()).style(|t: &Theme| {
            text::Style {
                color: Some(tokens(t).text_tertiary),
            }
        }),
        Space::new().width(theme::SP_SM),
        idle_timeout_control,
        Space::new().width(theme::SP_MD),
        idle_sweep_btn,
        Space::new().width(theme::SP_MD),
        theme::role_body_text(idle_status_label).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut connections_section = column![
        section_header(
            lang.tr("runtime_connections_title").as_ref(),
            Some(header_trailing.into())
        ),
        Space::new().height(theme::SP_SM),
        traffic_summary,
        Space::new().height(theme::SP_SM),
        conn_sort_control,
        Space::new().height(theme::SP_SM),
        filter_bar,
        Space::new().height(theme::SP_SM),
        idle_bar,
        Space::new().height(theme::SP_MD),
    ];

    // The column chooser is its own band above the table: twenty-two
    // labelled columns need room the filter bar does not have, and a band
    // that scrolls sideways keeps the page's vertical budget for the table.
    if let Some(chooser) = connection_view_controls::column_chooser_controls(state) {
        connections_section = connections_section.push(chooser);
        connections_section = connections_section.push(Space::new().height(theme::SP_MD));
    }

    if is_closed_tab {
        let mut closed_rows = state
            .diag
            .closed_connections_model
            .visible_rows()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        state
            .diag
            .closed_connections_model
            .sort_rows(&mut closed_rows);
        if closed_rows.is_empty() {
            connections_section = connections_section.push(empty_state(
                Icon::Plug,
                lang.tr("conn_no_closed").as_ref(),
                lang.tr("conn_no_closed_desc").as_ref(),
            ));
        } else {
            connections_section = connections_section.push(
                button(theme::role_body_text(lang.tr("common_clear")))
                    .padding([7, 10])
                    .style(style_ghost)
                    .on_press(Message::ClearClosedConnections),
            );
            if table_mode {
                // The closed records carry fewer facts than a live row, so
                // the table shows the honest unknown where the owner
                // published nothing.
                let rated: Vec<RatedConnection<'_, _>> = closed_rows
                    .iter()
                    .map(|row| RatedConnection::new(row, ConnectionRate::default()))
                    .collect();
                let projection = project_connection_view_table(
                    &state.diag.connection_view.columns,
                    &rated,
                    ConnectionTableStatus::Closed,
                    ConnectionObservationInstant::unobserved(),
                );
                connections_section =
                    connections_section.push(connections_table::connection_table(
                        state,
                        &projection,
                        ConnectionTableStatus::Closed,
                        |id| state.diag.closed_connections_model.search_row(id),
                    ));
            } else {
                let mut closed_list = column![].spacing(theme::SP_SM);
                for row in closed_rows {
                    let traffic = format!(
                        "↑ {} · ↓ {}",
                        format_bytes(row.upload_bytes),
                        format_bytes(row.download_bytes)
                    );
                    closed_list = closed_list.push(
                        container(
                            row![
                                connections_table::connection_status_dot(
                                    ConnectionTableStatus::Closed
                                ),
                                Space::new().width(theme::SP_SM),
                                theme::role_body_text(row.destination).width(Length::Fill),
                                badge(row.outbound, BadgeKind::Neutral),
                                Space::new().width(theme::SP_SM),
                                theme::role_body_text(traffic),
                            ]
                            .align_y(Alignment::Center),
                        )
                        .padding([10, 16])
                        .style(row_card_surface),
                    );
                }
                connections_section =
                    connections_section.push(modern_scrollable(closed_list).height(Length::Fill));
            }
        }
    } else if state.diag.connections_model.source_current() {
        // The shared page model owns the query fold and the row order; this
        // surface only renders the rows it hands back.
        // DUAL-13-12: the shared sort runs with the derived instantaneous
        // rates attached, so the rate keys rank on real bytes-per-second.
        let rated_rows = state
            .diag
            .connections_model
            .ordered_rated_rows(&state.diag.connection_rate_book);

        if rated_rows.is_empty() {
            connections_section = connections_section.push(empty_state(
                Icon::Plug,
                lang.tr("runtime_no_matching_connections").as_ref(),
                "",
            ));
        } else if table_mode {
            // The table morphology renders one row per connection with the
            // user's own columns; the shared projection formats every cell,
            // so this surface only places the published columns.
            let projection = project_connection_view_table(
                &state.diag.connection_view.columns,
                &rated_rows,
                ConnectionTableStatus::Active,
                observed_instant(state),
            );
            connections_section = connections_section.push(connections_table::connection_table(
                state,
                &projection,
                ConnectionTableStatus::Active,
                |id| state.diag.connections_model.search_row(id),
            ));
        } else if !state.diag.connections_model.mode().is_flat() {
            // DUAL-13-02: both grouped dimensions reduce through the shared
            // domain aggregation; Iced owns no aggregation logic of its own.
            let mode = state.diag.connections_model.mode();
            let aggregates = state.diag.connections_model.rows();
            if aggregates.is_empty() {
                connections_section = connections_section.push(empty_state(
                    Icon::Plug,
                    lang.tr("conn_aggregate_empty").as_ref(),
                    "",
                ));
            } else {
                let (icon, icon_color): (Icon, fn(&Theme) -> Color) =
                    if mode == ConnectionGroupingMode::ByProcess {
                        (Icon::Activity, |t: &Theme| tokens(t).accent)
                    } else {
                        (Icon::Globe, |t: &Theme| tokens(t).success)
                    };
                let mut grouped_list = column![].spacing(theme::SP_SM);
                for aggregate in aggregates {
                    let count_text = aggregate.count.to_string();
                    let count_label = interpolate(
                        &lang.tr("conn_aggregate_count"),
                        &[("count", count_text.as_str())],
                    );
                    let group_card = container(
                        row![
                            svg_icons::icon_themed(icon, 16.0, icon_color),
                            Space::new().width(theme::SP_MD),
                            theme::role_body_text(aggregate.key.clone()).width(Length::Fill),
                            badge(count_label, BadgeKind::Neutral),
                            Space::new().width(theme::SP_MD),
                            theme::role_body_text(aggregate.traffic.clone()).style(|t: &Theme| {
                                text::Style {
                                    color: Some(tokens(t).text_secondary),
                                }
                            }),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([10, 16])
                    .style(row_card_surface);
                    grouped_list = grouped_list.push(group_card);
                }
                let grouped_scroll = modern_scrollable(
                    container(grouped_list).id(InteractionRegion::ConnectionGroups.id()),
                )
                .height(Length::Fill);
                let grouped_scroll = match scroll_owner {
                    ConnectionListScrollOwner::RuntimePage => grouped_scroll,
                    ConnectionListScrollOwner::ConnectionsPage => {
                        grouped_scroll.id(CONNECTIONS_PAGE_SCROLL_ID)
                    }
                };
                connections_section = connections_section.push(grouped_scroll);
            }
        } else {
            // Windowed rendering: only current window items are instantiated into widgets
            let total = rated_rows.len();
            let (page, start, end) = state.connections_window(total);
            let mut conn_list = column![].spacing(theme::SP_SM);

            for rated in &rated_rows[start..end] {
                let conn = rated.row;
                let rate = rated.rate;
                let pulse = connection_pulse_intensity(
                    conn,
                    &state.diag.connection_rate_book,
                    state.diag.connection_pulse_phase,
                );
                let process_name = extract_process_name(&conn.metadata.process_path);
                let host = if conn.metadata.host.is_empty() {
                    conn.metadata.destination_ip.clone()
                } else {
                    conn.metadata.host.clone()
                };

                let (target_node, target_badge_kind) = outbound_target_info(conn);
                let rule_str = format!("{}({})", conn.rule, conn.rule_payload);
                let payload_str = format!("{}:{}", host, conn.metadata.destination_port);
                let source_str = format!(
                    "{} → {}",
                    conn.metadata.source_ip, conn.metadata.source_port
                );
                let network = conn.metadata.network.to_uppercase();

                // Headline row: status dot, optional process chip, destination domain/IP,
                // protocol chip, outbound target badge, monospace traffic counts, kill button
                let mut headline_items: Vec<Element<'_, Message>> =
                    vec![status_dot(true), Space::new().width(theme::SP_SM).into()];

                if !process_name.is_empty() {
                    headline_items.push(
                        if let Some(search) = state.diag.connections_model.search_row(&conn.id) {
                            connection_search::connection_text(state, &search.process_name, 12)
                        } else {
                            chip(process_name)
                        },
                    );
                    headline_items.push(Space::new().width(theme::SP_SM).into());
                }

                let search = state.diag.connections_model.search_row(&conn.id);
                let host_text = if let Some(search) = search {
                    connection_search::connection_text(state, &search.endpoint, 13)
                } else {
                    theme::role_body_text(host).into()
                };
                headline_items.push(
                    container(host_text)
                        .id(connection_search::highlight_id(&conn.id))
                        .width(Length::Fill)
                        .into(),
                );

                headline_items.push(chip(network));
                headline_items.push(Space::new().width(theme::SP_SM).into());
                headline_items.push(badge(target_node, target_badge_kind));
                if pulse > 0.0 {
                    // DUAL-13-10: only a connection that really crossed the
                    // shared threshold renders the breathing glow.
                    headline_items.push(Space::new().width(theme::SP_SM).into());
                    headline_items.push(high_throughput_pulse(
                        lang.tr("conn_pulse_high_throughput").to_string(),
                        pulse,
                    ));
                }
                headline_items.push(Space::new().width(theme::SP_MD).into());
                headline_items.push(
                    theme::role_body_text(format!(
                        "↑ {} / ↓ {}",
                        format_bytes(conn.upload),
                        format_bytes(conn.download)
                    ))
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    })
                    .into(),
                );
                headline_items.push(Space::new().width(theme::SP_SM).into());
                headline_items.push(icon_button(
                    Icon::Activity,
                    14.0,
                    Message::InspectConnection(Some(conn.id.clone())),
                ));
                headline_items.push(Space::new().width(theme::SP_XS).into());
                headline_items.push(icon_button(
                    Icon::X,
                    14.0,
                    Message::CloseConnection(conn.id.clone()),
                ));

                let headline: Element<'_, Message> = if connection_rows_use_stacked_headline(state)
                {
                    column(headline_items).spacing(theme::SP_XS).into()
                } else {
                    row(headline_items).align_y(Alignment::Center).into()
                };
                let match_detail = search
                    .filter(|row| !row.matched_term.is_empty())
                    .map(|row| connection_search::connection_text(state, &row.matched_term, 11));

                // Subline row: rule payload, destination host:port, optional chains, source:port
                let mut subline_items: Vec<Element<'_, Message>> = vec![
                    theme::role_body_text(rule_str)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).accent),
                        })
                        .into(),
                    Space::new().width(theme::SP_MD).into(),
                    theme::role_body_text(payload_str)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        })
                        .into(),
                ];

                // DUAL-13-06: the route chain renders one element per parsed
                // hop through the shared model, not a pre-joined string.
                let route_chain = connection_view::route_chain(conn);
                if route_chain.len() > 1 {
                    subline_items.push(Space::new().width(theme::SP_MD).into());
                    for (hop_idx, hop) in route_chain.hops().iter().enumerate() {
                        if hop_idx > 0 {
                            subline_items.push(
                                theme::role_body_text(" → ")
                                    .style(|t: &Theme| text::Style {
                                        color: Some(tokens(t).text_tertiary),
                                    })
                                    .into(),
                            );
                        }
                        subline_items.push(
                            theme::role_body_text(hop.clone())
                                .style(|t: &Theme| text::Style {
                                    color: Some(tokens(t).text_tertiary),
                                })
                                .into(),
                        );
                    }
                }

                // DUAL-13-10/12: the shared derivation publishes real
                // instantaneous rates; the row shows them only once a window
                // exists (a fresh connection honestly shows nothing).
                if rate.peak_bps() > 0.0 {
                    subline_items.push(Space::new().width(theme::SP_MD).into());
                    subline_items.push(
                        theme::role_body_text(format!(
                            "⚡ ↑ {} ↓ {}",
                            format_rate(rate.upload_bps),
                            format_rate(rate.download_bps)
                        ))
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).success),
                        })
                        .into(),
                    );
                }

                subline_items.push(Space::new().width(Length::Fill).into());
                subline_items.push(
                    theme::role_body_text(source_str)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        })
                        .into(),
                );

                let subline = row(subline_items).align_y(Alignment::Center);

                let mut row_content = column![headline, Space::new().height(theme::SP_XS), subline];
                if let Some(detail) = match_detail {
                    row_content = row_content.push(detail);
                }
                let row_content = row_content.spacing(theme::SP_XS);

                conn_list = conn_list.push(
                    container(row_content)
                        .id(connection_search::row_id(&conn.id))
                        .padding(SP_MD)
                        .width(Length::Fill)
                        .style(move |t: &Theme| {
                            // DUAL-13-10: the breathing glow is the shared
                            // intensity tinted into the card background.
                            let mut card = row_card_surface(t);
                            if pulse > 0.0 {
                                card.background = Some(
                                    Color {
                                        a: pulse * 0.35,
                                        ..tokens(t).accent
                                    }
                                    .into(),
                                );
                            }
                            card
                        }),
                );
            }

            let connection_list: Element<'_, Message> = match scroll_owner {
                ConnectionListScrollOwner::RuntimePage => conn_list.into(),
                ConnectionListScrollOwner::ConnectionsPage => modern_scrollable(conn_list)
                    .id(CONNECTIONS_PAGE_SCROLL_ID)
                    .height(Length::Fill)
                    .into(),
            };
            connections_section = connections_section.push(connection_list);

            if total > state.diag.connections_page_size {
                connections_section = connections_section.push(
                    row![
                        text_btn(
                            "‹".to_string(),
                            style_ghost,
                            (page > 0).then_some(Message::ConnectionsPrevPage),
                        ),
                        Space::new().width(theme::SP_SM),
                        theme::role_body_text(format!("{}–{} / {}", start + 1, end, total)).style(
                            |t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary),
                            }
                        ),
                        Space::new().width(theme::SP_SM),
                        text_btn(
                            "›".to_string(),
                            style_ghost,
                            (end < total).then_some(Message::ConnectionsNextPage),
                        ),
                    ]
                    .align_y(Alignment::Center),
                );
            }
        }
    } else {
        connections_section = connections_section.push(empty_state(
            Icon::Plug,
            lang.tr("runtime_no_connections").as_ref(),
            "",
        ));
    }

    match scroll_owner {
        ConnectionListScrollOwner::RuntimePage => connections_section.into(),
        ConnectionListScrollOwner::ConnectionsPage => {
            connections_section.height(Length::Fill).into()
        }
    }
}

/// DUAL-13-10: the pulsing chip of a connection above the shared
/// high-throughput threshold. The glow intensity is the shared breathing
/// function, so this surface and Bevy pulse identically.
fn high_throughput_pulse<'a>(label: String, intensity: f32) -> Element<'a, Message> {
    container(
        row![
            container(Space::new().width(8).height(8)).style(move |t: &Theme| {
                container::Style {
                    background: Some(
                        Color {
                            a: intensity,
                            ..tokens(t).accent
                        }
                        .into(),
                    ),
                    border: Border {
                        radius: theme::R_XS.into(),
                        width: theme::HAIRLINE,
                        color: Color {
                            a: intensity,
                            ..tokens(t).accent
                        },
                    },
                    ..Default::default()
                }
            }),
            Space::new().width(theme::SP_XS),
            theme::role_body_text(label),
        ]
        .align_y(Alignment::Center),
    )
    .padding([2, 8])
    .style(move |t: &Theme| container::Style {
        background: Some(
            Color {
                a: intensity * 0.22,
                ..tokens(t).accent
            }
            .into(),
        ),
        border: Border {
            radius: theme::R_PILL.into(),
            width: theme::HAIRLINE,
            color: Color {
                a: intensity,
                ..tokens(t).accent
            },
        },
        text_color: Some(tokens(t).accent),
        ..Default::default()
    })
    .into()
}

fn stream_badge<'a>(state: &RuntimeStreamState, lang: &Lang<'_>) -> Element<'a, Message> {
    use infiltrator_contract::connection::ConnectionStreamPhase;
    let (key, kind) = match state.shared_phase() {
        ConnectionStreamPhase::Idle => ("conn_state_disconnected", BadgeKind::Neutral),
        ConnectionStreamPhase::Connecting => ("conn_state_connecting", BadgeKind::Neutral),
        ConnectionStreamPhase::Live => ("conn_state_live", BadgeKind::Success),
        ConnectionStreamPhase::Reconnecting => ("conn_state_reconnecting", BadgeKind::Warning),
        ConnectionStreamPhase::Unavailable => ("conn_state_unavailable", BadgeKind::Danger),
    };
    badge(lang.tr(key).to_string(), kind)
}

#[cfg(test)]
#[path = "../../../tests/gui/view_runtime_connections_tests.rs"]
mod tests;
