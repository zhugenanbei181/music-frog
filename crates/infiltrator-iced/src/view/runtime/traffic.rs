//! Runtime page traffic section: KPI metrics tiles, the real-time rates chart,
//! and the traffic audit - one owner's buckets, read through the reader's own
//! range and dimension selection (TRAFFICAUDIT-001). The section folds no
//! ranking of its own: it renders the readback the application answers with.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::runtime::traffic_legend::legend_indicator;
use infiltrator_contract::design_resources::TextRole;
use infiltrator_domain::traffic_audit_readback::{TrafficAuditReadback, TrafficAuditRow};
use infiltrator_presentation::traffic_audit_options::{
    TRAFFIC_AUDIT_DIMENSIONS, TRAFFIC_AUDIT_WINDOWS, traffic_audit_dimension_at,
    traffic_audit_dimension_index, traffic_audit_dimension_label_key, traffic_audit_window_at,
    traffic_audit_window_index, traffic_audit_window_label_key,
};
use infiltrator_shared::byte_units::format_bytes;
use zhuge_ui_core::curve::catmull_rom_windows;

use crate::view::component_card::card;
use crate::view::component_forms::row_card_surface;
use crate::view::components::{
    BadgeKind, badge, empty_state, section_header, segmented_control, stat_card,
};
use crate::view::svg_icons::{Icon, icon};
use crate::view::theme;
use crate::view::theme::{SP_LG, SP_MD, SP_SM, SP_XS, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use canvas::path::Builder;
use iced::widget::{Space, button, canvas, column, container, row, text};
use iced::{
    Alignment, Border, Color, Element, Length, Point, Rectangle, Renderer, Theme, border, mouse,
};
use infiltrator_contract::resources::{CoreResourceSnapshot, ResourceAvailability};
use infiltrator_presentation::traffic_readout::TrafficFreshness;
use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::VecDeque;

pub(crate) fn traffic_section<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let theme_tokens = tokens(&state.shell.theme);
    let _is_zh = !lang.0.starts_with("en");

    // 1. KPI metrics tiles: Upload speed, Download speed, Memory usage, Public exit IP.
    let rates = state.traffic_readout();
    let observation_badge = badge(
        rates.status.clone(),
        match rates.freshness {
            TrafficFreshness::Failed => BadgeKind::Danger,
            TrafficFreshness::Current => BadgeKind::Success,
            TrafficFreshness::Stale | TrafficFreshness::Unobserved => BadgeKind::Neutral,
        },
    );
    let up_rate = &rates.upload;
    let down_rate = &rates.download;
    let mem_usage = match &state.runtime.core_resources {
        CoreResourceSnapshot::Observed {
            memory: ResourceAvailability::Available(memory),
            ..
        } => format_bytes(*memory),
        CoreResourceSnapshot::NotObserved | CoreResourceSnapshot::Observed { .. } => {
            "—".to_string()
        }
    };

    let up_stat = stat_card(
        Icon::ArrowUp,
        lang.tr("runtime_stat_up").as_ref(),
        up_rate,
        theme_tokens.success,
        false,
    );
    let down_stat = stat_card(
        Icon::ArrowDown,
        lang.tr("runtime_stat_down").as_ref(),
        down_rate,
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
    let realtime_up_badge = badge(
        format!("↑ {}", rates.upload),
        match rates.freshness {
            TrafficFreshness::Current => BadgeKind::Success,
            TrafficFreshness::Failed | TrafficFreshness::Stale | TrafficFreshness::Unobserved => {
                BadgeKind::Neutral
            }
        },
    );
    let realtime_down_badge = badge(
        format!("↓ {}", rates.download),
        match rates.freshness {
            TrafficFreshness::Current => BadgeKind::Accent,
            TrafficFreshness::Failed | TrafficFreshness::Stale | TrafficFreshness::Unobserved => {
                BadgeKind::Neutral
            }
        },
    );
    let waiting_note: Element<'a, Message> = match rates.freshness {
        TrafficFreshness::Failed => {
            container(theme::role_body_text(rates.failure).width(Length::Fill))
                .id(InteractionRegion::TrafficFailure.id())
                .width(Length::Fill)
                .into()
        }
        TrafficFreshness::Unobserved => {
            theme::role_body_text(lang.tr("waiting_traffic").to_string())
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                })
                .into()
        }
        TrafficFreshness::Current | TrafficFreshness::Stale => Space::new().width(0).into(),
    };

    let legends_row = row![
        legend_indicator(
            theme_tokens.success,
            lang.tr("runtime_stat_up").as_ref(),
            &rates.upload,
            rates
                .upload_peak
                .as_ref()
                .map(|peak| format!("{}: {}", lang.tr("traffic_peak"), peak))
        ),
        legend_indicator(
            theme_tokens.accent,
            lang.tr("runtime_stat_down").as_ref(),
            &rates.download,
            rates.download_peak.as_ref().map(|peak| format!(
                "{}: {}",
                lang.tr("traffic_peak"),
                peak
            ))
        ),
    ]
    .spacing(SP_MD)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .wrap();
    let chart_canvas = canvas::Canvas::new(SmoothTrafficChart {
        history: state.diag.traffic_history.clone(),
    })
    .width(Length::Fill)
    .height(Length::Fixed(140.0));

    let chart_card = card(
        None,
        column![
            section_header(lang.tr("traffic_trend_title").as_ref(), None),
            row![realtime_up_badge, realtime_down_badge, observation_badge]
                .spacing(SP_SM)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .wrap(),
            Space::new().height(theme::SP_SM),
            container(legends_row).id(InteractionRegion::TrafficRates.id()),
            waiting_note,
            Space::new().height(theme::SP_SM),
            chart_canvas,
        ]
        .spacing(SP_XS),
    );
    let chart_card = container(chart_card)
        .id(InteractionRegion::TrafficObservation.id())
        .width(Length::Fill);

    // 3. Traffic audit: the reader picks a range and a dimension, the owner
    // answers with that window's buckets and its ranked entities.
    let audit_card = render_traffic_audit(state, &lang);

    column![kpi_row, chart_card, audit_card]
        .spacing(SP_LG)
        .width(Length::Fill)
        .into()
}

