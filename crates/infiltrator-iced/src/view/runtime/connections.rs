//! Runtime page connections section: header with traffic totals, sort
//! segmented control, live search filter, close-all button, sub-tabs
//! (Active / Closed), and windowed connection card rows with process names,
//! protocol chips, outbound target badges, instantaneous rates and the
//! high-throughput pulse, plus kill buttons.

use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::utils::format_bytes;
use crate::view::components::{
    BadgeKind, badge, chip, empty_state, icon_button, modern_scrollable, row_card_surface,
    search_input, section_header, segmented_control, status_dot, style_danger, style_ghost,
    text_btn,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};
use infiltrator_domain::connection_rate::{self, ConnectionRates};
use infiltrator_domain::connection_view::{self, ConnectionGroupingMode, ConnectionSortKey};
use infiltrator_domain::runtime::Connection;
use infiltrator_shared::locales::{Lang, Localizer};

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

/// Filter connections matching query across ID, host, process, IP, and rule.
pub fn filter_connection(conn: &Connection, query: &str) -> bool {
    connection_view::matches_search(conn, query)
}

/// DUAL-13-12: order the live rows through the one shared sort reduction with
/// the derived instantaneous rates attached, so the rate keys rank on real
/// bytes-per-second and the cumulative keys keep their previous behavior.
pub fn sort_rated_connections<'a>(
    conns: &'a [Connection],
    rates: &ConnectionRates,
    sort_key: &str,
) -> Vec<connection_view::RatedConnection<'a, Connection>> {
    let mut rows: Vec<connection_view::RatedConnection<'_, Connection>> = conns
        .iter()
        .map(|conn| connection_view::RatedConnection::new(conn, rates.get(&conn.id)))
        .collect();
    connection_view::sort_connections(&mut rows, ConnectionSortKey::from_identifier(sort_key));
    rows
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

