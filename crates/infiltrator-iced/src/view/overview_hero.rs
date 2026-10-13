//! Runtime status hero: the accent status card, its meta chips and the
//! one-click speedtest / cancel control.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{chip, premium_card, status_dot};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, MONO, R_CHIP, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_application::routing_intent_projection::routing_intent_copy;
use infiltrator_contract::core_control::CoreControlAction;
use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_presentation::core_status_projection::lifecycle_copy;
use infiltrator_shared::locales::{Lang, Localizer};

/// Accent hero: status dot + localized status, routing intent / core-version meta row
/// and the prominent start/stop control.
pub fn overview_speedtest_button<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    // The Speedtest snapshot and typed outbound-batch owner each provide their
    // own running state; the button never invents batch progress.
    let snapshot = &state.diag.speedtest;
    let snapshot_running = snapshot.is_running();
    let batch_running = state.runtime.outbound_probe_batch.is_running();
    let is_testing = snapshot_running || batch_running;
    let label: String = if snapshot_running {
        let progress = snapshot.state.progress();
        let done = progress.map_or(0, |progress| progress.completed_targets);
        let total = progress.map_or(0, |progress| progress.total_targets);
        if total > 0 {
            format!("{} {done}/{total}", lang.tr("runtime_delay_testing_all"))
        } else {
            lang.tr("runtime_delay_testing_all").into_owned()
        }
    } else if batch_running {
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
        .on_press_maybe(
            (!is_testing)
                .then(|| state.outbound_probe_batch_message(OutboundProbeBatchScope::All))
                .flatten(),
        );

    if !is_testing {
        return test_btn.into();
    }

    // Speedtest has its own cancellation token. A typed outbound batch can be
    // dismissed locally; its late receipt is discarded by the batch token.
    let dismiss_message = if snapshot_running {
        let Some(run_id) = snapshot.state.run_id() else {
            return test_btn.into();
        };
        Message::CancelSpeedtestRun(run_id)
    } else {
        Message::DismissOutboundProbeBatch
    };
    let dismiss_label = if snapshot_running {
        lang.tr("speedtest_cancel").into_owned()
    } else {
        "Close".to_owned()
    };
    let cancel_btn = button(
        row![
            icon_themed(Icon::X, 13.0, |t: &Theme| tokens(t).danger),
            Space::new().width(theme::SP_XS),
            text(dismiss_label)
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
    .on_press(dismiss_message);

    row![test_btn, Space::new().width(theme::SP_XS), cancel_btn]
        .align_y(Alignment::Center)
        .into()
}

pub fn hero_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let lifecycle = state.core_lifecycle_snapshot().lifecycle;
    let running = matches!(lifecycle, CoreLifecycle::Running | CoreLifecycle::Ready);
    let status_text = lifecycle_copy(&lifecycle, lang.0);

    let projection = state.core_control_projection();
    let stop = projection.action == Some(CoreControlAction::Stop);
    let message = projection.action.map(|action| match action {
        CoreControlAction::Start => Message::StartCore,
        CoreControlAction::Stop => Message::StopCore,
    });
    let control: Element<'a, Message> = button(
        row![
            icon_themed(
                if stop { Icon::Plug } else { Icon::Zap },
                14.0,
                |t: &Theme| tokens(t).on_accent
            ),
            text(lang.tr(projection.label.key()).into_owned())
                .size(13)
                .font(FONT_SEMIBOLD),
        ]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center),
    )
    .padding([10, 20])
    .style(if stop { button::danger } else { style_accent })
    .on_press_maybe(message)
    .into();

    premium_card(
        row![
            status_dot(running),
            Space::new().width(theme::SP_MD),
            column![
                text(status_text)
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

    if let Some(intent) = state.runtime.routing_intent.as_ref() {
        meta = meta.push(chip(routing_intent_copy(intent, lang.0)));
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

    let exit_node = (!state.runtime.runtime_selected_proxy.trim().is_empty())
        .then_some(state.runtime.runtime_selected_proxy.as_str());
    if let Some(exit_node) = exit_node {
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

/// Version of the installed default kernel, if one is registered.
fn default_core_version(state: &AppState) -> Option<String> {
    state
        .runtime
        .installed_kernels
        .iter()
        .find(|kernel| kernel.is_default)
        .map(|kernel| kernel.version.clone())
}