/// One audit card: the range and dimension controls, the window's own bucket
/// strip, and the owner's ranked rows for the selected dimension.
fn render_traffic_audit<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let window_labels: Vec<String> = TRAFFIC_AUDIT_WINDOWS
        .iter()
        .map(|window| lang.tr(traffic_audit_window_label_key(*window)).to_string())
        .collect();
    let dimension_labels: Vec<String> = TRAFFIC_AUDIT_DIMENSIONS
        .iter()
        .map(|dimension| {
            lang.tr(traffic_audit_dimension_label_key(*dimension)
                .expect("an offered dimension always has a copy key"))
                .to_string()
        })
        .collect();

    let window_selector = segmented_control(
        &window_labels,
        traffic_audit_window_index(state.diag.traffic_audit.window),
        |index| Message::SetTrafficAuditWindow(traffic_audit_window_at(index)),
    );
    let dimension_selector = segmented_control(
        &dimension_labels,
        traffic_audit_dimension_index(state.diag.traffic_audit.dimension),
        |index| Message::SetTrafficAuditDimension(traffic_audit_dimension_at(index)),
    );

    let header = Some(
        row![
            window_selector,
            Space::new().width(theme::SP_SM),
            dimension_selector,
        ]
        .align_y(Alignment::Center)
        .into(),
    );

    let Some(readback) = state.diag.traffic_audit.readback(AUDIT_RANK_LIMIT) else {
        return card(
            None,
            column![
                section_header(lang.tr("traffic_multidim_analysis").as_ref(), header),
                Space::new().height(theme::SP_SM),
                empty_state(
                    Icon::Activity,
                    lang.tr("traffic_no_stats").as_ref(),
                    lang.tr("traffic_no_stats_desc").as_ref(),
                ),
            ],
        );
    };

    let totals = row![
        badge(
            format!(
                "{} {}",
                lang.tr("traffic_total"),
                format_bytes(readback.totals.total_bytes())
            ),
            BadgeKind::Accent,
        ),
        badge(
            format!("↑ {}", format_bytes(readback.totals.upload_bytes)),
            BadgeKind::Success,
        ),
        badge(
            format!("↓ {}", format_bytes(readback.totals.download_bytes)),
            BadgeKind::Accent,
        ),
        badge(format!("Top {}", readback.rows.len()), BadgeKind::Neutral),
    ]
    .spacing(theme::SP_SM)
    .align_y(Alignment::Center)
    .wrap();

    container(card(
        None,
        column![
            section_header(lang.tr("traffic_multidim_analysis").as_ref(), header),
            Space::new().height(theme::SP_SM),
            audit_trend_strip(&readback, lang, tokens(&state.shell.theme)),
            Space::new().height(theme::SP_MD),
            totals,
            Space::new().height(theme::SP_MD),
            render_audit_rankings(&readback, lang),
        ]
        .spacing(SP_XS),
    ))
    .id(InteractionRegion::TrafficAudit.id())
    .width(Length::Fill)
    .into()
}

