//! Runtime page connections section: filter/sort controls plus the filtered,
//! sorted connection row list.

use infiltrator_shared::locales::{Lang, Localizer};
use crate::utils::format_bytes;
use crate::view::components::{
    chip, empty_state, icon_button, section_header, segmented_control, status_dot,
};
use crate::view::runtime::styles::{input_style, row_card, style_danger, text_btn};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};

pub(super) fn connections_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let sort_labels: Vec<String> = vec![
        lang.tr("runtime_conn_sort_download_desc").to_string(),
        lang.tr("runtime_conn_sort_upload_desc").to_string(),
        lang.tr("runtime_conn_sort_latest_desc").to_string(),
        lang.tr("runtime_conn_sort_host_asc").to_string(),
    ];
    let sort_index = match state.runtime.runtime_connection_sort.as_str() {
        "upload_desc" => 1,
        "latest_desc" => 2,
        "host_asc" => 3,
        _ => 0,
    };
    let conn_sort_control = segmented_control(&sort_labels, sort_index, |index| {
        let key = match index {
            1 => "upload_desc",
            2 => "latest_desc",
            3 => "host_asc",
            _ => "download_desc",
        };
        Message::UpdateRuntimeConnectionSort(key.to_string())
    });

    let filter_row = row![
        svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_SM),
        text_input(
            lang.tr("runtime_conn_filter_placeholder").as_ref(),
            &state.runtime.runtime_connection_filter
        )
        .on_input(Message::UpdateRuntimeConnectionFilter)
        .padding([8, 12])
        .size(12)
        .font(MONO)
        .width(Length::Fixed(260.0))
        .style(input_style),
        Space::new().width(theme::SP_SM),
        if state.runtime.runtime_connection_filter.is_empty() {
            Space::new().width(0).into()
        } else {
            icon_button(
                Icon::X,
                14.0,
                Message::UpdateRuntimeConnectionFilter(String::new()),
            )
        },
        Space::new().width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let mut connections_section = column![
        section_header(
            lang.tr("runtime_connections_title").as_ref(),
            Some(
                row![
                    stream_badge(&state.diag.connections_stream_state),
                    Space::new().width(theme::SP_SM),
                    conn_sort_control,
                    Space::new().width(theme::SP_SM),
                    icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        lang.tr("btn_close_all").to_string(),
                        style_danger,
                        Some(Message::RequestConfirmation(
                            ConfirmAction::CloseAllConnections,
                        ))
                    ),
                ]
                .align_y(Alignment::Center)
                .into(),
            ),
        ),
        Space::new().height(theme::SP_MD),
        filter_row,
        Space::new().height(theme::SP_SM),
    ];

    if let Some(c) = &state.diag.connections {
        let mut conn_list = column![].spacing(theme::SP_SM);

        let mut sorted_conns = c.connections.clone();
        let connection_filter = state
            .runtime
            .runtime_connection_filter
            .trim()
            .to_lowercase();
        if !connection_filter.is_empty() {
            sorted_conns.retain(|conn| {
                let metadata = &conn.metadata;
                let id = conn.id.to_lowercase();
                let host = metadata.host.to_lowercase();
                let process = metadata.process_path.to_lowercase();
                let source = metadata.source_ip.to_lowercase();
                let destination = metadata.destination_ip.to_lowercase();
                let rule = conn.rule.to_lowercase();
                id.contains(&connection_filter)
                    || host.contains(&connection_filter)
                    || process.contains(&connection_filter)
                    || source.contains(&connection_filter)
                    || destination.contains(&connection_filter)
                    || rule.contains(&connection_filter)
            });
        }
        sorted_conns.sort_by(|a, b| {
            let ordering = match state.runtime.runtime_connection_sort.as_str() {
                "upload_desc" => b.upload.cmp(&a.upload),
                "latest_desc" => b.start.cmp(&a.start),
                "host_asc" => {
                    let left_host = if a.metadata.host.is_empty() {
                        a.metadata.destination_ip.as_str()
                    } else {
                        a.metadata.host.as_str()
                    };
                    let right_host = if b.metadata.host.is_empty() {
                        b.metadata.destination_ip.as_str()
                    } else {
                        b.metadata.host.as_str()
                    };
                    left_host.cmp(right_host)
                }
                _ => b.download.cmp(&a.download),
            };
            if ordering == std::cmp::Ordering::Equal {
                a.id.cmp(&b.id)
            } else {
                ordering
            }
        });

        if sorted_conns.is_empty() {
            connections_section = connections_section.push(empty_state(
                Icon::Plug,
                lang.tr("runtime_no_matching_connections").as_ref(),
                "",
            ));
        }

        for conn in sorted_conns {
            let host = if conn.metadata.host.is_empty() {
                conn.metadata.destination_ip.clone()
            } else {
                conn.metadata.host.clone()
            };

            let rule_str = format!("{}({})", conn.rule, conn.rule_payload);
            let payload_str = format!("{}:{}", host, conn.metadata.destination_port);
            let source_str = format!(
                "{} → {}",
                conn.metadata.source_ip, conn.metadata.source_port
            );
            let network = conn.metadata.network.to_uppercase();

            let row_content = column![
                row![
                    status_dot(true),
                    Space::new().width(theme::SP_SM),
                    chip(network),
                    Space::new().width(theme::SP_SM),
                    text(host)
                        .size(13)
                        .font(FONT_SEMIBOLD)
                        .width(Length::Fill)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        }),
                    text(format!(
                        "↑ {} / ↓ {}",
                        format_bytes(conn.upload),
                        format_bytes(conn.download)
                    ))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                    Space::new().width(theme::SP_MD),
                    icon_button(Icon::X, 14.0, Message::CloseConnection(conn.id.clone())),
                ]
                .align_y(Alignment::Center),
                Space::new().height(theme::SP_XS),
                row![
                    text(rule_str).size(11).style(|t: &Theme| text::Style {
                        color: Some(tokens(t).accent),
                    }),
                    Space::new().width(theme::SP_MD),
                    text(payload_str)
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                    Space::new().width(Length::Fill),
                    text(source_str)
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                ]
                .align_y(Alignment::Center),
            ]
            .spacing(theme::SP_XS);

            conn_list = conn_list.push(
                container(row_content)
                    .padding(SP_MD)
                    .width(Length::Fill)
                    .style(row_card),
            );
        }

        // Natural height: the page-level scrollable owns scrolling, so every
        // connection row renders fully instead of collapsing inside an
        // auto-height card.
        connections_section = connections_section.push(conn_list);
    } else {
        connections_section = connections_section.push(empty_state(
            Icon::Plug,
            lang.tr("runtime_no_connections").as_ref(),
            "",
        ));
    }

    connections_section.into()
}

fn stream_badge(state: &RuntimeStreamState) -> Element<'static, Message> {
    let (label, kind) = match state {
        RuntimeStreamState::Idle => ("未连接", crate::view::components::BadgeKind::Neutral),
        RuntimeStreamState::Connecting => {
            ("连接中", crate::view::components::BadgeKind::Neutral)
        }
        RuntimeStreamState::Connected => ("实时", crate::view::components::BadgeKind::Success),
        RuntimeStreamState::Reconnecting => {
            ("重连中", crate::view::components::BadgeKind::Warning)
        }
        RuntimeStreamState::Failed(_) => {
            ("不可用", crate::view::components::BadgeKind::Danger)
        }
    };
    crate::view::components::badge(label, kind)
}
