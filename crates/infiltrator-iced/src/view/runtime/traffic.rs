//! Runtime page traffic section: KPI metrics tiles, real-time traffic chart
//! with smooth curves and legends, time range selector pills, and multi-dimension
//! traffic breakdown & rankings (Domains, Devices, Proxies, Processes).

use std::collections::{HashMap, VecDeque};

use iced::widget::{Space, button, canvas, column, container, row, text};
use iced::{
    Alignment, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme, border, mouse,
};
use infiltrator_shared::locales::{Lang, Localizer};

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStreamState;
use crate::utils::format_bytes;
use crate::view::components::{
    BadgeKind, badge, card, empty_state, row_card_surface, section_header, segmented_control,
    stat_card,
};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, SP_LG, SP_MD, SP_SM, SP_XS, tokens};

/// Multi-dimension breakdown category for traffic usage analytics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrafficDimension {
    #[default]
    Domains,
    Devices,
    Proxies,
    Processes,
}

impl TrafficDimension {
    pub fn label(self, lang: &Lang<'_>) -> String {
        match self {
            Self::Domains => lang.tr("traffic_dim_domains").to_string(),
            Self::Devices => lang.tr("traffic_dim_devices").to_string(),
            Self::Proxies => lang.tr("traffic_dim_proxies").to_string(),
            Self::Processes => lang.tr("traffic_dim_processes").to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn icon(self) -> Icon {
        match self {
            Self::Domains => Icon::Globe,
            Self::Devices => Icon::Server,
            Self::Proxies => Icon::Zap,
            Self::Processes => Icon::Code2,
        }
    }
}

/// Extract clean executable/binary name from a system process path.
pub fn extract_process_name(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let filename = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
    let name = filename.strip_suffix(".exe").unwrap_or(filename);
    name.to_string()
}

/// Traffic ranking summary entry for any dimension (domain, device, proxy, process).
#[derive(Debug, Clone, PartialEq)]
pub struct HostTrafficRank {
    pub host: String,
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub share_percent: f64,
}

pub(super) fn traffic_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let theme_tokens = tokens(&state.shell.theme);
    let _is_zh = !lang.0.starts_with("en");

    // 1. KPI metrics tiles: Upload speed, Download speed, Memory usage, Public exit IP.
    let up_rate = state
        .diag
        .traffic
        .as_ref()
        .map(|t| format!("{}/s", format_bytes(t.up)))
        .unwrap_or_else(|| "—".to_string());
    let down_rate = state
        .diag
        .traffic
        .as_ref()
        .map(|t| format!("{}/s", format_bytes(t.down)))
        .unwrap_or_else(|| "—".to_string());
    let mem_usage = state
        .diag
        .memory
        .as_ref()
        .map(|m| format_bytes(m.in_use))
        .unwrap_or_else(|| "—".to_string());

    let up_stat = stat_card(
        Icon::ArrowUp,
        lang.tr("runtime_stat_up").as_ref(),
        &up_rate,
        theme_tokens.success,
        false,
    );
    let down_stat = stat_card(
        Icon::ArrowDown,
        lang.tr("runtime_stat_down").as_ref(),
        &down_rate,
        theme_tokens.accent,
        false,
    );
    let mem_stat = stat_card(
        Icon::Server,
        lang.tr("runtime_stat_memory").as_ref(),
        &mem_usage,
        theme_tokens.warning,
        false,
    );
    let ip_card = public_ip_tile(state, &lang, theme_tokens.accent);

    let kpi_row = row![up_stat, down_stat, mem_stat, ip_card]
        .spacing(SP_MD)
        .width(Length::Fill);

    // 2. Real-time speeds, peaks, and time range selector pills ("1小时", "24小时", "7天", "30天")
    let cur_up = state.diag.traffic.as_ref().map(|t| t.up).unwrap_or(0);
    let cur_down = state.diag.traffic.as_ref().map(|t| t.down).unwrap_or(0);
    let peak_up = state.diag.traffic_history.iter().map(|(u, _)| *u).max().unwrap_or(0);
    let peak_down = state.diag.traffic_history.iter().map(|(_, d)| *d).max().unwrap_or(0);

    let realtime_up_badge = badge(format!("↑ {}/s", format_bytes(cur_up)), BadgeKind::Success);
    let realtime_down_badge = badge(format!("↓ {}/s", format_bytes(cur_down)), BadgeKind::Accent);

    let time_range_labels = vec![
        lang.tr("traffic_1h").to_string(),
        lang.tr("traffic_24h").to_string(),
        lang.tr("traffic_7d").to_string(),
        lang.tr("traffic_30d").to_string(),
    ];
    let time_range_pills = segmented_control(&time_range_labels, 0, |_| Message::Noop);

    let waiting_note: Element<'a, Message> = if state.diag.traffic.is_none() {
        text(lang.tr("waiting_traffic").to_string())
            .size(11)
            .style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) })
            .into()
    } else {
        Space::new().width(0).into()
    };

    let peak_indicator: Element<'a, Message> = if peak_up > 0 || peak_down > 0 {
        badge(
            format!(
                "{}: ↑ {}/s · ↓ {}/s",
                lang.tr("traffic_peak").as_ref(),
                format_bytes(peak_up),
                format_bytes(peak_down)
            ),
            BadgeKind::Neutral,
        )
    } else {
        Space::new().width(0).into()
    };

    let legends_row = row![
        legend_indicator(
            theme_tokens.success,
            lang.tr("runtime_stat_up").as_ref(),
            &format!("{}/s", format_bytes(cur_up)),
            (peak_up > 0).then(|| format!("{}: {}/s", lang.tr("traffic_peak").as_ref(), format_bytes(peak_up))),
        ),
        Space::new().width(SP_LG),
        legend_indicator(
            theme_tokens.accent,
            lang.tr("runtime_stat_down").as_ref(),
            &format!("{}/s", format_bytes(cur_down)),
            (peak_down > 0).then(|| format!("{}: {}/s", lang.tr("traffic_peak").as_ref(), format_bytes(peak_down))),
        ),
        Space::new().width(SP_MD),
        peak_indicator,
        Space::new().width(Length::Fill),
        waiting_note,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let chart_canvas = canvas::Canvas::new(SmoothTrafficChart {
        history: state.diag.traffic_history.clone(),
    })
    .width(Length::Fill)
    .height(Length::Fixed(140.0));

    let chart_card = card(
        None,
        column![
            section_header(
                lang.tr("traffic_trend_title").as_ref(),
                Some(
                    row![
                        realtime_up_badge,
                        Space::new().width(theme::SP_XS),
                        realtime_down_badge,
                        Space::new().width(theme::SP_MD),
                        time_range_pills,
                        Space::new().width(theme::SP_MD),
                        stream_badge(&state.diag.traffic_stream_state, &lang),
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                ),
            ),
            Space::new().height(theme::SP_SM),
            legends_row,
            Space::new().height(theme::SP_SM),
            chart_canvas,
        ]
        .spacing(SP_XS),
    );

    // 3. Multi-Dimension Dimension Selector & Rankings List
    let dim_labels = vec![
        TrafficDimension::Domains.label(&lang),
        TrafficDimension::Devices.label(&lang),
        TrafficDimension::Proxies.label(&lang),
        TrafficDimension::Processes.label(&lang),
    ];
    let dim_selector = segmented_control(&dim_labels, 0, |_| Message::Noop);

    let host_ranks = compute_host_rankings(state);
    let rankings_card = render_domain_rankings(&host_ranks, dim_selector, &lang);

    column![kpi_row, chart_card, rankings_card]
        .spacing(SP_LG)
        .width(Length::Fill)
        .into()
}

/// Specialized public exit IP tile matching KPI stat card visual styling.
fn public_ip_tile<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
    accent: Color,
) -> Element<'a, Message> {
    let _is_zh = !lang.0.starts_with("en");
    let icon_chip = container(crate::view::svg_icons::icon(
        Icon::Globe,
        20.0,
        Color { a: 0.9, ..accent },
    ))
    .width(40)
    .height(40)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(move |_theme: &Theme| container::Style {
        background: Some(Color { a: 0.14, ..accent }.into()),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
        ..Default::default()
    });

    let ip_text = state.diag.public_ip.as_deref().unwrap_or("—");
    let probe_btn = button(text(lang.tr("traffic_probe_btn").to_string()).size(10).font(theme::FONT_MEDIUM))
        .padding([2, 6])
        .style(iced::widget::button::secondary)
        .on_press(Message::FetchIpInfo);

    let sub_note = match (
        state.diag.public_ip_provider.as_deref(),
        state.diag.public_ip_checked_at.as_deref(),
        state.diag.public_ip_error.as_deref(),
    ) {
        (Some(provider), Some(checked_at), _) => text(format!("{provider} · {checked_at}")),
        (_, _, Some(error)) => text(format!("Error: {error}")),
        _ => text(lang.tr("traffic_probe_tooltip").to_string()),
    }
    .size(10)
    .font(MONO)
    .style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) });

    let texts = column![
        row![
            text(lang.tr("runtime_stat_public_ip").to_string())
                .size(11)
                .font(theme::FONT_MEDIUM)
                .style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().width(Length::Fill),
            probe_btn,
        ]
        .align_y(Alignment::Center),
        text(ip_text.to_string())
            .size(16)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
        sub_note,
    ]
    .spacing(2)
    .width(Length::Fill);

    container(row![icon_chip, texts].spacing(theme::SP_MD).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding(theme::SP_LG)
        .style(move |t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border { radius: border::Radius::from(theme::R_CARD), width: 1.0, color: tk.card_border },
                shadow: tk.card_shadow,
                ..Default::default()
            }
        })
        .into()
}