pub(super) fn connections_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let _is_zh = state.shell.lang.starts_with("zh");

    // 1. Sort segmented control: cumulative download / cumulative upload /
    // instantaneous download / instantaneous upload / latest / host. The
    // instantaneous entries are DUAL-13-12; every index maps to a shared
    // `ConnectionSortKey`, and the wire value is its identifier.
    let sort_labels: Vec<String> = vec![
        lang.tr("runtime_conn_sort_download_desc").to_string(),
        lang.tr("runtime_conn_sort_upload_desc").to_string(),
        lang.tr("runtime_conn_sort_download_rate").to_string(),
        lang.tr("runtime_conn_sort_upload_rate").to_string(),
        lang.tr("runtime_conn_sort_latest_desc").to_string(),
        lang.tr("runtime_conn_sort_host_asc").to_string(),
    ];
    let sort_index =
        match ConnectionSortKey::from_identifier(&state.runtime.runtime_connection_sort) {
            ConnectionSortKey::DownloadDesc => 0,
            ConnectionSortKey::UploadDesc => 1,
            ConnectionSortKey::DownloadRateDesc => 2,
            ConnectionSortKey::UploadRateDesc => 3,
            ConnectionSortKey::LatestDesc => 4,
            ConnectionSortKey::HostAsc => 5,
        };
    let conn_sort_control = segmented_control(&sort_labels, sort_index, |index| {
        let key = match index {
            1 => ConnectionSortKey::UploadDesc,
            2 => ConnectionSortKey::DownloadRateDesc,
            3 => ConnectionSortKey::UploadRateDesc,
            4 => ConnectionSortKey::LatestDesc,
            5 => ConnectionSortKey::HostAsc,
            _ => ConnectionSortKey::DownloadDesc,
        };
        Message::UpdateRuntimeConnectionSort(key.as_str().to_string())
    });

    // 2. Traffic totals and stream status
    let (upload_total, download_total) = match &state.diag.connections {
        Some(c) => (c.upload_total, c.download_total),
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
            text(lang.tr("btn_close_all").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_danger)
    .on_press(Message::RequestConfirmation(
        ConfirmAction::CloseAllConnections,
    ));

    // 4. Header toolbar
    let header_trailing = row![
        stream_badge(&state.diag.connections_stream_state, &lang),
        Space::new().width(theme::SP_SM),
        upload_badge,
        Space::new().width(theme::SP_XS),
        download_badge,
        Space::new().width(theme::SP_MD),
        conn_sort_control,
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
        Space::new().width(theme::SP_SM),
        close_all_btn,
    ]
    .align_y(Alignment::Center);

    // 5. Sub-tabs: 活动中 (Active) and 已关闭 (Closed)
    let (is_closed_tab, user_filter) = if let Some(stripped) = state
        .runtime
        .runtime_connection_filter
        .strip_prefix("tab:closed")
    {
        (true, stripped.trim())
    } else {
        (false, state.runtime.runtime_connection_filter.trim())
    };

    let total_active_conns = state
        .diag
        .connections
        .as_ref()
        .map(|c| c.connections.len())
        .unwrap_or(0);

    let tab_labels = vec![
        format!("{} ({total_active_conns})", lang.tr("conn_status_active")),
        format!("{} (0)", lang.tr("conn_status_closed")),
    ];
    let tab_index = if is_closed_tab { 1 } else { 0 };
    let sub_tabs = segmented_control(&tab_labels, tab_index, move |idx| {
        if idx == 1 {
            let q = if user_filter.is_empty() {
                "tab:closed".to_string()
            } else {
                format!("tab:closed {user_filter}")
            };
            Message::UpdateRuntimeConnectionFilter(q)
        } else {
            Message::UpdateRuntimeConnectionFilter(user_filter.to_string())
        }
    });

    // 6. Live search filter
    let on_search_input = move |query: String| {
        if is_closed_tab {
            if query.trim().is_empty() {
                Message::UpdateRuntimeConnectionFilter("tab:closed".to_string())
            } else {
                Message::UpdateRuntimeConnectionFilter(format!("tab:closed {query}"))
            }
        } else {
            Message::UpdateRuntimeConnectionFilter(query)
        }
    };
    let on_search_clear = if is_closed_tab {
        Message::UpdateRuntimeConnectionFilter("tab:closed".to_string())
    } else {
        Message::UpdateRuntimeConnectionFilter(String::new())
    };

    let group_labels = vec![
        lang.tr("conn_group_flat").to_string(),
        lang.tr("conn_group_process").to_string(),
        lang.tr("conn_group_host").to_string(),
    ];
    let group_idx = match state.diag.connection_grouping_mode {
        ConnectionGroupingMode::Flat => 0,
        ConnectionGroupingMode::ByProcess => 1,
        ConnectionGroupingMode::ByHost => 2,
    };
    let group_control = segmented_control(&group_labels, group_idx, |idx| {
        Message::SetConnectionGroupingMode(match idx {
            1 => ConnectionGroupingMode::ByProcess,
            2 => ConnectionGroupingMode::ByHost,
            _ => ConnectionGroupingMode::Flat,
        })
    });

    let close_filtered_btn = button(
        row![
            svg_icons::icon_themed(Icon::X, 13.0, |t: &Theme| tokens(t).warning),
            Space::new().width(4),
            text(lang.tr("conn_close_filtered_btn").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press_maybe((!user_filter.is_empty()).then_some(Message::CloseFilteredConnections));

    let filter_bar = row![
        sub_tabs,
        Space::new().width(theme::SP_MD),
        group_control,
        Space::new().width(theme::SP_LG),
        search_input(
            lang.tr("runtime_conn_filter_placeholder").as_ref(),
            user_filter,
            on_search_input,
            on_search_clear,
        ),
        Space::new().width(theme::SP_SM),
        close_filtered_btn,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    // DUAL-13-11: idle-timeout control + manual sweep with honest last-sweep
    // status. The timeout choices and the idle reduction are the shared domain
    // seam; this surface only selects and renders them.
    let idle_choices = infiltrator_domain::connection_activity::IDLE_TIMEOUT_CHOICES;
    let idle_timeout_labels: Vec<String> = idle_choices
        .iter()
        .map(|secs| infiltrator_domain::connection_activity::idle_timeout_minutes_label(*secs))
        .collect();
    let idle_timeout_index = idle_choices
        .iter()
        .position(|secs| *secs == state.diag.connection_idle_timeout_secs)
        .unwrap_or(1);
    let idle_timeout_control = segmented_control(&idle_timeout_labels, idle_timeout_index, |idx| {
        let choices = infiltrator_domain::connection_activity::IDLE_TIMEOUT_CHOICES;
        Message::SetConnectionIdleTimeout(choices[idx.min(choices.len() - 1)])
    });
    let idle_status_label = match state.diag.last_idle_sweep {
        Some(count) => {
            let count_text = count.to_string();
            infiltrator_shared::i18n_interpolator::interpolate(
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
            text(lang.tr("conn_idle_sweep_btn").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press_maybe(
        (!state.diag.connection_activity.is_empty()).then_some(Message::SweepIdleConnections),
    );

    let idle_bar = row![
        text(lang.tr("conn_idle_timeout_label").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        Space::new().width(theme::SP_SM),
        idle_timeout_control,
        Space::new().width(theme::SP_MD),
        idle_sweep_btn,
        Space::new().width(theme::SP_MD),
        text(idle_status_label)
            .size(11)
            .style(|t: &Theme| text::Style {
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
        Space::new().height(theme::SP_MD),
        filter_bar,
        Space::new().height(theme::SP_SM),
        idle_bar,
        Space::new().height(theme::SP_MD),
    ];

    if is_closed_tab {
        // Sub-tab: 已关闭 (Closed)
        connections_section = connections_section.push(empty_state(
            Icon::Plug,
            lang.tr("conn_no_closed").as_ref(),
            lang.tr("conn_no_closed_desc").as_ref(),
        ));
    } else if let Some(c) = &state.diag.connections {
        let mut filtered_conns = c.connections.clone();
        if !user_filter.is_empty() {
            filtered_conns.retain(|conn| filter_connection(conn, user_filter));
        }
        // DUAL-13-12: the shared sort runs with the derived instantaneous
        // rates attached, so the rate keys rank on real bytes-per-second.
        let rated_rows = sort_rated_connections(
            &filtered_conns,
            &state.diag.connection_rate_book,
            &state.runtime.runtime_connection_sort,
        );

        if rated_rows.is_empty() {
            connections_section = connections_section.push(empty_state(
                Icon::Plug,
                lang.tr("runtime_no_matching_connections").as_ref(),
                "",
            ));
        } else if !state.diag.connection_grouping_mode.is_flat() {
            // DUAL-13-02: both grouped dimensions reduce through the shared
            // domain aggregation; Iced owns no aggregation logic of its own.
            let mode = state.diag.connection_grouping_mode;
            let aggregates = connection_view::aggregate_connections(&filtered_conns, mode);
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
                for aggregate in &aggregates {
                    let count_text = aggregate.count.to_string();
                    let count_label = infiltrator_shared::i18n_interpolator::interpolate(
                        &lang.tr("conn_aggregate_count"),
                        &[("count", count_text.as_str())],
                    );
                    let group_card = container(
                        row![
                            svg_icons::icon_themed(icon, 16.0, icon_color),
                            Space::new().width(theme::SP_MD),
                            text(aggregate.key.clone())
                                .size(13)
                                .font(FONT_SEMIBOLD)
                                .width(Length::Fill),
                            badge(count_label, BadgeKind::Neutral),
                            Space::new().width(theme::SP_MD),
                            text(format!(
                                "↑ {} / ↓ {}",
                                format_bytes(aggregate.upload_total),
                                format_bytes(aggregate.download_total)
                            ))
                            .size(11)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary)
                            }),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([10, 16])
                    .style(row_card_surface);
                    grouped_list = grouped_list.push(group_card);
                }
                connections_section =
                    connections_section.push(modern_scrollable(grouped_list).height(Length::Fill));
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
                    headline_items.push(chip(process_name));
                    headline_items.push(Space::new().width(theme::SP_SM).into());
                }

                headline_items.push(
                    text(host)
                        .size(13)
                        .font(FONT_SEMIBOLD)
                        .width(Length::Fill)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        })
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
                    text(format!(
                        "↑ {} / ↓ {}",
                        format_bytes(conn.upload),
                        format_bytes(conn.download)
                    ))
                    .size(11)
                    .font(MONO)
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

                let headline = row(headline_items).align_y(Alignment::Center);

                // Subline row: rule payload, destination host:port, optional chains, source:port
                let mut subline_items: Vec<Element<'_, Message>> = vec![
                    text(rule_str)
                        .size(11)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).accent),
                        })
                        .into(),
                    Space::new().width(theme::SP_MD).into(),
                    text(payload_str)
                        .size(11)
                        .font(MONO)
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
                                text(" → ")
                                    .size(11)
                                    .font(MONO)
                                    .style(|t: &Theme| text::Style {
                                        color: Some(tokens(t).text_tertiary),
                                    })
                                    .into(),
                            );
                        }
                        subline_items.push(
                            text(hop.clone())
                                .size(11)
                                .font(MONO)
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
                        text(format!(
                            "⚡ ↑ {} ↓ {}",
                            format_rate(rate.upload_bps),
                            format_rate(rate.download_bps)
                        ))
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).success),
                        })
                        .into(),
                    );
                }

                subline_items.push(Space::new().width(Length::Fill).into());
                subline_items.push(
                    text(source_str)
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        })
                        .into(),
                );

                let subline = row(subline_items).align_y(Alignment::Center);

                let row_content = column![headline, Space::new().height(theme::SP_XS), subline]
                    .spacing(theme::SP_XS);

                conn_list = conn_list.push(
                    container(row_content)
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

            connections_section = connections_section.push(conn_list);

            if total > state.diag.connections_page_size {
                connections_section = connections_section.push(
                    row![
                        text_btn(
                            "‹".to_string(),
                            style_ghost,
                            (page > 0).then_some(Message::ConnectionsPrevPage),
                        ),
                        Space::new().width(theme::SP_SM),
                        text(format!("{}–{} / {}", start + 1, end, total))
                            .size(11)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary),
                            }),
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

    connections_section.into()
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
                        radius: 4.0.into(),
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
            text(label).size(11).font(FONT_MEDIUM),
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
            radius: 999.0.into(),
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
