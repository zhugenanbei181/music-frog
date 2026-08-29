use crate::locales::{Lang, Localizer};
use crate::types::RuntimeStatus;
use crate::utils::format_bytes;
use crate::view::components::{
    BadgeKind, TrafficChart, card, chip, empty_state, icon_button, latency_badge, modern_scrollable,
    section_header, segmented_control, stat_card, status_dot, toggle_switch,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CONTROL, SP_LG, SP_MD, tokens};
use crate::{AppState, Message};
use iced::widget::{
    Scrollable, Space, button, column, container, pick_list, row, text, text_input,
};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

// ---------------------------------------------------------------------------
// Token-driven control styles (ui-wave2-r)
// ---------------------------------------------------------------------------

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => {
            (Color { a: 0.85, ..tk.accent }, tk.on_accent)
        }
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
}

fn style_danger(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.text_tertiary),
        button::Status::Hovered | button::Status::Pressed => {
            (Color { a: 0.24, ..tk.danger }, tk.on_accent)
        }
        _ => (Color { a: 0.14, ..tk.danger }, tk.danger),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

/// Text push button: `on_press == None` renders the disabled state.
fn text_btn<'a>(
    label: String,
    style: fn(&Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([7, 14])
        .style(style)
        .on_press_maybe(on_press)
        .into()
}

fn input_style(t: &Theme, status: text_input::Status) -> text_input::Style {
    let tk = tokens(t);
    let (border_color, border_width) = match status {
        text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    text_input::Style {
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: border_width,
            color: border_color,
        },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color { a: 0.25, ..tk.accent },
    }
}

fn pick_style(t: &Theme, _status: pick_list::Status) -> pick_list::Style {
    let tk = tokens(t);
    pick_list::Style {
        text_color: tk.text_primary,
        placeholder_color: tk.text_tertiary,
        handle_color: tk.text_secondary,
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
    }
}