/// Legend item showing colored bullet, label, current speed and optional peak.
fn legend_indicator<'a>(
    color: Color,
    label: &str,
    current_speed: &str,
    peak_speed: Option<String>,
) -> Element<'a, Message> {
    let bullet = container(Space::new().width(8).height(8)).style(move |_t: &Theme| container::Style {
        background: Some(color.into()),
        border: Border { radius: border::Radius::from(4.0), ..Default::default() },
        ..Default::default()
    });

    let mut content = row![
        bullet,
        Space::new().width(theme::SP_XS),
        text(label.to_string())
            .size(11)
            .font(theme::FONT_MEDIUM)
            .style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
        Space::new().width(theme::SP_XS),
        text(current_speed.to_string())
            .size(11)
            .font(MONO)
            .style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
    ]
    .align_y(Alignment::Center);

    if let Some(peak) = peak_speed {
        content = content.push(Space::new().width(theme::SP_XS)).push(
            text(format!("({peak})"))
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style { color: Some(tokens(t).text_tertiary) }),
        );
    }

    content.into()
}

/// Helper to convert aggregated upload/download counts into sorted rankings with share percentages.
fn summarize_and_sort_rankings(map: HashMap<String, (u64, u64)>, limit: usize) -> Vec<HostTrafficRank> {
    let grand_total: u64 = map.values().map(|(u, d)| u.saturating_add(*d)).sum();

    let mut ranks: Vec<HostTrafficRank> = map
        .into_iter()
        .map(|(host, (upload, download))| {
            let total = upload.saturating_add(download);
            let share_percent = if grand_total > 0 {
                (total as f64 / grand_total as f64) * 100.0
            } else {
                0.0
            };
            HostTrafficRank {
                host,
                upload,
                download,
                total,
                share_percent,
            }
        })
        .collect();

    ranks.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.host.cmp(&b.host)));
    ranks.truncate(limit);
    ranks
}

