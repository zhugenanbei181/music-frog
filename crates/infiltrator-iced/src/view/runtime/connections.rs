//! Runtime page connections section: header with traffic totals, sort
//! segmented control, live search filter, close-all button, sub-tabs
//! (Active / Closed), and windowed connection card rows with process names,
//! protocol chips, outbound target badges, and kill buttons.

use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::runtime::{ConnectionGroupingMode, RuntimeStreamState};
use crate::utils::format_bytes;
use crate::view::components::{
    modern_scrollable,
    BadgeKind, badge, chip, empty_state, icon_button, row_card_surface,
    search_input, section_header, segmented_control, status_dot, style_danger, style_ghost,
    text_btn,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, SP_MD, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};
use infiltrator_domain::runtime::Connection;

/// Extract clean executable/binary name from a system process path.
pub fn extract_process_name(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let filename = trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed);
    let name = filename.strip_suffix(".exe").unwrap_or(filename);
    name.to_string()
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
    let kind = if target_upper == "DIRECT" || target.contains("\u{76f4}\u{8fde}") || target_upper == "DIRECT" {
        BadgeKind::Success
    } else if target_upper == "REJECT" || target.contains("\u{62d2}\u{7edd}") || target_upper == "REJECT" {
        BadgeKind::Danger
    } else {
        BadgeKind::Accent
    };

    (target, kind)
}

/// Filter connections matching query across ID, host, process, IP, and rule.
pub fn filter_connection(conn: &Connection, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query_lower = query.to_lowercase();
    let meta = &conn.metadata;
    conn.id.to_lowercase().contains(&query_lower)
        || meta.host.to_lowercase().contains(&query_lower)
        || meta.process_path.to_lowercase().contains(&query_lower)
        || meta.source_ip.to_lowercase().contains(&query_lower)
        || meta.destination_ip.to_lowercase().contains(&query_lower)
        || meta.source_port.contains(&query_lower)
        || meta.destination_port.contains(&query_lower)
        || meta.network.to_lowercase().contains(&query_lower)
        || conn.rule.to_lowercase().contains(&query_lower)
        || conn.rule_payload.to_lowercase().contains(&query_lower)
        || conn.chains.iter().any(|c| c.to_lowercase().contains(&query_lower))
}