/// White list-row surface used inside section cards.
fn row_card(t: &Theme) -> container::Style {
    let tk = tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

/// Classify a raw log line into a badge kind (info→Neutral, warn→Warning,
/// error→Danger). Lines without a recognizable level get `None`.
fn log_kind(line: &str) -> Option<BadgeKind> {
    let upper = line.to_uppercase();
    if upper.contains("ERROR") || upper.contains("ERR") || upper.contains("FATAL") {
        Some(BadgeKind::Danger)
    } else if upper.contains("WARN") {
        Some(BadgeKind::Warning)
    } else if upper.contains("INFO") || upper.contains("INF") {
        Some(BadgeKind::Neutral)
    } else if upper.contains("DEBUG") || upper.contains("DBG") {
        Some(BadgeKind::Neutral)
    } else {
        None
    }
}

/// Localized proxy-mode pick-list option: renders `label` (via `Display`)
/// while `Message::SetProxyMode` keeps carrying the raw mihomo `value`.
#[derive(Clone, PartialEq)]
struct ModeOption {
    value: &'static str,
    label: String,
}

impl std::fmt::Display for ModeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);

    if !matches!(
        state.status,
        RuntimeStatus::Running | RuntimeStatus::Starting
    ) {
        return container(card(
            None,
            column![
                empty_state(Icon::Plug, lang.tr("proxy_not_running").as_ref(), ""),
                Space::new().height(SP_MD),
                text_btn(
                    lang.tr("start_proxy").to_string(),
                    style_accent,
                    Some(Message::StartProxy)
                ),
            ]
            .align_x(Alignment::Center),
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let mode_options = vec![
        ModeOption {
            value: "rule",
            label: lang.tr("proxy_mode_rule").to_string(),
        },
        ModeOption {
            value: "global",
            label: lang.tr("proxy_mode_global").to_string(),
        },
        ModeOption {
            value: "direct",
            label: lang.tr("proxy_mode_direct").to_string(),
        },
        ModeOption {
            value: "script",
            label: lang.tr("proxy_mode_script").to_string(),
        },
    ];
    let selected_mode = mode_options
        .iter()
        .find(|option| Some(option.value) == state.proxy_mode.as_deref())
        .cloned();
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
            text_btn(lang.tr("status_starting").to_string(), style_ghost, None)
        } else if matches!(state.status, RuntimeStatus::Running) {
            text_btn(
                lang.tr("stop_proxy").to_string(),
                style_danger,
                Some(Message::StopProxy),
            )
        } else {
            text_btn(
                lang.tr("start_proxy").to_string(),
                style_accent,
                Some(Message::StartProxy),
            )
        };

    let header = row![
        text(lang.tr("runtime_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        text(lang.tr("proxy_mode").to_string()).size(12).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
        Space::new().width(theme::SP_SM),
        pick_list(mode_options, selected_mode, |mode: ModeOption| {
            Message::SetProxyMode(mode.value.to_string())
        })
        .text_size(12)
        .width(Length::Fixed(110.0))
        .style(pick_style),
        Space::new().width(theme::SP_LG),
        text(lang.tr("runtime_auto_refresh").to_string()).size(12).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
        Space::new().width(theme::SP_SM),
        toggle_switch(state.runtime_auto_refresh, Message::UpdateRuntimeAutoRefresh),
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 16.0, Message::RefreshRuntimeNow),
        Space::new().width(theme::SP_SM),
        runtime_action_btn,
    ]
    .align_y(Alignment::Center);

    let apply_proxy_enabled = !state.runtime_selected_group.trim().is_empty()
        && !state.runtime_selected_proxy.trim().is_empty();
    let apply_proxy_btn = text_btn(
        lang.tr("runtime_apply_proxy").to_string(),
        if apply_proxy_enabled {
            style_accent
        } else {
            style_ghost
        },
        apply_proxy_enabled.then_some(Message::ApplyRuntimeSelectedProxy),
    );

    let runtime_proxy_selector = card(
        None,
        row![
            column![
                text(lang.tr("runtime_proxy_group").to_string()).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
                Space::new().height(theme::SP_XS),
                pick_list(
                    runtime_group_options,
                    selected_runtime_group,
                    Message::UpdateRuntimeSelectedGroup
                )
                .width(Length::Fixed(180.0))
                .text_size(12)
                .style(pick_style),
            ],
            Space::new().width(theme::SP_XL),
            column![
                text(lang.tr("runtime_proxy_node").to_string()).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
                Space::new().height(theme::SP_XS),
                pick_list(
                    runtime_proxy_options,
                    selected_runtime_proxy,
                    Message::UpdateRuntimeSelectedProxy
                )
                .width(Length::Fixed(220.0))
                .text_size(12)
                .style(pick_style),
            ],
            Space::new().width(theme::SP_XL),
            container(apply_proxy_btn).align_y(Alignment::Center),
        ]
        .align_y(Alignment::Center),
    );

    // 1. Real-time Traffic Section
    let theme_tokens = tokens(&state.theme);
    let ip_stat = stat_card(
        Icon::Globe,
        lang.tr("runtime_stat_public_ip").as_ref(),
        state.public_ip.as_deref().unwrap_or("—"),
        theme_tokens.accent,
        false,
    );
    let traffic_trailing = if state.traffic.is_none() {
        Some(
            text(lang.tr("waiting_traffic").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                })
                .into(),
        )
    } else {
        None
    };

    let traffic_section = card(
        None,
        column![
            section_header(lang.tr("overview_traffic").as_ref(), traffic_trailing),
            Space::new().height(theme::SP_MD),
            row![
                stat_card(
                    Icon::ArrowUp,
                    lang.tr("runtime_stat_up").as_ref(),
                    state
                        .traffic
                        .as_ref()
                        .map(|t| format_bytes(t.up))
                        .unwrap_or_else(|| "—".to_string())
                        .as_str(),
                    theme_tokens.success,
                    false,
                ),
                stat_card(
                    Icon::ArrowDown,
                    lang.tr("runtime_stat_down").as_ref(),
                    state
                        .traffic
                        .as_ref()
                        .map(|t| format_bytes(t.down))
                        .unwrap_or_else(|| "—".to_string())
                        .as_str(),
                    theme_tokens.accent,
                    false,
                ),
                stat_card(
                    Icon::Server,
                    lang.tr("runtime_stat_memory").as_ref(),
                    state
                        .memory
                        .as_ref()
                        .map(|m| format_bytes(m.in_use))
                        .unwrap_or_else(|| "—".to_string())
                        .as_str(),
                    theme_tokens.warning,
                    false,
                ),
                ip_stat,
            ]
            .spacing(SP_MD),
            Space::new().height(theme::SP_MD),
            // The chart owns its surface; it lives directly in this card like
            // the section header above (no extra frame here).
            iced::widget::Canvas::new(TrafficChart {
                history: state.traffic_history.clone()
            })
            .width(Length::Fill)
            .height(Length::Fixed(110.0)),
        ],
    );

    let sort_labels: Vec<String> = vec![
        lang.tr("runtime_conn_sort_download_desc").to_string(),
        lang.tr("runtime_conn_sort_upload_desc").to_string(),
        lang.tr("runtime_conn_sort_latest_desc").to_string(),
        lang.tr("runtime_conn_sort_host_asc").to_string(),
    ];
    let sort_index = match state.runtime_connection_sort.as_str() {
        "upload_desc" => 1,
        "latest_desc" => 2,
        "host_asc" => 3,
        _ => 0,
    };
    let conn_sort_control = segmented_control(
        &sort_labels,
        sort_index,
        |index| {
            let key = match index {
                1 => "upload_desc",
                2 => "latest_desc",
                3 => "host_asc",
                _ => "download_desc",
            };
            Message::UpdateRuntimeConnectionSort(key.to_string())
        },
    );

    let filter_row = row![
        svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_SM),
        text_input(
            lang.tr("runtime_conn_filter_placeholder").as_ref(),
            &state.runtime_connection_filter
        )
        .on_input(Message::UpdateRuntimeConnectionFilter)
        .padding([8, 12])
        .size(12)
        .font(MONO)
        .width(Length::Fixed(260.0))
        .style(input_style),
        Space::new().width(theme::SP_SM),
        if state.runtime_connection_filter.is_empty() {
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
                    conn_sort_control,
                    Space::new().width(theme::SP_SM),
                    icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
                    Space::new().width(theme::SP_SM),
                    text_btn(
                        lang.tr("btn_close_all").to_string(),
                        style_danger,
                        Some(Message::CloseAllConnections)
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

    if let Some(c) = &state.connections {
        let mut conn_list = column![].spacing(theme::SP_SM);

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
                    text(rule_str)
                        .size(11)
                        .style(|t: &Theme| text::Style {
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

    let delay_sort_labels: Vec<String> = vec![
        lang.tr("runtime_delay_sort_delay_asc").to_string(),
        lang.tr("runtime_delay_sort_delay_desc").to_string(),
        lang.tr("runtime_delay_sort_name_asc").to_string(),
        lang.tr("runtime_delay_sort_name_desc").to_string(),
    ];
    let delay_sort_index = match state.proxy_delay_sort.as_str() {
        "delay_desc" => 1,
        "name_asc" => 2,
        "name_desc" => 3,
        _ => 0,
    };
    let delay_sort_control = segmented_control(
        &delay_sort_labels,
        delay_sort_index,
        |index| {
            let key = match index {
                1 => "delay_desc",
                2 => "name_asc",
                3 => "name_desc",
                _ => "delay_asc",
            };
            Message::UpdateProxyDelaySort(key.to_string())
        },
    );

    let delay_testing = state.runtime_testing_all_delays
        || !state.runtime_testing_delay_proxy.is_empty();
    let delay_test_all_btn = text_btn(
        if delay_testing {
            lang.tr("runtime_delay_testing_all").to_string()
        } else {
            lang.tr("runtime_delay_test_all").to_string()
        },
        style_ghost,
        (!delay_testing).then_some(Message::TestAllProxyDelays),
    );

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

    let mut delay_list = column![].spacing(theme::SP_SM);
    if delay_nodes.is_empty() {
        delay_list = delay_list.push(empty_state(
            Icon::Activity,
            lang.tr("runtime_delay_empty").as_ref(),
            "",
        ));
    } else {
        for (name, proxy_type, delay) in delay_nodes {
            let is_testing =
                state.runtime_testing_all_delays || state.runtime_testing_delay_proxy == name;
            let test_button = text_btn(
                if is_testing {
                    lang.tr("runtime_delay_testing_one").to_string()
                } else {
                    lang.tr("runtime_delay_test_one").to_string()
                },
                style_ghost,
                (!is_testing).then_some(Message::TestProxyDelay(name.clone())),
            );

            delay_list = delay_list.push(
                container(
                    row![
                        column![
                            text(name)
                                .size(12)
                                .font(FONT_SEMIBOLD)
                                .style(|t: &Theme| text::Style {
                                    color: Some(tokens(t).text_primary),
                                }),
                            chip(proxy_type),
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        latency_badge(delay),
                        Space::new().width(theme::SP_MD),
                        test_button,
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([theme::SP_SM, SP_MD])
                .width(Length::Fill)
                .style(row_card),
            );
        }
    }

    let delay_section = card(
        None,
        column![
            section_header(
                lang.tr("runtime_delay_title").as_ref(),
                Some(
                    row![
                        delay_sort_control,
                        Space::new().width(theme::SP_SM),
                        icon_button(Icon::RefreshCw, 14.0, Message::LoadProxies),
                        Space::new().width(theme::SP_SM),
                        delay_test_all_btn,
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                ),
            ),
            Space::new().height(theme::SP_MD),
            row![
                text_input(
                    lang.tr("runtime_delay_test_url_placeholder").as_ref(),
                    &state.runtime_delay_test_url
                )
                .on_input(Message::UpdateDelayTestUrl)
                .padding([8, 12])
                .size(12)
                .width(Length::Fill)
                .style(input_style),
                Space::new().width(theme::SP_SM),
                text_input(
                    lang.tr("runtime_delay_timeout_ms_placeholder").as_ref(),
                    &state.runtime_delay_timeout_ms
                )
                .on_input(Message::UpdateDelayTimeoutMs)
                .padding([8, 12])
                .size(12)
                .font(MONO)
                .width(Length::Fixed(140.0))
                .style(input_style),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            delay_list,
        ],
    );

    let logs_trailing = row![
        pick_list(
            &["debug", "info", "warning", "error"][..],
            Some(state.log_level.as_str()),
            |l| Message::SetLogLevel(l.to_string())
        )
        .text_size(12)
        .style(pick_style),
        Space::new().width(theme::SP_SM),
        icon_button(Icon::Trash2, 14.0, Message::ClearRuntimeLogs),
    ]
    .align_y(Alignment::Center);

    let log_lines: Vec<Element<'_, Message>> = state
        .logs
        .iter()
        .map(|l| {
            row![
                match log_kind(l) {
                    Some(kind) => badge_for_kind(kind),
                    None => Space::new().width(0).height(0).into(),
                },
                Space::new().width(theme::SP_SM),
                text(l.clone())
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect();

    let logs_section = column![
        section_header(lang.tr("runtime_system_logs").as_ref(), Some(logs_trailing.into())),
        Space::new().height(theme::SP_MD),
        container(
            Scrollable::new(
                column(log_lines).spacing(2).padding(iced::Padding {
                    top: theme::SP_SM as f32,
                    right: SCROLL_PAD,
                    bottom: theme::SP_SM as f32,
                    left: theme::SP_SM as f32,
                })
            )
            .id(iced::widget::Id::new("log_scroller"))
            // Definite height: `snap_to("log_scroller", ...)` in the update
            // path needs a real scrolling viewport, and a Fill height would
            // collapse inside the auto-height card.
            .height(Length::Fixed(240.0))
        )
        .style(|t: &Theme| container::Style {
            background: Some(tokens(t).control_bg.into()),
            border: Border {
                radius: border::Radius::from(R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill),
    ];

    let content = column![
        header,
        runtime_proxy_selector,
        traffic_section,
        card(None, connections_section),
        card(None, delay_section),
        card(None, logs_section),
    ]
    .spacing(SP_LG);

    // Page-level scrolling (same idiom as overview/dns): sections keep their
    // natural height, so the Fill-height scrollables inside Shrink-height
    // cards can no longer collapse to blank slivers.
    modern_scrollable(content).height(Length::Fill).into()
}

/// Fixed right padding so log text does not sit under the scrollbar.
const SCROLL_PAD: f32 = 16.0;

/// Small tinted pill for a log level.
fn badge_for_kind(kind: BadgeKind) -> Element<'static, Message> {
    crate::view::components::badge(level_label(kind), kind)
}

fn level_label(kind: BadgeKind) -> &'static str {
    match kind {
        BadgeKind::Danger => "ERR",
        BadgeKind::Warning => "WARN",
        _ => "INFO",
    }
}