/// Compute traffic rankings for any given dimension from live connection snapshot.
pub fn compute_dimension_rankings(
    state: &AppState,
    dimension: TrafficDimension,
) -> Vec<HostTrafficRank> {
    let mut map: HashMap<String, (u64, u64)> = HashMap::new();
    if let Some(snapshot) = &state.diag.connections {
        for conn in &snapshot.connections {
            let key = match dimension {
                TrafficDimension::Domains => {
                    if !conn.metadata.host.trim().is_empty() {
                        conn.metadata.host.trim().to_string()
                    } else if !conn.metadata.destination_ip.trim().is_empty() {
                        conn.metadata.destination_ip.trim().to_string()
                    } else {
                        continue;
                    }
                }
                TrafficDimension::Devices => {
                    let src = conn.metadata.source_ip.trim();
                    if src.is_empty() || src == "127.0.0.1" || src == "::1" {
                        "127.0.0.1 (Localhost)".to_string()
                    } else {
                        src.to_string()
                    }
                }
                TrafficDimension::Proxies => conn
                    .chains
                    .first()
                    .cloned()
                    .filter(|c| !c.trim().is_empty())
                    .unwrap_or_else(|| {
                        if !conn.rule_payload.trim().is_empty() {
                            conn.rule_payload.trim().to_string()
                        } else if !conn.rule.trim().is_empty() {
                            conn.rule.trim().to_string()
                        } else {
                            "DIRECT".to_string()
                        }
                    }),
                TrafficDimension::Processes => {
                    let p = extract_process_name(&conn.metadata.process_path);
                    if p.is_empty() { "[System / Unknown]".to_string() } else { p }
                }
            };
            let entry = map.entry(key).or_insert((0, 0));
            entry.0 = entry.0.saturating_add(conn.upload);
            entry.1 = entry.1.saturating_add(conn.download);
        }
    }
    summarize_and_sort_rankings(map, 8)
}

