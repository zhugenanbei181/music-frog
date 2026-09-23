//! Runtime status hero: the accent status card, its meta chips and the
//! one-click speedtest / cancel control.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{chip, premium_card, status_dot};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, R_CHIP, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

/// Accent hero: status dot + localized status, mode / core-version meta row
/// and the prominent start/stop control.
pub fn overview_speedtest_button<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    // The running state and progress come from the shared engine snapshot the
    // desktop pump publishes; the legacy flag remains only as a UI spinner for
    // hosts whose surface pump has not yet delivered a snapshot this tick.
    let snapshot = &state.diag.speedtest;
    let snapshot_running = snapshot.is_running();
    let is_testing = snapshot_running || state.runtime.runtime_testing_all_delays;
    let label: String = if snapshot_running {
        let done = snapshot.progress.completed_nodes;
        let total = snapshot.progress.total_nodes;
        if total > 0 {
            format!("{} {done}/{total}", lang.tr("runtime_delay_testing_all"))
        } else {
            lang.tr("runtime_delay_testing_all").into_owned()
        }
    } else if state.runtime.runtime_testing_all_delays {
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

    let test_btn = button(btn_content)
        .padding([6, 12])
        .style(style_ghost)
        .on_press_maybe((!is_testing).then_some(Message::TestAllProxyDelays));

    if !is_testing {
        return test_btn.into();
    }

    // While a batch is in flight the same control becomes an honest cancel
    // action routed to the shared engine's cancel token.
    let cancel_btn = button(
        row![
            icon_themed(Icon::X, 13.0, |t: &Theme| tokens(t).danger),
            Space::new().width(theme::SP_XS),
            text(lang.tr("speedtest_cancel").to_string())
                .size(12)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger)
                }),
        ]
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(style_ghost)
    .on_press(Message::CancelSpeedtest);

    row![test_btn, Space::new().width(theme::SP_XS), cancel_btn]
        .align_y(Alignment::Center)
        .into()
}

pub fn hero_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
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