/// How many ranked entities a surface shows for one dimension.
pub const AUDIT_RANK_LIMIT: usize = 8;

/// Height of the window's bucket strip.
const AUDIT_TREND_HEIGHT: f32 = 56.0;

/// Width of one bucket's bar. Fixed, so a window with few buckets reads as a
/// series of bars instead of one stretched block.
const AUDIT_TREND_BUCKET_WIDTH: f32 = 18.0;

/// The selected window's whole traffic, one bar per recorded bucket.
///
/// A bucket with no traffic simply has no bar: the strip places each bar at
/// its own bucket, so a quiet hour reads quiet instead of reading invented.
fn audit_trend_strip<'a>(
    readback: &TrafficAuditReadback,
    lang: &Lang<'a>,
    theme_tokens: &'static theme::Tokens,
) -> Element<'a, Message> {
    let peak = readback
        .trend
        .iter()
        .map(|point| point.total_bytes())
        .max()
        .unwrap_or(0);
    if readback.trend.is_empty() || peak == 0 {
        return theme::role_text(lang.tr("traffic_no_stats").to_string(), TextRole::Caption)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            })
            .into();
    }

    let mut bars = row![].spacing(2.0).align_y(Alignment::End);
    for point in &readback.trend {
        let down_height = (point.download_bytes as f32 / peak as f32) * AUDIT_TREND_HEIGHT;
        let up_height = (point.upload_bytes as f32 / peak as f32) * AUDIT_TREND_HEIGHT;
        bars = bars.push(
            column![
                container(Space::new().width(Length::Fill))
                    .width(Length::Fixed(AUDIT_TREND_BUCKET_WIDTH))
                    .height(Length::Fixed(up_height.max(1.0)))
                    .style(move |_: &Theme| container::Style {
                        background: Some(theme_tokens.success.into()),
                        border: Border {
                            radius: border::Radius::from(2.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                Space::new().height(1.0),
                container(Space::new().width(Length::Fill))
                    .width(Length::Fixed(AUDIT_TREND_BUCKET_WIDTH))
                    .height(Length::Fixed(down_height.max(1.0)))
                    .style(move |_: &Theme| container::Style {
                        background: Some(theme_tokens.accent.into()),
                        border: Border {
                            radius: border::Radius::from(2.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
            ]
            .spacing(0.0)
            .width(Length::Fixed(AUDIT_TREND_BUCKET_WIDTH))
            .align_x(Alignment::Center),
        );
    }

    container(bars)
        .width(Length::Fill)
        .height(Length::Fixed(AUDIT_TREND_HEIGHT))
        .align_x(Alignment::Start)
        .into()
}

/// Specialized public exit IP tile matching KPI stat card visual styling.
fn public_ip_tile<'a>(state: &'a AppState, lang: &Lang<'a>, accent: Color) -> Element<'a, Message> {
    let _is_zh = !lang.0.starts_with("en");
    let icon_chip = container(icon(Icon::Globe, 20.0, Color { a: 0.9, ..accent }))
        .width(40)
        .height(40)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Color { a: 0.14, ..accent }.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        });

    let ip_text = state.diag.public_ip.as_deref().unwrap_or("—");
    let probe_btn = button(theme::role_body_text(
        lang.tr("traffic_probe_btn").to_string(),
    ))
    .padding([2, 6])
    .style(button::secondary)
    .on_press(Message::FetchIpInfo);

    let sub_note = match (
        state.diag.public_ip_provider.as_deref(),
        state.diag.public_ip_checked_at.as_deref(),
        state.diag.public_ip_error.as_deref(),
    ) {
        (Some(provider), Some(checked_at), _) => {
            theme::role_body_text(format!("{provider} · {checked_at}"))
        }
        (_, _, Some(error)) => theme::role_body_text(format!("Error: {error}")),
        _ => theme::role_body_text(lang.tr("traffic_probe_tooltip").to_string()),
    }
    .style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_tertiary),
    });

    let texts = column![
        row![
            theme::role_body_text(lang.tr("runtime_stat_public_ip").to_string()).style(
                |t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }
            ),
            Space::new().width(Length::Fill),
            probe_btn,
        ]
        .align_y(Alignment::Center),
        theme::role_body_text(ip_text.to_string()).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary)
        }),
        sub_note,
    ]
    .spacing(2)
    .width(Length::Fill);

    container(
        row![icon_chip, texts]
            .spacing(theme::SP_MD)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(theme::SP_LG)
    .style(move |t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: theme::HAIRLINE,
                color: tk.card_border,
            },
            shadow: tk.card_shadow,
            ..Default::default()
        }
    })
    .into()
}