/// Compute top domain / host traffic rankings from live connection snapshot.
pub fn compute_host_rankings(state: &AppState) -> Vec<HostTrafficRank> {
    compute_dimension_rankings(state, TrafficDimension::Domains)
}

/// Compute top device / client IP traffic rankings from live connection snapshot.
#[allow(dead_code)]
pub fn compute_device_rankings(state: &AppState) -> Vec<HostTrafficRank> {
    compute_dimension_rankings(state, TrafficDimension::Devices)
}

/// Compute top proxy outbound node traffic rankings from live connection snapshot.
#[allow(dead_code)]
pub fn compute_proxy_rankings(state: &AppState) -> Vec<HostTrafficRank> {
    compute_dimension_rankings(state, TrafficDimension::Proxies)
}

/// Compute top process / application traffic rankings from live connection snapshot.
#[allow(dead_code)]
pub fn compute_process_rankings(state: &AppState) -> Vec<HostTrafficRank> {
    compute_dimension_rankings(state, TrafficDimension::Processes)
}

/// Render the domain / multi-dimension traffic rankings card.
fn render_domain_rankings<'a>(
    ranks: &[HostTrafficRank],
    dim_selector: Element<'a, Message>,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    let _is_zh = !lang.0.starts_with("en");
    let header_title = lang.tr("traffic_domain_rank");

    if ranks.is_empty() {
        return card(
            None,
            column![
                section_header(
                    &header_title,
                    Some(
                        row![
                            dim_selector,
                            Space::new().width(theme::SP_SM),
                            badge(lang.tr("traffic_multidim_analysis").to_string(), BadgeKind::Neutral),
                        ]
                        .align_y(Alignment::Center)
                        .into()
                    ),
                ),
                Space::new().height(theme::SP_SM),
                empty_state(
                    Icon::Globe,
                    lang.tr("traffic_no_stats").as_ref(),
lang.tr("traffic_no_stats_desc").as_ref(),
                ),
            ],
        );
    }

    let mut list = column![].spacing(SP_SM);
    for (i, rank) in ranks.iter().enumerate() {
        let rank_num = i + 1;
        let rank_kind = match rank_num {
            1 => BadgeKind::Accent,
            2 | 3 => BadgeKind::Neutral,
            _ => BadgeKind::Neutral,
        };

        let rank_badge = badge(format!("#{rank_num}"), rank_kind);
        let progress_bar = share_bar(rank.share_percent);
        let total_label = lang.tr("traffic_total");

        let row_content = column![
            row![
                rank_badge,
                Space::new().width(theme::SP_SM),
                text(rank.host.clone())
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .width(Length::Fill)
                    .style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
                text(format!("↑ {} / ↓ {}", format_bytes(rank.upload), format_bytes(rank.download)))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                Space::new().width(theme::SP_MD),
                text(format!("{total_label} {}", format_bytes(rank.total)))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
                Space::new().width(theme::SP_MD),
                text(format!("{:.1}%", rank.share_percent))
                    .size(11)
                    .font(MONO)
                    .style(|t: &Theme| text::Style { color: Some(tokens(t).text_primary) }),
            ]
            .align_y(Alignment::Center),
            Space::new().height(SP_XS),
            progress_bar,
        ]
        .spacing(2);

        list = list.push(
            container(row_content)
                .padding([theme::SP_SM, SP_MD])
                .width(Length::Fill)
                .style(row_card_surface),
        );
    }

    card(
        None,
        column![
            section_header(
                &header_title,
                Some(
                    row![
                        dim_selector,
                        Space::new().width(theme::SP_MD),
                        badge(format!("Top {}", ranks.len()), BadgeKind::Neutral),
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                ),
            ),
            Space::new().height(theme::SP_MD),
            list,
        ],
    )
}