/// Sort connection list according to user-selected sort key.
pub fn sort_connections(conns: &mut [Connection], sort_key: &str) {
    conns.sort_by(|a, b| {
        let ordering = match sort_key {
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
}

pub(super) fn connections_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let _is_zh = state.shell.lang.starts_with("zh");

    // 1. Sort segmented control: download / upload / latest / host
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

    // 2. Traffic totals and stream status
    let (upload_total, download_total) = match &state.diag.connections {
        Some(c) => (c.upload_total, c.download_total),
        None => (0, 0),
    };
    let upload_badge = badge(format!("↑ {}", format_bytes(upload_total)), BadgeKind::Success);
    let download_badge = badge(format!("↓ {}", format_bytes(download_total)), BadgeKind::Accent);

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
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut connections_section = column![
        section_header(lang.tr("runtime_connections_title").as_ref(), Some(header_trailing.into())),
        Space::new().height(theme::SP_MD),
        filter_bar,
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
        let mut sorted_conns = c.connections.clone();
        if !user_filter.is_empty() {
            sorted_conns.retain(|conn| filter_connection(conn, user_filter));
        }
        sort_connections(&mut sorted_conns, &state.runtime.runtime_connection_sort);

        if sorted_conns.is_empty() {
            connections_section = connections_section.push(empty_state(
                Icon::Plug,
                lang.tr("runtime_no_matching_connections").as_ref(),
                "",
            ));
        } else if state.diag.connection_grouping_mode == ConnectionGroupingMode::ByProcess {
            let mut proc_map: std::collections::HashMap<String, (usize, u64, u64)> = std::collections::HashMap::new();
            for conn in &sorted_conns {
                let name = extract_process_name(&conn.metadata.process_path);
                let entry = proc_map.entry(if name.is_empty() { "unknown".to_string() } else { name }).or_insert((0, 0, 0));
                entry.0 += 1;
                entry.1 += conn.upload;
                entry.2 += conn.download;
            }
            let mut grouped_list = column![].spacing(theme::SP_SM);
            let mut sorted_procs: Vec<_> = proc_map.into_iter().collect();
            sorted_procs.sort_by_key(|item| std::cmp::Reverse(item.1 .1));
            for (proc_name, (cnt, up, down)) in sorted_procs {
                let proc_card = container(
                    row![
                        svg_icons::icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).accent),
                        Space::new().width(theme::SP_MD),
                        text(proc_name).size(13).font(FONT_SEMIBOLD).width(Length::Fill),
                        badge(format!("{cnt} connections"), BadgeKind::Neutral),
                        Space::new().width(theme::SP_MD),
                        text(format!("↑ {} / ↓ {}", format_bytes(up), format_bytes(down))).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                    ].align_y(Alignment::Center)
                ).padding([10, 16]).style(row_card_surface);
                grouped_list = grouped_list.push(proc_card);
            }
            connections_section = connections_section.push(modern_scrollable(grouped_list).height(Length::Fill));
        } else if state.diag.connection_grouping_mode == ConnectionGroupingMode::ByHost {
            let mut host_map: std::collections::HashMap<String, (usize, u64, u64)> = std::collections::HashMap::new();
            for conn in &sorted_conns {
                let h = if !conn.metadata.host.is_empty() { conn.metadata.host.clone() } else { conn.metadata.destination_ip.clone() };
                let entry = host_map.entry(h).or_insert((0, 0, 0));
                entry.0 += 1;
                entry.1 += conn.upload;
                entry.2 += conn.download;
            }
            let mut grouped_list = column![].spacing(theme::SP_SM);
            let mut sorted_hosts: Vec<_> = host_map.into_iter().collect();
            sorted_hosts.sort_by_key(|item| std::cmp::Reverse(item.1 .1));
            for (h_name, (cnt, up, down)) in sorted_hosts {
                let host_card = container(
                    row![
                        svg_icons::icon_themed(Icon::Globe, 16.0, |t: &Theme| tokens(t).success),
                        Space::new().width(theme::SP_MD),
                        text(h_name).size(13).font(MONO).width(Length::Fill),
                        badge(format!("{cnt} conns"), BadgeKind::Neutral),
                        Space::new().width(theme::SP_MD),
                        text(format!("↑ {} / ↓ {}", format_bytes(up), format_bytes(down))).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                    ].align_y(Alignment::Center)
                ).padding([10, 16]).style(row_card_surface);
                grouped_list = grouped_list.push(host_card);
            }
            connections_section = connections_section.push(modern_scrollable(grouped_list).height(Length::Fill));
        } else {
            // Windowed rendering: only current window items are instantiated into widgets
            let total = sorted_conns.len();
            let (page, start, end) = state.connections_window(total);
            let mut conn_list = column![].spacing(theme::SP_SM);

            for conn in &sorted_conns[start..end] {
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
                let mut headline_items: Vec<Element<'_, Message>> = vec![
                    status_dot(true),
                    Space::new().width(theme::SP_SM).into(),
                ];

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

                if conn.chains.len() > 1 {
                    subline_items.push(Space::new().width(theme::SP_MD).into());
                    subline_items.push(
                        text(conn.chains.join(" → "))
                            .size(11)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_tertiary),
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
                        .style(row_card_surface),
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

fn stream_badge<'a>(state: &RuntimeStreamState, lang: &Lang<'_>) -> Element<'a, Message> {
    let (key, kind) = match state {
        RuntimeStreamState::Idle => ("conn_state_disconnected", BadgeKind::Neutral),
        RuntimeStreamState::Connecting => ("conn_state_connecting", BadgeKind::Neutral),
        RuntimeStreamState::Connected => ("conn_state_live", BadgeKind::Success),
        RuntimeStreamState::Reconnecting => ("conn_state_reconnecting", BadgeKind::Warning),
        RuntimeStreamState::Failed(_) => ("conn_state_unavailable", BadgeKind::Danger),
    };
    badge(lang.tr(key).to_string(), kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_domain::runtime::{Connection, ConnectionMetadata};

    fn make_test_conn(id: &str, host: &str, process: &str, up: u64, down: u64) -> Connection {
        Connection {
            id: id.to_string(),
            metadata: ConnectionMetadata {
                network: "tcp".to_string(),
                connection_type: "TLS".to_string(),
                source_ip: "192.168.1.50".to_string(),
                destination_ip: "1.1.1.1".to_string(),
                source_port: "50000".to_string(),
                destination_port: "443".to_string(),
                host: host.to_string(),
                dns_mode: "fake-ip".to_string(),
                process_path: process.to_string(),
                special_proxy: String::new(),
            },
            upload: up,
            download: down,
            start: "2026-09-01T12:00:00Z".to_string(),
            rule: "DomainSuffix".to_string(),
            rule_payload: "example.com".to_string(),
            chains: vec!["DMIT".to_string(), "PROXY".to_string()],
        }
    }

    #[test]
    fn test_extract_process_name() {
        assert_eq!(extract_process_name("/usr/bin/firefox"), "firefox");
        assert_eq!(extract_process_name("C:\\Program Files\\Zed\\zed-editor.exe"), "zed-editor");
        assert_eq!(extract_process_name("zed-editor"), "zed-editor");
        assert_eq!(extract_process_name(""), "");
        assert_eq!(extract_process_name("   "), "");
    }

    #[test]
    fn test_outbound_target_info() {
        let mut conn = make_test_conn("1", "google.com", "", 100, 200);
        let (target, kind) = outbound_target_info(&conn);
        assert_eq!(target, "DMIT");
        assert_eq!(kind, BadgeKind::Accent);

        conn.chains = vec!["DIRECT".to_string()];
        let (target, kind) = outbound_target_info(&conn);
        assert_eq!(target, "DIRECT");
        assert_eq!(kind, BadgeKind::Success);

        conn.chains = vec!["REJECT".to_string()];
        let (target, kind) = outbound_target_info(&conn);
        assert_eq!(target, "REJECT");
        assert_eq!(kind, BadgeKind::Danger);
    }

    #[test]
    fn test_filter_connection() {
        let conn = make_test_conn("c1", "api.openai.com", "/usr/bin/chromium", 100, 200);
        assert!(filter_connection(&conn, ""));
        assert!(filter_connection(&conn, "openai"));
        assert!(filter_connection(&conn, "chromium"));
        assert!(filter_connection(&conn, "DMIT"));
        assert!(!filter_connection(&conn, "nonexistent"));
    }

    #[test]
    fn test_sort_connections() {
        let mut conns = vec![
            make_test_conn("1", "b.com", "", 100, 500),
            make_test_conn("2", "a.com", "", 900, 200),
            make_test_conn("3", "c.com", "", 500, 800),
        ];

        sort_connections(&mut conns, "download_desc");
        assert_eq!(conns[0].id, "3");
        assert_eq!(conns[1].id, "1");
        assert_eq!(conns[2].id, "2");

        sort_connections(&mut conns, "upload_desc");
        assert_eq!(conns[0].id, "2");
        assert_eq!(conns[1].id, "3");
        assert_eq!(conns[2].id, "1");

        sort_connections(&mut conns, "host_asc");
        assert_eq!(conns[0].id, "2");
        assert_eq!(conns[1].id, "1");
        assert_eq!(conns[2].id, "3");
    }

    #[test]
    fn test_stream_badge_kinds() {
        let _elem_idle: Element<'_, Message> = stream_badge(&RuntimeStreamState::Idle, &Lang("zh-CN"));
        let _elem_connected: Element<'_, Message> = stream_badge(&RuntimeStreamState::Connected, &Lang("zh-CN"));
        let _elem_failed: Element<'_, Message> = stream_badge(&RuntimeStreamState::Failed("err".into()), &Lang("zh-CN"));
    }
}
