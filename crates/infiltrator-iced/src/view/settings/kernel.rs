//! Kernel management card.

use super::integration::{
    CORE_CHANNEL_OPTIONS, CORE_LOG_LEVEL_OPTIONS, SettingsChoice, secondary_text,
};
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::view::component_forms::{
    form_pick_style, row_card_surface, style_accent, style_ghost, text_btn,
};
use crate::view::components::{BadgeKind, badge, card, icon_button, section_header};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, R_CONTROL, SP_MD, tokens};
use iced::widget::{Space, column, container, pick_list, progress_bar, row, text};
use iced::{Alignment, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

use super::format::{
    format_controller_auth, format_core_resources, format_core_versions, format_integrity,
    format_offline_startup,
};

pub(super) fn kernel_management_card<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
    _is_en: bool,
    selected_core_channel: Option<SettingsChoice>,
) -> Element<'a, Message> {
    let mut kernel_rows = column![].spacing(theme::SP_SM);
    let channel_probe = format_core_versions(&state.runtime.core_versions);
    let integrity = format_integrity(&state.runtime.core_integrity);
    let controller_auth = format_controller_auth(&state.runtime.controller_auth);
    let offline_startup = format_offline_startup(&state.runtime.offline_startup);
    let selected_log_level = CORE_LOG_LEVEL_OPTIONS
        .iter()
        .find(|option| option.value == state.diag.log_level)
        .copied();

    kernel_rows = kernel_rows.push(secondary_text(format!("Online channels: {channel_probe}")));
    kernel_rows = kernel_rows.push(secondary_text(format!("Artifact integrity: {integrity}")));
    kernel_rows = kernel_rows.push(secondary_text(format!(
        "Controller auth: {controller_auth}"
    )));
    kernel_rows = kernel_rows.push(secondary_text(format!(
        "Offline startup: {offline_startup}"
    )));
    kernel_rows = kernel_rows.push(secondary_text(format!(
        "Core resources: {}",
        format_core_resources(&state.runtime.core_resources)
    )));
    kernel_rows = kernel_rows.push(
        row![
            text("Core log level").size(13),
            Space::new().width(Length::Fill),
            pick_list(
                CORE_LOG_LEVEL_OPTIONS,
                selected_log_level,
                |choice: SettingsChoice| { Message::SetCoreLogLevel(choice.value.to_owned()) }
            )
            .width(Length::Fixed(120.0))
            .style(form_pick_style),
        ]
        .align_y(Alignment::Center),
    );
    let rollback_target = state.runtime.core_versions.rollback.target.clone();
    kernel_rows = kernel_rows.push(secondary_text(rollback_target.as_deref().map_or_else(
        || "Rollback: no previous runnable core".to_owned(),
        |version| format!("Rollback target: {version}"),
    )));
    if rollback_target.is_some() {
        kernel_rows = kernel_rows.push(
            row![
                secondary_text("Switch to the previous verified local binary"),
                Space::new().width(Length::Fill),
                text_btn("Rollback core", style_ghost, Some(Message::RollbackCore)),
            ]
            .align_y(Alignment::Center),
        );
    }

    if let Some(latest) = &state.runtime.latest_core_version {
        kernel_rows = kernel_rows.push(
            container(
                row![
                    text(if state.runtime.is_downloading_core {
                        format!(
                            "{} {:.0}%",
                            lang.tr("settings_downloading"),
                            state.runtime.download_progress * 100.0
                        )
                    } else {
                        format!("{} {}", lang.tr("settings_available"), latest)
                    })
                    .size(13)
                    .width(Length::Fill),
                    if state.runtime.is_downloading_core {
                        Element::from(
                            row![
                                column![
                                    progress_bar(0.0..=1.0, state.runtime.download_progress)
                                        .length(Length::Fixed(180.0)),
                                    secondary_text(format!(
                                        "{} {}/s",
                                        lang.tr("settings_speed"),
                                        state
                                            .runtime
                                            .download_stats
                                            .as_ref()
                                            .map(|s| crate::utils::format_bytes(s.speed_bytes))
                                            .unwrap_or_else(|| "—".to_string())
                                    )),
                                ]
                                .spacing(theme::SP_XS),
                                Space::new().width(theme::SP_SM),
                                text_btn(
                                    lang.tr("btn_cancel").to_string(),
                                    style_ghost,
                                    Some(Message::CancelCoreDownload)
                                ),
                            ]
                            .align_y(Alignment::Center),
                        )
                    } else {
                        Element::from(text_btn(
                            lang.tr("settings_download"),
                            style_accent,
                            Some(Message::DownloadCore(latest.clone())),
                        ))
                    },
                ]
                .align_y(Alignment::Center),
            )
            .padding(theme::SP_MD)
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = tokens(t);
                container::Style {
                    background: Some(
                        Color {
                            a: 0.10,
                            ..tk.success
                        }
                        .into(),
                    ),
                    border: border::Border {
                        radius: border::Radius::from(R_CONTROL),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
        );
    }

    if state.runtime.installed_kernels.is_empty() {
        kernel_rows = kernel_rows.push(secondary_text(lang.tr("settings_no_kernels")));
    } else {
        for kernel in &state.runtime.installed_kernels {
            kernel_rows = kernel_rows.push(
                container(
                    row![
                        column![
                            text(&kernel.version).size(13).font(FONT_SEMIBOLD).style(
                                |t: &Theme| text::Style {
                                    color: Some(tokens(t).text_primary)
                                }
                            ),
                            if kernel.is_default {
                                badge(lang.tr("active_tag").trim().to_string(), BadgeKind::Success)
                            } else {
                                Space::new().width(0).height(0).into()
                            },
                        ]
                        .spacing(2)
                        .width(Length::Fill),
                        if !kernel.is_default {
                            Element::from(
                                row![
                                    text_btn(
                                        lang.tr("settings_set_default"),
                                        style_ghost,
                                        Some(Message::SetDefaultKernel(kernel.version.clone()))
                                    ),
                                    Space::new().width(theme::SP_SM),
                                    icon_button(
                                        Icon::Trash2,
                                        14.0,
                                        Message::RequestConfirmation(ConfirmAction::DeleteKernel(
                                            kernel.version.clone()
                                        ))
                                    ),
                                ]
                                .align_y(Alignment::Center),
                            )
                        } else {
                            Element::from(secondary_text(lang.tr("settings_installed")))
                        },
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([theme::SP_SM, SP_MD])
                .width(Length::Fill)
                .style(row_card_surface),
            );
        }
    }

    card(
        None,
        column![
            section_header(
                lang.tr("settings_kernel_mgmt").as_ref(),
                Some(
                    row![
                        pick_list(
                            CORE_CHANNEL_OPTIONS,
                            selected_core_channel,
                            |choice: SettingsChoice| Message::SetCoreChannel(
                                choice.value.to_string()
                            )
                        )
                        .width(Length::Fixed(110.0))
                        .style(form_pick_style),
                        Space::new().width(theme::SP_SM),
                        if state.runtime.is_checking_update {
                            text(lang.tr("settings_checking").to_string())
                                .size(12)
                                .style(|t: &Theme| text::Style {
                                    color: Some(tokens(t).text_tertiary),
                                })
                                .into()
                        } else {
                            icon_button(Icon::RefreshCw, 14.0, Message::CheckCoreUpdate)
                        },
                    ]
                    .align_y(Alignment::Center)
                    .into(),
                ),
            ),
            Space::new().height(theme::SP_MD),
            kernel_rows,
        ],
    )
}
