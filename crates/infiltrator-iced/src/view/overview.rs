//! Overview page in the Clash-Party design language: section header with
//! quick actions, a runtime status hero card, a four-tile stats grid with mono numerals,
//! real-time traffic chart, network topology flow diagram, current IP probe card,
//! and multi-target latency comparison bars.

use crate::state::AppState;
use crate::types::app::{Route, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::active_exit::active_exit_card;
use crate::view::components::{
    BadgeKind, badge, card_surface, chip, icon_button, modern_scrollable, premium_card,
    row_card_surface, section_header, status_dot, style_accent, style_ghost,
};
use crate::view::overview_master_switches::overview_master_switches;
use crate::view::overview_mode_segment::overview_mode_segment;
use crate::view::subscription_quota::subscription_quota_card;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CHIP, R_CONTROL, tokens};
use crate::view::topology::topology_flow_canvas;
use crate::view::waveform::TrafficChart;
use iced::widget::{Space, button, canvas, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_application::traffic_topology_navigation_application::TrafficTopologyNavigationApplication;
use infiltrator_contract::traffic_topology::{
    TrafficTopologySnapshot, TrafficTopologyStage, TrafficTopologyStatus,
};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let is_en = state.shell.lang.starts_with("en");

    let header = section_header(
        &lang.tr("nav_overview"),
        Some(
            row![
                overview_speedtest_button(state, &lang),
                Space::new().width(theme::SP_SM),
                icon_button(Icon::RefreshCw, 16.0, Message::RefreshRuntimeNow),
                Space::new().width(theme::SP_SM),
                icon_button(Icon::Settings, 16.0, Message::Navigate(Route::Settings)),
            ]
            .align_y(Alignment::Center)
            .into(),
        ),
    );

    let hero = hero_card(state, &lang);
    let mode_segment = overview_mode_segment(state, &lang);
    let stats = stats_grid(state, &lang);
    let masters = overview_master_switches(state, &lang);
    let traffic = traffic_card(state, &lang);
    let topology = topology_card(state, &lang, is_en);
    let quota = subscription_quota_card(state, &lang);

    // Graceful degradation mask shared with Bevy: while the core reloads or
    // the watchdog reconnects, the last valid frame stays visible and the
    // banner names the phase instead of silently blanking the page.
    let reconnect_banner: Option<Element<'_, Message>> = if state.runtime.reconnect_mask.is_active()
    {
        let mask = &state.runtime.reconnect_mask;
        let detail = match (mask.attempt, mask.retry_in_ms) {
            (Some(attempt), Some(retry_ms)) => {
                format!(
                    "{} · {} {}",
                    mask.message.clone().unwrap_or_default(),
                    attempt,
                    retry_ms
                )
            }
            (Some(attempt), None) => {
                format!(
                    "{} · #{}",
                    mask.message.clone().unwrap_or_default(),
                    attempt
                )
            }
            _ => mask.message.clone().unwrap_or_default(),
        };
        Some(
            container(
                row![
                    icon_themed(Icon::Shield, 16.0, |t: &Theme| tokens(t).on_accent),
                    Space::new().width(theme::SP_SM),
                    text(detail)
                        .size(12)
                        .font(FONT_MEDIUM)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).on_accent)
                        }),
                    Space::new().width(Length::Fill),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 16])
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(
                        Color {
                            a: 0.85,
                            ..tk.accent
                        }
                        .into(),
                    ),
                    border: Border {
                        radius: border::Radius::from(theme::R_CARD),
                        width: 1.0,
                        color: tk.accent,
                    },
                    ..Default::default()
                }
            })
            .into(),
        )
    } else {
        None
    };

    // Reorderable cards are assembled according to the shared layout order so
    // the Iced surface honours the same `OverviewLayoutSnapshot` Bevy does.
    // Active-exit / public-IP / latency are the fixed support row and are not
    // part of the reorderable set.
    let mut reorderable: Vec<(
        infiltrator_contract::overview_layout::OverviewCardKind,
        Element<'_, Message>,
    )> = vec![
        (
            infiltrator_contract::overview_layout::OverviewCardKind::ModeSegment,
            mode_segment,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Traffic,
            traffic,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Metrics,
            stats,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::MasterSwitches,
            masters,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Topology,
            topology,
        ),
        (
            infiltrator_contract::overview_layout::OverviewCardKind::Quota,
            quota,
        ),
    ];
    let lower_row = row![
        active_exit_card(state, &lang),
        current_ip_card(state, &lang, is_en),
        latency_card(state, &lang, is_en),
    ]
    .spacing(theme::SP_LG)
    .width(Length::Fill);

    let mut content = column![header];
    if let Some(banner) = reconnect_banner {
        content = content.push(banner);
    }
    content = content.push(hero).spacing(theme::SP_LG);
    let mut ordered: Vec<(_, Element<'_, Message>)> = Vec::new();
    for kind in &state.diag.overview_card_order {
        if let Some(pos) = reorderable.iter().position(|(k, _)| k == kind) {
            let (_, element) = reorderable.remove(pos);
            ordered.push((*kind, element));
        }
    }
    // Any card missing from the persisted order (e.g. a newly added kind) is
    // appended in canonical order so no card can silently disappear.
    ordered.extend(reorderable);
    for (kind, element) in ordered {
        content = content.push(card_reorder_row(state, &lang, kind, element));
    }
    content = content.push(lower_row);
    // Reset only appears once the order diverges from the canonical default.
    if state.diag.overview_card_order
        != infiltrator_contract::overview_layout::OverviewCardKind::DEFAULT_ORDER.to_vec()
    {
        content = content.push(
            row![
                Space::new().width(Length::Fill),
                icon_button(Icon::RefreshCw, 13.0, Message::ResetOverviewCardOrder),
                Space::new().width(theme::SP_XS),
                text(lang.tr("overview_reset_card_order").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
            ]
            .align_y(Alignment::Center),
        );
    }
    let content = content.spacing(theme::SP_LG).max_width(1100);

    modern_scrollable(content).height(Length::Fill).into()
}

/// Wrap one reorderable Overview card with its up/down controls so both
/// surfaces expose the same shared-layout reorder intent.
fn card_reorder_row<'a>(
    state: &AppState,
    _lang: &Lang<'a>,
    kind: infiltrator_contract::overview_layout::OverviewCardKind,
    card: Element<'a, Message>,
) -> Element<'a, Message> {
    let order = &state.diag.overview_card_order;
    let position = order.iter().position(|k| *k == kind);
    let can_up = position.is_some_and(|p| p > 0);
    let can_down = position.is_some_and(|p| p + 1 < order.len());

    let up: Element<'a, Message> = if can_up {
        icon_button(Icon::ArrowUp, 12.0, Message::MoveOverviewCardUp(kind))
    } else {
        Space::new().width(0).into()
    };
    let down: Element<'a, Message> = if can_down {
        icon_button(Icon::ArrowDown, 12.0, Message::MoveOverviewCardDown(kind))
    } else {
        Space::new().width(0).into()
    };

    column![
        row![
            Space::new().width(Length::Fill),
            down,
            Space::new().width(2),
            up
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
        card,
    ]
    .spacing(theme::SP_XS)
    .width(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Runtime status hero
// ---------------------------------------------------------------------------

/// Accent hero: status dot + localized status, mode / core-version meta row
/// and the prominent start/stop control.
pub fn overview_speedtest_button<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let is_testing = state.runtime.runtime_testing_all_delays;
    let label = if is_testing {
        lang.tr("runtime_delay_testing_all").into_owned()
    } else {
        lang.tr("runtime_delay_test_all").into_owned()
    };
    let btn_content = row![
        icon_themed(Icon::Zap, 14.0, move |t: &Theme| {
            if is_testing {
                tokens(t).text_secondary
            } else {
                tokens(t).accent
            }
        }),
        Space::new().width(theme::SP_XS),
        text(label)
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(move |t: &Theme| text::Style {
                color: Some(if is_testing {
                    tokens(t).text_secondary
                } else {
                    tokens(t).accent
                }),
            }),
    ]
    .align_y(Alignment::Center);

    button(btn_content)
        .padding([6, 12])
        .style(style_ghost)
        .on_press_maybe((!is_testing).then_some(Message::TestAllProxyDelays))
        .into()
}

fn hero_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let running = matches!(state.runtime.status, RuntimeStatus::Running);
    let status_text = match &state.runtime.status {
        RuntimeStatus::Starting => lang.tr("status_starting"),
        RuntimeStatus::Running => lang.tr("status_running"),
        RuntimeStatus::Error(_) => lang.tr("status_error"),
        RuntimeStatus::Stopped => lang.tr("status_stopped"),
    };

    let control: Element<'a, Message> = if running {
        button(
            row![
                icon_themed(Icon::Plug, 14.0, |t: &Theme| tokens(t).on_accent),
                text(lang.tr("stop_proxy").into_owned())
                    .size(13)
                    .font(FONT_SEMIBOLD),
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center),
        )
        .padding([10, 20])
        .style(button::danger)
        .on_press(Message::StopProxy)
        .into()
    } else {
        button(
            row![
                icon_themed(Icon::Zap, 14.0, |t: &Theme| tokens(t).on_accent),
                text(lang.tr("start_proxy").into_owned())
                    .size(13)
                    .font(FONT_SEMIBOLD),
            ]
            .spacing(theme::SP_SM)
            .align_y(Alignment::Center),
        )
        .padding([10, 20])
        .style(style_accent)
        .on_press(Message::StartProxy)
        .into()
    };

    premium_card(
        row![
            status_dot(running),
            Space::new().width(theme::SP_MD),
            column![
                text(status_text.into_owned())
                    .size(22)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                meta_row(state, lang),
            ]
            .spacing(theme::SP_XS),
            Space::new().width(Length::Fill),
            control,
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
}

/// Mode chip + core version + current GLOBAL exit, all from existing state.
fn meta_row<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let mut meta = row![].spacing(theme::SP_SM).align_y(Alignment::Center);

    if let Some(mode) = state.runtime.proxy_mode.as_deref() {
        meta = meta.push(chip(mode_label(mode, lang)));
    }

    if let Some(version) = default_core_version(state) {
        meta = meta.push(
            container(
                row![
                    icon_themed(Icon::Server, 12.0, |t: &Theme| tokens(t).text_secondary),
                    Space::new().width(theme::SP_XS),
                    text(format!("mihomo {version}"))
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| {
                            text::Style {
                                color: Some(tokens(t).text_secondary),
                            }
                        }),
                ]
                .align_y(Alignment::Center),
            )
            .padding([3, 8])
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).chip_bg.into()),
                border: Border {
                    radius: border::Radius::from(R_CHIP),
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }

    if let Some(exit_node) = state.runtime.proxies.get("GLOBAL").and_then(|g| g.now()) {
        meta = meta.push(
            container(
                row![
                    icon_themed(Icon::Globe, 12.0, |t: &Theme| tokens(t).accent),
                    Space::new().width(theme::SP_XS),
                    text(exit_node.to_string())
                        .size(11)
                        .font(MONO)
                        .style(|t: &Theme| {
                            text::Style {
                                color: Some(tokens(t).text_primary),
                            }
                        }),
                ]
                .align_y(Alignment::Center),
            )
            .padding([3, 10])
            .style(|t: &Theme| container::Style {
                background: Some(tokens(t).chip_bg.into()),
                border: Border {
                    radius: border::Radius::from(R_CHIP),
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
    }

    meta.into()
}

/// Localized label for a mihomo mode identifier (unknown values pass through).
fn mode_label(mode: &str, lang: &Lang<'_>) -> String {
    match mode {
        "rule" => lang.tr("mode_rule").into_owned(),
        "global" => lang.tr("mode_global").into_owned(),
        "direct" => lang.tr("mode_direct").into_owned(),
        "script" => lang.tr("mode_script").into_owned(),
        _ => mode.to_string(),
    }
}

/// Version of the installed default kernel, if one is registered.
fn default_core_version(state: &AppState) -> Option<String> {
    state
        .runtime
        .installed_kernels
        .iter()
        .find(|kernel| kernel.is_default)
        .map(|kernel| kernel.version.clone())
}

// ---------------------------------------------------------------------------
// Stats grid
// ---------------------------------------------------------------------------

/// 连接数 / 内存 / 上传 / 下载 tiles with mono numerals.
///
/// The six tiles wrap into rows of `metrics_grid_columns` for the shell's
/// current tier (2 / 3 / 6 / 6) so a compact window never squeezes six columns
/// into one unreadable strip.
fn stats_grid<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let connections = state
        .diag
        .connections
        .as_ref()
        .map(|snapshot| snapshot.connections.len().to_string())
        .unwrap_or_else(|| "—".to_string());
    let memory = state
        .diag
        .memory
        .as_ref()
        .map(|memory| crate::utils::format_bytes(memory.in_use))
        .unwrap_or_else(|| "—".to_string());
    let upload = state
        .diag
        .traffic
        .as_ref()
        .map(|traffic| format!("{}/s", crate::utils::format_bytes(traffic.up)))
        .unwrap_or_else(|| "—".to_string());
    let cpu = state
        .runtime
        .core_resources
        .cpu_percent
        .map(|p| format!("{:.1}%", p))
        .unwrap_or_else(|| "—".to_string());
    let download = state
        .diag
        .traffic
        .as_ref()
        .map(|traffic| format!("{}/s", crate::utils::format_bytes(traffic.down)))
        .unwrap_or_else(|| "—".to_string());
    let total = state
        .diag
        .connections
        .as_ref()
        .map(|c| crate::utils::format_bytes(c.download_total + c.upload_total))
        .unwrap_or_else(|| "—".to_string());

    let tiles: Vec<Element<'a, Message>> = vec![
        metric_tile(
            Icon::Activity,
            lang.tr("overview_connections").to_string(),
            connections,
            |t| tokens(t).accent,
        ),
        metric_tile(
            Icon::Server,
            lang.tr("overview_memory").to_string(),
            memory,
            |t| tokens(t).warning,
        ),
        metric_tile(Icon::Zap, "CPU".to_string(), cpu, |t| tokens(t).accent),
        metric_tile(
            Icon::ArrowUp,
            lang.tr("overview_upload").to_string(),
            upload,
            |t| tokens(t).success,
        ),
        metric_tile(
            Icon::ArrowDown,
            lang.tr("overview_download").to_string(),
            download,
            |t| tokens(t).accent,
        ),
        metric_tile(
            Icon::Globe,
            lang.tr("overview_total_traffic").to_string(),
            total,
            |t| tokens(t).success,
        ),
    ];

    // Column count comes from the shared tier operator, so Iced and Bevy agree.
    let columns = state
        .shell
        .viewport
        .tier
        .metrics_grid_columns()
        .clamp(1, tiles.len().max(1));

    let mut grid = column![].spacing(theme::SP_MD).width(Length::Fill);
    let mut row_tiles: Vec<Element<'a, Message>> = Vec::with_capacity(columns);
    for tile in tiles {
        row_tiles.push(tile);
        if row_tiles.len() == columns {
            grid = grid.push(
                row::Row::with_children(std::mem::take(&mut row_tiles))
                    .spacing(theme::SP_MD)
                    .width(Length::Fill),
            );
        }
    }
    if !row_tiles.is_empty() {
        // Pad the trailing row so the last tiles keep the same width as a full row.
        for _ in row_tiles.len()..columns {
            row_tiles.push(Space::new().width(Length::FillPortion(1)).into());
        }
        grid = grid.push(
            row::Row::with_children(row_tiles)
                .spacing(theme::SP_MD)
                .width(Length::Fill),
        );
    }

    grid.into()
}

fn metric_tile<'a>(
    glyph: Icon,
    label: String,
    value: String,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let icon_chip = container(icon_themed(glyph, 16.0, color))
        .width(36)
        .height(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |t: &Theme| {
            let c = color(t);
            container::Style {
                background: Some(Color { a: 0.14, ..c }.into()),
                border: Border {
                    radius: border::Radius::from(R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    container(
        row![
            icon_chip,
            column![
                text(label)
                    .size(11)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                text(value)
                    .size(15)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary),
                    }),
            ]
            .spacing(2),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .width(Length::FillPortion(1))
    .padding([theme::SP_MD, theme::SP_MD])
    .style(card_surface)
    .into()
}

// ---------------------------------------------------------------------------
// Traffic chart
// ---------------------------------------------------------------------------

fn traffic_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let (up_speed, down_speed) = state
        .diag
        .traffic
        .as_ref()
        .map(|t| (t.up, t.down))
        .unwrap_or((0, 0));

    let speed_legend = row![
        speed_pill(Icon::ArrowUp, up_speed, |t| tokens(t).success),
        Space::new().width(theme::SP_MD),
        speed_pill(Icon::ArrowDown, down_speed, |t| tokens(t).accent),
    ]
    .align_y(Alignment::Center);
    let scale = if state.runtime.traffic_waveform.is_drawable() {
        state.runtime.traffic_scale.clone()
    } else {
        let upload: Vec<f64> = state
            .diag
            .traffic_history
            .iter()
            .map(|(up, _)| *up as f64)
            .collect();
        let download: Vec<f64> = state
            .diag
            .traffic_history
            .iter()
            .map(|(_, down)| *down as f64)
            .collect();
        infiltrator_domain::traffic_scale::compute_from_rates(&upload, &download, 0)
    };

    let card_header = row![
        row![
            icon_themed(Icon::Activity, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_traffic").into_owned())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        text(format!(
            "{} {}",
            lang.tr("overview_scale_max"),
            scale.format_max()
        ))
        .size(10)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        }),
        Space::new().width(theme::SP_SM),
        speed_legend,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let chart = canvas::Canvas::new(TrafficChart {
        history: state.diag.traffic_history.clone(),
        shared: state
            .runtime
            .traffic_waveform
            .is_drawable()
            .then(|| state.runtime.traffic_waveform.clone()),
        scale: Some(scale),
    })
    .width(Length::Fill)
    .height(Length::Fixed(130.0));

    container(column![card_header, Space::new().height(theme::SP_MD), chart].spacing(theme::SP_XS))
        .width(Length::Fill)
        .padding(theme::SP_XXL)
        .style(card_surface)
        .into()
}

fn speed_pill<'a>(
    glyph: Icon,
    bytes_per_second: u64,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    container(
        row![
            icon_themed(glyph, 13.0, color),
            Space::new().width(theme::SP_XS),
            text(format!(
                "{}/s",
                crate::utils::format_bytes(bytes_per_second)
            ))
            .size(13)
            .font(MONO)
            .style(move |t: &Theme| text::Style {
                color: Some(color(t))
            }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(move |t: &Theme| {
        let c = color(t);
        container::Style {
            background: Some(Color { a: 0.10, ..c }.into()),
            border: Border {
                radius: border::Radius::from(R_CHIP),
                width: 1.0,
                color: Color { a: 0.20, ..c },
            },
            ..Default::default()
        }
    })
    .into()
}

// ---------------------------------------------------------------------------
// Network topology graph / Flow preview (P02-04 ~ P02-07)
// ---------------------------------------------------------------------------

fn topology_card<'a>(state: &'a AppState, lang: &Lang<'a>, _is_en: bool) -> Element<'a, Message> {
    let topology = &state.runtime.traffic_topology;

    let card_header = row![
        row![
            icon_themed(Icon::Network, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_topology_title").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        badge(
            topology_badge(topology, lang),
            topology_badge_kind(topology),
        ),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let node_inbound = topology_stage_node(
        topology,
        TrafficTopologyStage::Inbound,
        Icon::Server,
        "Client / Inbound",
        BadgeKind::Neutral,
        |t| tokens(t).text_secondary,
    );
    let node_sniffer = topology_stage_node(
        topology,
        TrafficTopologyStage::Sniffer,
        Icon::Search,
        "Sniffer",
        BadgeKind::Accent,
        |t| tokens(t).accent,
    );
    let node_ruleset = topology_stage_node(
        topology,
        TrafficTopologyStage::RuleSet,
        Icon::Shield,
        "RuleSet",
        BadgeKind::Accent,
        |t| tokens(t).accent,
    );
    let node_group = topology_stage_node(
        topology,
        TrafficTopologyStage::ProxyGroup,
        Icon::LayoutGrid,
        "Proxy Group",
        BadgeKind::Warning,
        |t| tokens(t).warning,
    );
    let node_outbound = topology_stage_node(
        topology,
        TrafficTopologyStage::Outbound,
        Icon::Globe,
        "Outbound Node",
        BadgeKind::Success,
        |t| tokens(t).success,
    );

    let flow_row = row![
        node_inbound,
        arrow_connector(),
        node_sniffer,
        arrow_connector(),
        node_ruleset,
        arrow_connector(),
        node_group,
        arrow_connector(),
        node_outbound,
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill);

    container(
        column![
            card_header,
            Space::new().height(theme::SP_SM),
            topology_flow_canvas(topology, state.diag.topology_flow_phase),
            flow_row,
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::Fill)
    .padding(theme::SP_XXL)
    .style(card_surface)
    .into()
}

fn topology_stage_node<'a>(
    snapshot: &TrafficTopologySnapshot,
    stage: TrafficTopologyStage,
    glyph: Icon,
    fallback_label: &'static str,
    badge_kind: BadgeKind,
    color_fn: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let (label, detail, badge_label) = snapshot
        .node(stage)
        .map(|node| {
            let badge = if stage == TrafficTopologyStage::Sniffer {
                match snapshot.sniffer_enabled {
                    Some(true) => "On".to_owned(),
                    Some(false) => "Off".to_owned(),
                    None => "—".to_owned(),
                }
            } else if node.active_connections > 0 {
                format!("{} conns", node.active_connections)
            } else {
                "idle".to_owned()
            };
            (node.label.clone(), node.detail.clone(), badge)
        })
        .unwrap_or_else(|| {
            (
                fallback_label.to_owned(),
                snapshot
                    .failure
                    .clone()
                    .unwrap_or_else(|| "not available".to_owned()),
                "—".to_owned(),
            )
        });
    let node = topology_node_box(glyph, label, detail, badge_label, badge_kind, color_fn);
    let Some(route) = topology_route_for_stage(stage) else {
        return node;
    };
    if !snapshot.is_drawable() {
        return node;
    }
    button(node)
        .width(Length::Fill)
        .padding(0)
        .on_press(Message::Navigate(route))
        .into()
}

fn topology_route_for_stage(stage: TrafficTopologyStage) -> Option<Route> {
    match TrafficTopologyNavigationApplication::page_for_stage(stage)? {
        infiltrator_contract::surface_snapshot::PageId::Settings => Some(Route::Settings),
        infiltrator_contract::surface_snapshot::PageId::Rules => Some(Route::Rules),
        infiltrator_contract::surface_snapshot::PageId::Proxies => Some(Route::Proxies),
        _ => None,
    }
}

fn topology_badge(snapshot: &TrafficTopologySnapshot, lang: &Lang<'_>) -> String {
    match snapshot.status {
        TrafficTopologyStatus::Ready => format!(
            "{} {} · flowing",
            snapshot.active_connections,
            lang.tr("overview_conn_unit")
        ),
        TrafficTopologyStatus::Empty => format!("0 {} · idle", lang.tr("overview_conn_unit")),
        TrafficTopologyStatus::Unknown => "topology pending".to_owned(),
        TrafficTopologyStatus::Unsupported => "topology unavailable".to_owned(),
        TrafficTopologyStatus::Failed => "topology read failed".to_owned(),
    }
}

fn topology_badge_kind(snapshot: &TrafficTopologySnapshot) -> BadgeKind {
    match snapshot.status {
        TrafficTopologyStatus::Ready => BadgeKind::Success,
        TrafficTopologyStatus::Empty | TrafficTopologyStatus::Unknown => BadgeKind::Neutral,
        TrafficTopologyStatus::Unsupported => BadgeKind::Warning,
        TrafficTopologyStatus::Failed => BadgeKind::Danger,
    }
}

fn topology_node_box<'a>(
    glyph: Icon,
    stage_name: String,
    chip_label: String,
    badge_label: String,
    badge_kind: BadgeKind,
    color_fn: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let icon_part = container(icon_themed(glyph, 14.0, color_fn))
        .width(26)
        .height(26)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |t: &Theme| {
            let c = color_fn(t);
            container::Style {
                background: Some(Color { a: 0.14, ..c }.into()),
                border: Border {
                    radius: border::Radius::from(R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    let top_row = row![
        icon_part,
        Space::new().width(theme::SP_XS),
        text(stage_name)
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(Length::Fill),
        badge(badge_label, badge_kind),
    ]
    .align_y(Alignment::Center);

    let bottom_row = row![colored_flow_chip(chip_label, color_fn)].align_y(Alignment::Center);

    container(
        column![top_row, Space::new().height(theme::SP_XS), bottom_row]
            .spacing(theme::SP_XS)
            .width(Length::Fill),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_MD)
    .style(row_card_surface)
    .into()
}

fn colored_flow_chip<'a>(
    label: String,
    color_fn: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    container(
        text(label)
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(move |t: &Theme| text::Style {
                color: Some(color_fn(t)),
            }),
    )
    .padding([3, 10])
    .style(move |t: &Theme| {
        let c = color_fn(t);
        container::Style {
            background: Some(Color { a: 0.12, ..c }.into()),
            border: Border {
                radius: border::Radius::from(R_CHIP),
                width: 1.0,
                color: Color { a: 0.25, ..c },
            },
            ..Default::default()
        }
    })
    .into()
}

fn arrow_connector<'a>() -> Element<'a, Message> {
    container(icon_themed(Icon::ChevronRight, 16.0, |t: &Theme| {
        tokens(t).text_tertiary
    }))
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

// ---------------------------------------------------------------------------
// Current IP & Multi-Source Probe Card (P02-01 ~ P02-03)
// ---------------------------------------------------------------------------

fn current_ip_card<'a>(state: &'a AppState, lang: &Lang<'a>, _is_en: bool) -> Element<'a, Message> {
    let public_ip_str = state
        .diag
        .public_ip
        .as_deref()
        .unwrap_or(if state.shell.demo {
            "203.0.113.7"
        } else {
            "—"
        });
    let provider_name = state
        .diag
        .public_ip_provider
        .as_deref()
        .unwrap_or("ipapi.is");

    let copy_msg = Message::ShowToast(
        infiltrator_shared::i18n_interpolator::interpolate(
            &lang.tr("overview_copied_ip"),
            &[("ip", public_ip_str)],
        ),
        ToastStatus::Success,
    );

    let copy_btn = button(
        row![
            icon_themed(Icon::Copy, 12.0, |t| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("common_copy").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(copy_msg);

    let card_header = row![
        row![
            icon_themed(Icon::Globe, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("overview_current_ip").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        chip(provider_name),
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 14.0, Message::FetchIpInfo),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let ip_readout_row = row![
        text(public_ip_str)
            .size(20)
            .font(MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        copy_btn,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let meta_text: Element<'a, Message> = if let Some(err) = state.diag.public_ip_error.as_deref() {
        text(format!("{}: {err}", lang.tr("overview_probe_failed")))
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).danger),
            })
            .into()
    } else if let Some(checked_at) = state.diag.public_ip_checked_at.as_deref() {
        text(format!(
            "{} · {provider_name} · {checked_at}",
            lang.tr("overview_via_current_proxy")
        ))
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
        .into()
    } else {
        text(lang.tr("overview_probe_source_desc").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            })
            .into()
    };

    container(
        column![
            card_header,
            Space::new().height(theme::SP_MD),
            ip_readout_row,
            Space::new().height(theme::SP_XS),
            meta_text,
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_XXL)
    .style(card_surface)
    .into()
}

// ---------------------------------------------------------------------------
// Multi-Target Latency Comparison Bars (P02-08 ~ P02-10)
// ---------------------------------------------------------------------------

fn latency_card<'a>(_state: &'a AppState, lang: &Lang<'a>, _is_en: bool) -> Element<'a, Message> {
    let avg_pill = container(
        row![
            icon_themed(Icon::Activity, 11.0, |t| tokens(t).success),
            Space::new().width(theme::SP_XS),
            text(lang.tr("overview_avg_latency").to_string())
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success)
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([3, 9])
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(
                Color {
                    a: 0.12,
                    ..tk.success
                }
                .into(),
            ),
            border: Border {
                radius: border::Radius::from(R_CHIP),
                width: 1.0,
                color: Color {
                    a: 0.25,
                    ..tk.success
                },
            },
            ..Default::default()
        }
    });

    let card_header = row![
        row![
            icon_themed(Icon::Target, 16.0, |t: &Theme| tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("runtime_delay_title").to_string())
                .size(14)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        avg_pill,
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let bars = column![
        latency_comparison_bar("Google", 180, 300.0),
        latency_comparison_bar("Cloudflare", 178, 300.0),
        latency_comparison_bar("GitHub", 182, 300.0),
    ]
    .spacing(theme::SP_SM)
    .width(Length::Fill);

    container(column![card_header, Space::new().height(theme::SP_MD), bars].spacing(theme::SP_XS))
        .width(Length::FillPortion(1))
        .padding(theme::SP_XXL)
        .style(card_surface)
        .into()
}

fn latency_comparison_bar<'a>(name: &'static str, ms: u32, max_ms: f32) -> Element<'a, Message> {
    let fill_pct = (ms as f32 / max_ms).clamp(0.05, 1.0);
    let fill_portion = (fill_pct * 100.0) as u16;
    let empty_portion = (100 - fill_portion).max(1);

    let bar = container(
        row![
            container(
                Space::new()
                    .width(Length::FillPortion(fill_portion))
                    .height(6)
            )
            .style(move |t: &Theme| {
                let c = theme::latency_color(tokens(t), Some(ms));
                container::Style {
                    background: Some(c.into()),
                    border: Border {
                        radius: border::Radius::from(3.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
            Space::new().width(Length::FillPortion(empty_portion)),
        ]
        .width(Length::Fill)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(6)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border {
                radius: border::Radius::from(3.0),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    row![
        text(name)
            .size(12)
            .font(FONT_MEDIUM)
            .width(Length::Fixed(80.0))
            .style(|t: &Theme| {
                text::Style {
                    color: Some(tokens(t).text_primary),
                }
            }),
        bar,
        Space::new().width(theme::SP_MD),
        text(format!("{ms} ms"))
            .size(12)
            .font(MONO)
            .width(Length::Fixed(55.0))
            .style(move |t: &Theme| {
                text::Style {
                    color: Some(theme::latency_color(tokens(t), Some(ms))),
                }
            }),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topology_badge_reflects_the_shared_status_and_count() {
        let mut snapshot = TrafficTopologySnapshot::demo_fixture();
        snapshot.active_connections = 3;
        assert_eq!(
            topology_badge(&snapshot, &Lang("zh-CN")),
            "3 连接 · flowing"
        );

        snapshot.status = TrafficTopologyStatus::Empty;
        snapshot.active_connections = 0;
        assert_eq!(topology_badge(&snapshot, &Lang("zh-CN")), "0 连接 · idle");

        snapshot.status = TrafficTopologyStatus::Unsupported;
        assert_eq!(
            topology_badge(&snapshot, &Lang("zh-CN")),
            "topology unavailable"
        );
    }

    #[test]
    fn topology_stage_routes_follow_the_shared_application_mapping() {
        assert_eq!(
            topology_route_for_stage(TrafficTopologyStage::Inbound),
            Some(Route::Settings)
        );
        assert_eq!(
            topology_route_for_stage(TrafficTopologyStage::Sniffer),
            Some(Route::Settings)
        );
        assert_eq!(
            topology_route_for_stage(TrafficTopologyStage::RuleSet),
            Some(Route::Rules)
        );
        assert_eq!(
            topology_route_for_stage(TrafficTopologyStage::ProxyGroup),
            Some(Route::Proxies)
        );
        assert_eq!(
            topology_route_for_stage(TrafficTopologyStage::Outbound),
            Some(Route::Proxies)
        );
    }

    #[test]
    fn overview_speedtest_button_renders_when_idle_and_testing() {
        let mut state = AppState::empty();
        let lang = Lang("zh-CN");
        let _btn_idle = overview_speedtest_button(&state, &lang);

        state.runtime.runtime_testing_all_delays = true;
        let _btn_testing = overview_speedtest_button(&state, &lang);
    }
}