/// The owner's ranked rows for the selected dimension.
///
/// The surface renders what it was handed - the entity, the two directions and
/// the share of the window the owner computed. No ranking, no share and no
/// entity name is derived here.
fn render_audit_rankings<'a>(
    readback: &TrafficAuditReadback,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    if readback.rows.is_empty() {
        return empty_state(
            Icon::Activity,
            lang.tr("traffic_no_stats").as_ref(),
            lang.tr("traffic_no_stats_desc").as_ref(),
        );
    }

    let mut list = column![].spacing(SP_SM);
    for (index, entry) in readback.rows.iter().enumerate() {
        list = list.push(audit_rank_row(entry, index + 1, lang));
    }
    list.into()
}

/// One ranked entity: the reference product's row shape - rank, name, the two
/// directions, the window total and the share bar.
fn audit_rank_row<'a>(
    entry: &TrafficAuditRow,
    position: usize,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    let rank_kind = if position == 1 {
        BadgeKind::Accent
    } else {
        BadgeKind::Neutral
    };
    let total_label = lang.tr("traffic_total");
    let row_content = column![
        row![
            badge(format!("#{position}"), rank_kind),
            Space::new().width(theme::SP_SM),
            theme::role_body_text(entry.entity.clone())
                .width(Length::Fill)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            theme::role_body_text(format!(
                "\u{2191} {} / \u{2193} {}",
                format_bytes(entry.upload_bytes),
                format_bytes(entry.download_bytes)
            ))
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
            Space::new().width(theme::SP_MD),
            theme::role_body_text(format!("{total_label} {}", format_bytes(entry.total_bytes)))
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().width(theme::SP_MD),
            theme::role_body_text(format!("{:.1}%", entry.share_percent)).style(|t: &Theme| {
                text::Style {
                    color: Some(tokens(t).text_primary),
                }
            }),
        ]
        .align_y(Alignment::Center),
        Space::new().height(SP_XS),
        share_bar(entry.share_percent),
    ]
    .spacing(2);

    container(row_content)
        .padding([theme::SP_SM, SP_MD])
        .width(Length::Fill)
        .style(row_card_surface)
        .into()
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
                        border: Border {
                            radius: border::Radius::from(theme::R_PILL),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
            container(Space::new().height(Length::Fixed(BAR_HEIGHT)))
                .width(Length::FillPortion(
                    ((1.0 - clamped_ratio) * 1000.0).max(1.0) as u16
                ))
                .style(|_t: &Theme| container::Style {
                    background: None,
                    ..Default::default()
                }),
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
            border: Border {
                radius: border::Radius::from(theme::R_PILL),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
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
                    .with_color(Color {
                        a: 0.15,
                        ..tk.text_tertiary
                    })
                    .with_width(1.0),
            );
            return vec![frame.into_geometry()];
        }

        let max_points: usize = 60;
        let left_pad = 4.0_f32;
        let right_pad = 12.0_f32;
        let usable_w = (width - left_pad - right_pad).max(10.0);
        let x_step = usable_w / (max_points - 1) as f32;
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
                    .with_color(Color {
                        a: 0.08,
                        ..tk.divider
                    })
                    .with_width(1.0),
            );
        }

        let start_offset = max_points.saturating_sub(self.history.len());

        let down_pts: Vec<Point> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, (_, down))| {
                Point::new(
                    left_pad + (start_offset + i) as f32 * x_step,
                    scale_y(*down),
                )
            })
            .collect();

        let up_pts: Vec<Point> = self
            .history
            .iter()
            .enumerate()
            .map(|(i, (up, _))| {
                Point::new(left_pad + (start_offset + i) as f32 * x_step, scale_y(*up))
            })
            .collect();

        // 1. Download area & smooth curve
        let down_area = build_smooth_area(&down_pts, height);
        frame.fill(&down_area, Color { a: 0.12, ..accent });
        let down_curve = build_smooth_path(&down_pts);
        frame.stroke(
            &down_curve,
            canvas::Stroke::default().with_color(accent).with_width(2.5),
        );

        if let Some(&last_pt) = down_pts.last() {
            frame.fill(
                &canvas::Path::circle(last_pt, 6.0),
                Color { a: 0.20, ..accent },
            );
            frame.fill(&canvas::Path::circle(last_pt, 3.8), accent);
            frame.fill(&canvas::Path::circle(last_pt, 1.8), Color::WHITE);
        }

        // 2. Upload area & smooth curve
        let up_area = build_smooth_area(&up_pts, height);
        frame.fill(&up_area, Color { a: 0.08, ..success });
        let up_curve = build_smooth_path(&up_pts);
        frame.stroke(
            &up_curve,
            canvas::Stroke::default()
                .with_color(success)
                .with_width(2.0),
        );

        if let Some(&last_pt) = up_pts.last() {
            frame.fill(
                &canvas::Path::circle(last_pt, 5.0),
                Color { a: 0.20, ..success },
            );
            frame.fill(&canvas::Path::circle(last_pt, 3.2), success);
            frame.fill(&canvas::Path::circle(last_pt, 1.6), Color::WHITE);
        }

        vec![frame.into_geometry()]
    }
}

/// Helper to add Catmull-Rom cubic Bezier spline segments to a path builder.
fn add_smooth_curves(builder: &mut Builder, points: &[Point]) {
    if points.len() < 2 {
        return;
    }
    if points.len() == 2 {
        builder.line_to(points[1]);
        return;
    }
    let n = points.len();
    for window in catmull_rom_windows(n) {
        let p0 = points[window.previous];
        let p1 = points[window.start];
        let p2 = points[window.end];
        let p3 = points[window.next];

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
#[path = "../../../tests/gui/view_runtime_traffic_tests.rs"]
mod tests;