/// Visual horizontal share proportion bar.
pub fn share_bar<'a, Message: 'a>(percent: f64) -> Element<'a, Message> {
    const BAR_HEIGHT: f32 = 4.0;
    let clamped_ratio = (percent as f32 / 100.0).clamp(0.005, 1.0);

    container(
        row![
            container(Space::new().height(Length::Fixed(BAR_HEIGHT)))
                .width(Length::FillPortion((clamped_ratio * 1000.0).max(1.0) as u16))
                .style(|t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(tk.accent.into()),
                        border: Border { radius: border::Radius::from(2.0), ..Default::default() },
                        ..Default::default()
                    }
                }),
            container(Space::new().height(Length::Fixed(BAR_HEIGHT)))
                .width(Length::FillPortion(((1.0 - clamped_ratio) * 1000.0).max(1.0) as u16))
                .style(|_t: &Theme| container::Style { background: None, ..Default::default() }),
        ]
        .width(Length::Fill)
        .height(Length::Fixed(BAR_HEIGHT)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(BAR_HEIGHT))
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.chip_bg.into()),
            border: Border { radius: border::Radius::from(2.0), ..Default::default() },
            ..Default::default()
        }
    })
    .into()
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

// ---------------------------------------------------------------------------
// Smooth Curves Traffic Chart Canvas
// ---------------------------------------------------------------------------

pub struct SmoothTrafficChart {
    pub history: VecDeque<(u64, u64)>,
}

impl<Message> canvas::Program<Message> for SmoothTrafficChart {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tk = tokens(theme);
        let accent = tk.accent;
        let success = tk.success;

        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (width, height) = (bounds.width, bounds.height);

        if self.history.len() < 2 {
            let baseline = canvas::Path::line(
                Point::new(0.0, height - 10.0),
                Point::new(width, height - 10.0),
            );
            frame.stroke(
                &baseline,
                canvas::Stroke::default()
                    .with_color(Color { a: 0.15, ..tk.text_tertiary })
                    .with_width(1.0),
            );
            return vec![frame.into_geometry()];
        }

        let max_points: usize = 60;
        let x_step = width / (max_points - 1) as f32;
        let mut max_speed = self
            .history
            .iter()
            .map(|(u, d)| (*u).max(*d))
            .max()
            .unwrap_or(1024 * 100);

        if max_speed < 1024 * 100 {
            max_speed = 1024 * 100;
        }
        let effective_max = (max_speed as f64 * 1.15) as u64;

        let top_pad = 8.0_f32;
        let bottom_pad = 6.0_f32;
        let usable_h = (height - top_pad - bottom_pad).max(10.0);

