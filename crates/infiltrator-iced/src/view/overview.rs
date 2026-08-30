//! Overview page in the Clash-Party design language: section header with
//! quick actions, a runtime status hero card, a real-time traffic chart and
//! a four-tile stats grid with mono numerals. Everything is backed by
//! existing [`AppState`] fields — nothing is faked.

use crate::locales::{Lang, Localizer};
use crate::types::{Route, RuntimeStatus};
use crate::view::components::{
    TrafficChart, card, card_surface, chip, icon_button, modern_scrollable, premium_card,
    section_header, status_dot,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, R_CONTROL};
use crate::{AppState, Message};
use iced::widget::{Space, button, canvas, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let header = section_header(
        &lang.tr("nav_overview"),
        Some(
            row![
                icon_button(Icon::RefreshCw, 16.0, Message::RefreshRuntimeNow),
                Space::new().width(theme::SP_SM),
                icon_button(Icon::Settings, 16.0, Message::Navigate(Route::Settings)),
            ]
            .align_y(Alignment::Center)
            .into(),
        ),
    );

    let hero = hero_card(state, &lang);
    let traffic_chart = traffic_card(state, &lang);
    let stats = stats_grid(state, &lang);

    let content = column![header, hero, traffic_chart, stats]
        .spacing(theme::SP_LG)
        .max_width(1100);

    modern_scrollable(content).height(Length::Fill).into()
}

// ---------------------------------------------------------------------------
// Runtime status hero
// ---------------------------------------------------------------------------

/// Accent hero: status dot + localized status, mode / core-version meta row
/// and the prominent start/stop control.
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
            text(lang.tr("stop_proxy").into_owned())
                .size(13)
                .font(FONT_SEMIBOLD),
        )
        .padding([10, 20])
        .style(button::danger)
        .on_press(Message::StopProxy)
        .into()
    } else {
        button(
            text(lang.tr("start_proxy").into_owned())
                .size(13)
                .font(FONT_SEMIBOLD),
        )
        .padding([10, 20])
        .style(button::primary)
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
                        color: Some(theme::tokens(t).text_primary),
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
    let mut meta = row![].spacing(theme::SP_SM);

    if let Some(mode) = state.runtime.proxy_mode.as_deref() {
        meta = meta.push(chip(mode_label(mode, lang)));
    }

    if let Some(version) = default_core_version(state) {
        meta = meta.push(text(format!("mihomo {version}")).size(12).font(MONO).style(
            |t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_secondary),
            },
        ));
    }

    if let Some(exit_node) = state.runtime.proxies.get("GLOBAL").and_then(|g| g.now()) {
        meta = meta.push(
            text(exit_node.to_string())
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_tertiary),
                }),
        );
    }

    meta.into()
}

/// Localized label for a mihomo mode identifier (unknown values pass through).
fn mode_label(mode: &str, lang: &Lang<'_>) -> String {
    let key = match mode {
        "rule" => "mode_rule",
        "global" => "mode_global",
        "direct" => "mode_direct",
        _ => return mode.to_string(),
    };
    lang.tr(key).into_owned()
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
// Traffic chart
// ---------------------------------------------------------------------------

fn traffic_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let speeds: Element<'a, Message> = if let Some(traffic) = &state.diag.traffic {
        row![
            speed_text(Icon::ArrowUp, traffic.up, |t: &Theme| theme::tokens(t)
                .success,),
            Space::new().width(theme::SP_LG),
            speed_text(Icon::ArrowDown, traffic.down, |t: &Theme| theme::tokens(t)
                .text_secondary,),
        ]
        .align_y(Alignment::Center)
        .into()
    } else {
        text(lang.tr("waiting_traffic").into_owned())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_tertiary),
            })
            .into()
    };

    card(
        Some(lang.tr("overview_traffic").into_owned()),
        column![
            speeds,
            Space::new().height(theme::SP_MD),
            canvas::Canvas::new(TrafficChart {
                history: state.diag.traffic_history.clone(),
            })
            .width(Length::Fill)
            .height(Length::Fixed(120.0)),
        ],
    )
}

fn speed_text<'a>(
    glyph: Icon,
    bytes_per_second: u64,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    row![
        icon_themed(glyph, 14.0, color),
        Space::new().width(theme::SP_XS),
        text(format!(
            "{}/s",
            crate::utils::format_bytes(bytes_per_second)
        ))
        .size(14)
        .font(MONO)
        .style(move |t: &Theme| text::Style {
            color: Some(color(t))
        }),
    ]
    .align_y(Alignment::Center)
    .into()
}

// ---------------------------------------------------------------------------
// Stats grid
// ---------------------------------------------------------------------------

/// 连接数 / 内存 / 上传 / 下载 tiles with mono numerals.
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
    let download = state
        .diag
        .traffic
        .as_ref()
        .map(|traffic| format!("{}/s", crate::utils::format_bytes(traffic.down)))
        .unwrap_or_else(|| "—".to_string());

    row![
        metric_tile(
            Icon::Activity,
            stat_label(lang, "连接数", "Connections"),
            connections,
            |t| theme::tokens(t).accent,
        ),
        metric_tile(
            Icon::Server,
            stat_label(lang, "内存", "Memory"),
            memory,
            |t| theme::tokens(t).accent,
        ),
        metric_tile(
            Icon::ArrowUp,
            stat_label(lang, "上传", "Upload"),
            upload,
            |t| theme::tokens(t).success,
        ),
        metric_tile(
            Icon::ArrowDown,
            stat_label(lang, "下载", "Download"),
            download,
            |t| theme::tokens(t).accent,
        ),
    ]
    .spacing(theme::SP_SM)
    .width(Length::Fill)
    .into()
}

fn metric_tile<'a>(
    glyph: Icon,
    label: String,
    value: String,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    let icon_chip = container(icon_themed(glyph, 18.0, color))
        .width(36)
        .height(36)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).accent_soft.into()),
            border: Border {
                radius: border::Radius::from(R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        });

    container(
        row![
            icon_chip,
            column![
                text(label).size(11).style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_secondary),
                }),
                text(value)
                    .size(16)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(theme::tokens(t).text_primary),
                    }),
            ]
            .spacing(2),
        ]
        .spacing(theme::SP_MD)
        .align_y(Alignment::Center),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_LG)
    .style(card_surface)
    .into()
}

/// Bilingual fallback for the few stat labels that have no locale key yet
/// (locales.rs is outside this wave's file ownership).
fn stat_label(lang: &Lang<'_>, zh: &str, en: &str) -> String {
    if lang.0.starts_with("en") {
        en.to_string()
    } else {
        zh.to_string()
    }
}
