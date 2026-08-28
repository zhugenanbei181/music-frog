use crate::locales::{Lang, Localizer};
use crate::types::RuntimeStatus;
use crate::utils::format_bytes;
use crate::view::components::{TrafficChart, card, modern_scrollable};
use crate::{AppState, Message};
use iced::widget::{
    Canvas, Scrollable, Space, button, checkbox, column, container, pick_list, row, text,
    text_input,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    if !matches!(
        state.status,
        RuntimeStatus::Running | RuntimeStatus::Starting
    ) {
        return container(card(column![
            text(lang.tr("proxy_not_running")),
            Space::new().height(12),
            button(text(lang.tr("start_proxy")).size(12))
                .on_press(Message::StartProxy)
                .padding([8, 16])
                .style(button::primary),
        ]))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let mode_options = vec![
        "rule".to_string(),
        "global".to_string(),
        "direct".to_string(),
        "script".to_string(),
    ];
    let selected_mode = state.proxy_mode.as_ref();
    let mut runtime_group_options: Vec<String> = state
        .proxies
        .iter()
        .filter_map(|(name, proxy)| {
            if proxy.is_group() {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    runtime_group_options.sort();
    if let Some(index) = runtime_group_options
        .iter()
        .position(|group| group == "GLOBAL")
    {
        let global = runtime_group_options.remove(index);
        runtime_group_options.insert(0, global);
    }
    let selected_runtime_group = if state.runtime_selected_group.trim().is_empty() {
        None
    } else {
        Some(&state.runtime_selected_group)
    };
    let runtime_proxy_options: Vec<String> = state
        .proxies
        .get(&state.runtime_selected_group)
        .and_then(|proxy| proxy.all())
        .map(|all| all.to_vec())
        .unwrap_or_default();
    let selected_runtime_proxy = if state.runtime_selected_proxy.trim().is_empty() {
        None
    } else {
        Some(&state.runtime_selected_proxy)
    };

    let runtime_action_btn: Element<'_, Message> =
        if matches!(state.status, RuntimeStatus::Starting) {
            button(text(lang.tr("status_starting")).size(12))
                .padding([6, 12])
                .style(button::secondary)
                .into()
        } else if matches!(state.status, RuntimeStatus::Running) {
            button(text(lang.tr("stop_proxy")).size(12))
                .on_press(Message::StopProxy)
                .padding([6, 12])
                .style(button::danger)
                .into()
        } else {
            button(text(lang.tr("start_proxy")).size(12))
                .on_press(Message::StartProxy)
                .padding([6, 12])
                .style(button::primary)
                .into()
        };

    let header = row![
        text(lang.tr("runtime_title")).size(24).font(bold_font),
        Space::new().width(Length::Fill),
        text(lang.tr("proxy_mode")).size(12),
        Space::new().width(8),
        pick_list(mode_options, selected_mode, Message::SetProxyMode)
            .text_size(11)
            .width(Length::Fixed(110.0)),
        Space::new().width(10),
        checkbox(state.runtime_auto_refresh)
            .label(lang.tr("runtime_auto_refresh").into_owned())
            .on_toggle(Message::UpdateRuntimeAutoRefresh)
            .size(14),
        Space::new().width(8),
        button(text(lang.tr("refresh")).size(11))
            .on_press(Message::RefreshRuntimeNow)
            .padding([6, 10])
            .style(button::secondary),
        Space::new().width(10),
        runtime_action_btn
    ]
    .align_y(Alignment::Center);

    let apply_proxy_btn: Element<'_, Message> = if state.runtime_selected_group.trim().is_empty()
        || state.runtime_selected_proxy.trim().is_empty()
    {
        button(text(lang.tr("runtime_apply_proxy")).size(11))
            .padding([6, 10])
            .style(button::secondary)
            .into()
    } else {
        button(text(lang.tr("runtime_apply_proxy")).size(11))
            .on_press(Message::ApplyRuntimeSelectedProxy)
            .padding([6, 10])
            .style(button::secondary)
            .into()
    };

    let runtime_proxy_selector = card(column![
        row![
            text(lang.tr("runtime_proxy_group")).size(12),
            Space::new().width(8),
            pick_list(
                runtime_group_options,
                selected_runtime_group,
                Message::UpdateRuntimeSelectedGroup
            )
            .width(Length::Fixed(180.0))
            .text_size(11),
            Space::new().width(16),
            text(lang.tr("runtime_proxy_node")).size(12),
            Space::new().width(8),
            pick_list(
                runtime_proxy_options,
                selected_runtime_proxy,
                Message::UpdateRuntimeSelectedProxy
            )
            .width(Length::Fixed(220.0))
            .text_size(11),
            Space::new().width(12),
            apply_proxy_btn,
        ]
        .align_y(Alignment::Center)
    ]);

    // 1. Real-time Traffic Section
    let traffic_section = card(column![
        row![
            column![
                text(lang.tr("overview_traffic"))
                    .size(10)
                    .font(bold_font)
                    .style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                    }),
                if let Some(ip) = &state.public_ip {
                    Element::from(text(format!("Public IP: {}", ip)).size(10).style(|_theme| {
                        text::Style {
                            color: Some(Color::from_rgb(0.4, 0.4, 0.4)),
                        }
                    }))
                } else {
                    Element::from(Space::new().width(0).height(0))
                }
            ],
            Space::new().width(Length::Fill),
            if let Some(t) = &state.traffic {
                row![
                    text(format!("↑ {}", format_bytes(t.up)))
                        .size(10)
                        .style(|_theme| text::Style {
                            color: Some(Color::from_rgb(0.4, 0.8, 0.4))
                        }),
                    Space::new().width(15),
                    text(format!("↓ {}", format_bytes(t.down)))
                        .size(10)
                        .style(|_theme| text::Style {
                            color: Some(Color::from_rgb(0.4, 0.4, 0.8))
                        }),
                    Space::new().width(15),
                    if let Some(m) = &state.memory {
                        Element::from(
                            text(format!("MEM: {}", format_bytes(m.in_use)))
                                .size(10)
                                .style(|_theme| text::Style {
                                    color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                                }),
                        )
                    } else {
                        Element::from(Space::new().width(0).height(0))
                    }
                ]
            } else {
                row![text(lang.tr("waiting_traffic")).size(10)]
            }
        ],
        Space::new().height(10),
        Canvas::new(TrafficChart {
            history: state.traffic_history.clone()
        })
        .width(Length::Fill)
        .height(Length::Fixed(100.0)),
    ]);

    let conn_sort_download_btn: Element<'_, Message> =
        if state.runtime_connection_sort == "download_desc" {
            button(text(lang.tr("runtime_conn_sort_download_desc")).size(10))
                .padding([4, 8])
                .style(button::primary)
                .into()
        } else {
            button(text(lang.tr("runtime_conn_sort_download_desc")).size(10))
                .on_press(Message::UpdateRuntimeConnectionSort(
                    "download_desc".to_string(),
                ))
                .padding([4, 8])
                .style(button::secondary)
                .into()
        };
    let conn_sort_upload_btn: Element<'_, Message> =
        if state.runtime_connection_sort == "upload_desc" {
            button(text(lang.tr("runtime_conn_sort_upload_desc")).size(10))
                .padding([4, 8])
                .style(button::primary)
                .into()
        } else {
            button(text(lang.tr("runtime_conn_sort_upload_desc")).size(10))
                .on_press(Message::UpdateRuntimeConnectionSort(
                    "upload_desc".to_string(),
                ))
                .padding([4, 8])
                .style(button::secondary)
                .into()
        };
    let conn_sort_latest_btn: Element<'_, Message> =
        if state.runtime_connection_sort == "latest_desc" {
            button(text(lang.tr("runtime_conn_sort_latest_desc")).size(10))
                .padding([4, 8])
                .style(button::primary)
                .into()
        } else {
            button(text(lang.tr("runtime_conn_sort_latest_desc")).size(10))
                .on_press(Message::UpdateRuntimeConnectionSort(
                    "latest_desc".to_string(),
                ))
                .padding([4, 8])
                .style(button::secondary)
                .into()
        };
    let conn_sort_host_btn: Element<'_, Message> = if state.runtime_connection_sort == "host_asc" {
        button(text(lang.tr("runtime_conn_sort_host_asc")).size(10))
            .padding([4, 8])
            .style(button::primary)
            .into()
    } else {
        button(text(lang.tr("runtime_conn_sort_host_asc")).size(10))
            .on_press(Message::UpdateRuntimeConnectionSort("host_asc".to_string()))
            .padding([4, 8])
            .style(button::secondary)
            .into()
    };

    let mut connections_section = column![
        row![
            text("CONNECTIONS")
                .size(10)
                .font(bold_font)
                .style(|_theme| text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                }),
            Space::new().width(12),
            text_input(
                lang.tr("runtime_conn_filter_placeholder").as_ref(),
                &state.runtime_connection_filter
            )
            .on_input(Message::UpdateRuntimeConnectionFilter)
            .padding([6, 10])
            .size(12)
            .width(Length::Fixed(220.0)),
            Space::new().width(6),
            if state.runtime_connection_filter.is_empty() {
                Element::from(Space::new().width(0).height(0))
            } else {
                Element::from(
                    button(text("Clear").size(10))
                        .on_press(Message::UpdateRuntimeConnectionFilter(String::new()))
                        .padding([4, 8])
                        .style(button::secondary),
                )
            },
            Space::new().width(12),
            text(lang.tr("runtime_conn_sort")).size(10),
            Space::new().width(6),
            conn_sort_download_btn,
            Space::new().width(4),
            conn_sort_upload_btn,
            Space::new().width(4),
            conn_sort_latest_btn,
            Space::new().width(4),
            conn_sort_host_btn,
            Space::new().width(Length::Fill),
            button(text(lang.tr("refresh")).size(10))
                .on_press(Message::RefreshRuntimeNow)
                .padding([4, 8])
                .style(button::secondary),
            Space::new().width(6),
            button(text(lang.tr("btn_close_all")).size(10))
                .on_press(Message::CloseAllConnections)
                .padding([4, 8])
                .style(button::danger),
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
    ];

    if let Some(c) = &state.connections {
        let mut conn_list = column![].spacing(8);

        let mut sorted_conns = c.connections.clone();
        let connection_filter = state.runtime_connection_filter.trim().to_lowercase();
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
            let ordering = match state.runtime_connection_sort.as_str() {
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
            connections_section = connections_section.push(
                text(lang.tr("runtime_no_matching_connections"))
                    .size(12)
                    .style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
                    }),
            );
        }

        for conn in sorted_conns {
            let host = if conn.metadata.host.is_empty() {
                conn.metadata.destination_ip.clone()
            } else {
                conn.metadata.host.clone()
            };

            let rule_str = format!("{}({})", conn.rule, conn.rule_payload);
            let payload_str = format!("{}:{}", host, conn.metadata.destination_port);
            let source_ip = format!("{} ->", conn.metadata.source_ip);
            let network = conn.metadata.network.to_uppercase();

            let row_content = column![
                row![
                    text(network)
                        .size(10)
                        .font(bold_font)
                        .style(|_theme| text::Style {
                            color: Some(Color::from_rgb(0.4, 0.4, 0.4))
                        }),
                    Space::new().width(8),
                    text(host).size(13).font(bold_font).width(Length::Fill),
                    text(format!(
                        "↑ {} / ↓ {}",
                        format_bytes(conn.upload),
                        format_bytes(conn.download)
                    ))
                    .size(10)
                    .style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                    }),
                    Space::new().width(12),
                    button(text("×").size(10))
                        .on_press(Message::CloseConnection(conn.id.clone()))
                        .padding([2, 6])
                        .style(button::danger),
                ]
                .align_y(Alignment::Center),
                row![
                    text(rule_str).size(10).style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.4, 0.6, 0.9))
                    }),
                    Space::new().width(8),
                    text(payload_str).size(10).style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                    }),
                    Space::new().width(Length::Fill),
                    text(source_ip).size(10).style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.4, 0.4, 0.4))
                    }),
                ]
            ]
            .spacing(4);

            conn_list =
                conn_list.push(container(row_content).padding(8).style(|_theme: &Theme| {
                    container::Style {
                        background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into()),
                        border: Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }));
        }

        connections_section =
            connections_section.push(modern_scrollable(conn_list).height(Length::Fill));
    } else {
        connections_section = connections_section.push(text("No active connections").size(12));
    }

    let delay_sort_delay_asc_btn: Element<'_, Message> = if state.proxy_delay_sort == "delay_asc" {
        button(text(lang.tr("runtime_delay_sort_delay_asc")).size(10))
            .padding([4, 8])
            .style(button::primary)
            .into()
    } else {
        button(text(lang.tr("runtime_delay_sort_delay_asc")).size(10))
            .on_press(Message::UpdateProxyDelaySort("delay_asc".to_string()))
            .padding([4, 8])
            .style(button::secondary)
            .into()
    };
    let delay_sort_delay_desc_btn: Element<'_, Message> = if state.proxy_delay_sort == "delay_desc"
    {
        button(text(lang.tr("runtime_delay_sort_delay_desc")).size(10))
            .padding([4, 8])
            .style(button::primary)
            .into()
    } else {
        button(text(lang.tr("runtime_delay_sort_delay_desc")).size(10))
            .on_press(Message::UpdateProxyDelaySort("delay_desc".to_string()))
            .padding([4, 8])
            .style(button::secondary)
            .into()
    };
    let delay_sort_name_asc_btn: Element<'_, Message> = if state.proxy_delay_sort == "name_asc" {
        button(text(lang.tr("runtime_delay_sort_name_asc")).size(10))
            .padding([4, 8])
            .style(button::primary)
            .into()
    } else {
        button(text(lang.tr("runtime_delay_sort_name_asc")).size(10))
            .on_press(Message::UpdateProxyDelaySort("name_asc".to_string()))
            .padding([4, 8])
            .style(button::secondary)
            .into()
    };
    let delay_sort_name_desc_btn: Element<'_, Message> = if state.proxy_delay_sort == "name_desc" {
        button(text(lang.tr("runtime_delay_sort_name_desc")).size(10))
            .padding([4, 8])
            .style(button::primary)
            .into()
    } else {
        button(text(lang.tr("runtime_delay_sort_name_desc")).size(10))
            .on_press(Message::UpdateProxyDelaySort("name_desc".to_string()))
            .padding([4, 8])
            .style(button::secondary)
            .into()
    };

    let delay_test_all_btn: Element<'_, Message> =
        if state.runtime_testing_all_delays || !state.runtime_testing_delay_proxy.is_empty() {
            button(text(lang.tr("runtime_delay_testing_all")).size(10))
                .padding([4, 8])
                .style(button::secondary)
                .into()
        } else {
            button(text(lang.tr("runtime_delay_test_all")).size(10))
                .on_press(Message::TestAllProxyDelays)
                .padding([4, 8])
                .style(button::secondary)
                .into()
        };

    let mut delay_nodes: Vec<(String, String, Option<u32>)> = state
        .proxies
        .iter()
        .filter_map(|(name, proxy)| {
            if proxy.is_group() {
                None
            } else {
                Some((
                    name.clone(),
                    proxy.proxy_type().to_string(),
                    proxy
                        .history()
                        .last()
                        .map(|item| item.delay)
                        .filter(|delay| *delay > 0),
                ))
            }
        })
        .collect();
    delay_nodes.sort_by(|(left_name, _, left_delay), (right_name, _, right_delay)| {
        let compare_delay = |desc: bool| match (left_delay, right_delay) {
            (None, None) => left_name.cmp(right_name),
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(left), Some(right)) => {
                let base = if desc {
                    right.cmp(left)
                } else {
                    left.cmp(right)
                };
                if base == std::cmp::Ordering::Equal {
                    left_name.cmp(right_name)
                } else {
                    base
                }
            }
        };
        match state.proxy_delay_sort.as_str() {
            "name_asc" => left_name.cmp(right_name),
            "name_desc" => right_name.cmp(left_name),
            "delay_desc" => compare_delay(true),
            _ => compare_delay(false),
        }
    });

    let mut delay_list = column![].spacing(8);
    if delay_nodes.is_empty() {
        delay_list = delay_list.push(text(lang.tr("runtime_delay_empty")).size(12).style(
            |_theme| text::Style {
                color: Some(Color::from_rgb(0.5, 0.5, 0.5)),
            },
        ));
    } else {
        for (name, proxy_type, delay) in delay_nodes {
            let is_testing =
                state.runtime_testing_all_delays || state.runtime_testing_delay_proxy == name;
            let test_button: Element<'_, Message> = if is_testing {
                button(text(lang.tr("runtime_delay_testing_one")).size(10))
                    .padding([4, 8])
                    .style(button::secondary)
                    .into()
            } else {
                button(text(lang.tr("runtime_delay_test_one")).size(10))
                    .on_press(Message::TestProxyDelay(name.clone()))
                    .padding([4, 8])
                    .style(button::secondary)
                    .into()
            };
            let delay_text = delay
                .map(|value| format!("{} ms", value))
                .unwrap_or_else(|| "-".to_string());

            delay_list = delay_list.push(
                container(
                    row![
                        column![
                            text(name).size(12).font(bold_font),
                            text(proxy_type).size(10).style(|_theme| text::Style {
                                color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                            })
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        text(delay_text).size(11),
                        Space::new().width(10),
                        test_button,
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([6, 8])
                .style(|_theme: &Theme| container::Style {
                    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.03).into()),
                    border: Border {
                        radius: 6.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        }
    }

    let delay_section = card(column![
        row![
            text(lang.tr("runtime_delay_title"))
                .size(10)
                .font(bold_font)
                .style(|_theme| text::Style {
                    color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                }),
            Space::new().width(10),
            delay_sort_delay_asc_btn,
            Space::new().width(4),
            delay_sort_delay_desc_btn,
            Space::new().width(4),
            delay_sort_name_asc_btn,
            Space::new().width(4),
            delay_sort_name_desc_btn,
            Space::new().width(Length::Fill),
            button(text(lang.tr("refresh")).size(10))
                .on_press(Message::LoadProxies)
                .padding([4, 8])
                .style(button::secondary),
            Space::new().width(6),
            delay_test_all_btn,
        ]
        .align_y(Alignment::Center),
        Space::new().height(8),
        row![
            text_input(
                lang.tr("runtime_delay_test_url_placeholder").as_ref(),
                &state.runtime_delay_test_url
            )
            .on_input(Message::UpdateDelayTestUrl)
            .padding([6, 10])
            .size(12)
            .width(Length::Fill),
            Space::new().width(8),
            text_input(
                lang.tr("runtime_delay_timeout_ms_placeholder").as_ref(),
                &state.runtime_delay_timeout_ms
            )
            .on_input(Message::UpdateDelayTimeoutMs)
            .padding([6, 10])
            .size(12)
            .width(Length::Fixed(140.0)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        modern_scrollable(delay_list).height(Length::Fill),
    ]);

    let logs_section = column![
        row![
            text(lang.tr("runtime_system_logs"))
                .font(bold_font)
                .width(Length::Fill),
            pick_list(
                &["debug", "info", "warning", "error"][..],
                Some(state.log_level.as_str()),
                |l| Message::SetLogLevel(l.to_string())
            )
            .text_size(11),
            Space::new().width(8),
            button(text("Clear").size(10))
                .on_press(Message::ClearRuntimeLogs)
                .padding([4, 8])
                .style(button::secondary),
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        container(
            Scrollable::new(
                column(state.logs.iter().map(|l| text(l).size(11).into()))
                    .spacing(2)
                    .padding(iced::Padding {
                        top: 5.0,
                        right: 16.0,
                        bottom: 5.0,
                        left: 5.0,
                    })
            )
            .id(iced::widget::Id::new("log_scroller"))
            .height(Length::Fill)
        )
        .style(|_theme| container::Style {
            background: Some(Color::BLACK.into()),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
    ];

    let content = column![
        header,
        Space::new().height(12),
        runtime_proxy_selector,
        Space::new().height(12),
        traffic_section,
        Space::new().height(16),
        container(card(connections_section)).height(Length::FillPortion(2)),
        Space::new().height(16),
        container(delay_section).height(Length::FillPortion(1)),
        Space::new().height(16),
        container(card(logs_section)).height(Length::FillPortion(1)),
    ]
    .spacing(10);

    content.into()
}