        let scale_y = |speed: u64| -> f32 {
            let ratio = (speed as f32 / effective_max as f32).clamp(0.0, 1.0);
            height - bottom_pad - (ratio * usable_h)
        };

        // Draw 3 subtle horizontal reference grid lines
        for fraction in [0.25_f32, 0.50, 0.75] {
            let y = height - bottom_pad - (fraction * usable_h);
            let grid_line = canvas::Path::line(Point::new(0.0, y), Point::new(width, y));
            frame.stroke(
                &grid_line,
                canvas::Stroke::default()
                    .with_color(Color { a: 0.08, ..tk.divider })
                    .with_width(1.0),
            );
        }

        let start_offset = max_points.saturating_sub(self.history.len());

        let down_pts: Vec<Point> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, (_, down))| Point::new((start_offset + i) as f32 * x_step, scale_y(*down)))
            .collect();

        let up_pts: Vec<Point> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, (up, _))| Point::new((start_offset + i) as f32 * x_step, scale_y(*up)))
            .collect();

        // 1. Download area & smooth curve
        let down_area = build_smooth_area(&down_pts, height);
        frame.fill(&down_area, Color { a: 0.12, ..accent });
        let down_curve = build_smooth_path(&down_pts);
        frame.stroke(&down_curve, canvas::Stroke::default().with_color(accent).with_width(2.5));

        if let Some(&last_pt) = down_pts.last() {
            frame.fill(&canvas::Path::circle(last_pt, 6.0), Color { a: 0.20, ..accent });
            frame.fill(&canvas::Path::circle(last_pt, 3.8), accent);
            frame.fill(&canvas::Path::circle(last_pt, 1.8), Color::WHITE);
        }

        // 2. Upload area & smooth curve
        let up_area = build_smooth_area(&up_pts, height);
        frame.fill(&up_area, Color { a: 0.08, ..success });
        let up_curve = build_smooth_path(&up_pts);
        frame.stroke(&up_curve, canvas::Stroke::default().with_color(success).with_width(2.0));

        if let Some(&last_pt) = up_pts.last() {
            frame.fill(&canvas::Path::circle(last_pt, 5.0), Color { a: 0.20, ..success });
            frame.fill(&canvas::Path::circle(last_pt, 3.2), success);
            frame.fill(&canvas::Path::circle(last_pt, 1.6), Color::WHITE);
        }

        vec![frame.into_geometry()]
    }
}

/// Helper to add Catmull-Rom cubic Bezier spline segments to a path builder.
fn add_smooth_curves(builder: &mut canvas::path::Builder, points: &[Point]) {
    if points.len() < 2 {
        return;
    }
    if points.len() == 2 {
        builder.line_to(points[1]);
        return;
    }
    let n = points.len();
    for i in 0..n - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < n { points[i + 2] } else { points[i + 1] };

        let cp1 = Point::new(p1.x + (p2.x - p0.x) / 6.0, p1.y + (p2.y - p0.y) / 6.0);
        let cp2 = Point::new(p2.x - (p3.x - p1.x) / 6.0, p2.y - (p3.y - p1.y) / 6.0);
        builder.bezier_curve_to(cp1, cp2, p2);
    }
}

/// Construct smooth Catmull-Rom cubic Bezier path through discrete data points.
pub fn build_smooth_path(points: &[Point]) -> canvas::Path {
    canvas::Path::new(|builder| {
        if let Some(&first) = points.first() {
            builder.move_to(first);
            add_smooth_curves(builder, points);
        }
    })
}

/// Construct smooth closed area under the Catmull-Rom cubic Bezier curve.
pub fn build_smooth_area(points: &[Point], height: f32) -> canvas::Path {
    canvas::Path::new(|builder| {
        if let (Some(&first), Some(&last)) = (points.first(), points.last()) {
            builder.move_to(Point::new(first.x, height));
            builder.line_to(first);
            add_smooth_curves(builder, points);
            builder.line_to(Point::new(last.x, height));
            builder.close();
        }
    })
}

#[cfg(test)]
#[path = "traffic_tests.rs"]
mod tests;
