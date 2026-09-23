//! System integration cards: shell exports, inbounds, system proxy and
//! appearance/autostart settings.

use super::format::format_port_conflicts;
use crate::state::AppState;
use crate::types::app::{ConfirmAction, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::components::{
    BadgeKind, badge, card, form_field_label, form_input_style, form_pick_style, form_toggle_row,
    kbd_badge, row_card_surface, segmented_control, status_dot, style_accent, style_danger,
    style_ghost, text_btn,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct SettingsChoice {
    pub(super) value: &'static str,
}

impl std::fmt::Display for SettingsChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.value)
    }
}

pub(super) const LANGUAGE_OPTIONS: &[SettingsChoice] = &[
    SettingsChoice { value: "zh-CN" },
    SettingsChoice { value: "en-US" },
];
pub(super) const CORE_CHANNEL_OPTIONS: &[SettingsChoice] = &[
    SettingsChoice { value: "stable" },
    SettingsChoice { value: "alpha" },
    SettingsChoice { value: "meta-core" },
];
pub(super) const CORE_LOG_LEVEL_OPTIONS: &[SettingsChoice] = &[
    SettingsChoice { value: "debug" },
    SettingsChoice { value: "info" },
    SettingsChoice { value: "warn" },
    SettingsChoice { value: "error" },
];

pub(super) fn secondary_text(value: impl Into<String>) -> Element<'static, Message> {
    text(value.into())
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
        .into()
}

pub(super) fn shell_export_row<'a>(
    shell_name: &'static str,
    command: &'static str,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let copy_msg = Message::ShowToast(
        infiltrator_shared::i18n_interpolator::interpolate(
            &lang.tr("settings_copied_env"),
            &[("shell_name", shell_name)],
        ),
        ToastStatus::Success,
    );
    let copy_btn = button(
        row![
            icon_themed(Icon::Copy, 12.0, |t| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("settings_copy").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(copy_msg);

    container(
        row![
            kbd_badge(shell_name),
            Space::new().width(theme::SP_SM),
            text(command)
                .size(11)
                .font(MONO)
                .width(Length::Fill)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
            Space::new().width(theme::SP_SM),
            copy_btn,
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([theme::SP_SM, theme::SP_MD])
    .width(Length::Fill)
    .style(row_card_surface)
    .into()
}

pub(super) fn shell_export_card<'a>(lang: &Lang<'_>) -> Element<'a, Message> {
    let bash_cmd = "export http_proxy=http://127.0.0.1:7890 https_proxy=http://127.0.0.1:7890 all_proxy=socks5://127.0.0.1:7891";
    let fish_cmd = "set -gx http_proxy http://127.0.0.1:7890; set -gx https_proxy http://127.0.0.1:7890; set -gx all_proxy socks5://127.0.0.1:7891";
    let pwsh_cmd = "$env:http_proxy=\"http://127.0.0.1:7890\"; $env:https_proxy=\"http://127.0.0.1:7890\"; $env:all_proxy=\"socks5://127.0.0.1:7891\"";
    let cmd_cmd = "set http_proxy=http://127.0.0.1:7890 & set https_proxy=http://127.0.0.1:7890 & set all_proxy=socks5://127.0.0.1:7891";

    card(
        Some(lang.tr("settings_term_env_title").to_string()),
        column![
            secondary_text(lang.tr("settings_term_env_desc").to_string()),
            Space::new().height(theme::SP_XS),
            shell_export_row("Bash / Zsh", bash_cmd, lang),
            shell_export_row("Fish", fish_cmd, lang),
            shell_export_row("PowerShell", pwsh_cmd, lang),
            shell_export_row("Windows CMD", cmd_cmd, lang),
        ]
        .spacing(theme::SP_SM),
    )
}

fn inbound_port_tile<'a>(
    running: bool,
    title: String,
    badge_label: String,
    badge_kind: BadgeKind,
    proto_tag: &'static str,
    endpoint: &'static str,
) -> Element<'a, Message> {
    container(
        column![
            row![
                status_dot(running),
                Space::new().width(theme::SP_SM),
                text(title)
                    .size(13)
                    .font(FONT_SEMIBOLD)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                badge(badge_label, badge_kind),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                kbd_badge(proto_tag),
                Space::new().width(theme::SP_SM),
                text(endpoint)
                    .size(13)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_MD)
    .style(row_card_surface)
    .into()
}

pub(super) fn inbounds_card<'a>(state: &AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let running = matches!(state.runtime.status, RuntimeStatus::Running);
    let (status_str, badge_kind) = if running {
        (
            lang.tr("settings_listening").to_string(),
            BadgeKind::Success,
        )
    } else {
        (lang.tr("settings_ready").to_string(), BadgeKind::Neutral)
    };

    let mixed_tile = inbound_port_tile(
        running,
        lang.tr("settings_mixed_port").to_string(),
        status_str.clone(),
        badge_kind,
        "HTTP / SOCKS5",
        "127.0.0.1:7890",
    );
    let socks_tile = inbound_port_tile(
        running,
        lang.tr("settings_socks_port").to_string(),
        status_str,
        badge_kind,
        "SOCKS5",
        "127.0.0.1:7891",
    );

    card(
        Some(lang.tr("settings_inbound_title").to_string()),
        column![
            secondary_text(lang.tr("settings_inbound_desc").to_string()),
            Space::new().height(theme::SP_XS),
            row![mixed_tile, socks_tile].spacing(theme::SP_MD),
        ]
        .spacing(theme::SP_SM),
    )
}

pub(super) fn system_proxy_card<'a>(
    state: &AppState,
    lang: &Lang<'a>,
    _is_en: bool,
) -> Element<'a, Message> {
    const DEFAULT_BYPASS: &str = "localhost;127.*;10.*;192.168.*;*.lan";
    let proxy_mode_options = vec![
        lang.tr("settings_mode_manual").to_string(),
        "PAC".to_string(),
    ];
    let port_status = format_port_conflicts(&state.runtime.port_conflicts);

    card(
        Some(lang.tr("system_proxy").to_string()),
        column![
            form_toggle_row(
                lang.tr("settings_sys_proxy").to_string(),
                state.runtime.system_toggles.system_proxy.is_enabled(),
                Message::SetSystemProxy,
            ),
            row![
                text(lang.tr("settings_proxy_host").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                text("127.0.0.1")
                    .size(13)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
            ]
            .align_y(Alignment::Center),
            row![
                text(lang.tr("settings_proxy_mode").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                segmented_control(&proxy_mode_options, 0, |_| Message::Noop),
            ]
            .align_y(Alignment::Center),
            row![
                text("Port conflicts")
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                secondary_text(port_status),
                Space::new().width(theme::SP_SM),
                text_btn(
                    "Check & repair",
                    style_ghost,
                    Some(Message::RepairPortConflicts)
                ),
            ]
            .align_y(Alignment::Center),
            column![
                row![
                    form_field_label(lang.tr("settings_proxy_bypass").to_string()),
                    Space::new().width(Length::Fill),
                    text_btn(
                        lang.tr("settings_add_default_bypass").to_string(),
                        style_ghost,
                        Some(Message::UpdateSystemProxyBypass(DEFAULT_BYPASS.to_string()))
                    ),
                ]
                .align_y(Alignment::Center),
                Space::new().height(theme::SP_XS),
                text_input(DEFAULT_BYPASS, &state.shell.system_proxy_bypass)
                    .on_input(Message::UpdateSystemProxyBypass)
                    .padding([7, 11])
                    .size(12)
                    .font(MONO)
                    .style(form_input_style),
            ]
            .spacing(theme::SP_XS),
        ]
        .spacing(theme::SP_SM),
    )
}

pub(super) fn system_integration_card<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
    _is_en: bool,
    theme_selector: Element<'a, Message>,
    selected_language: Option<SettingsChoice>,
) -> Element<'a, Message> {
    card(
        Some(lang.tr("settings_system_integration").to_string()),
        column![
            form_toggle_row(
                lang.tr("autostart").to_string(),
                state.runtime.autostart_enabled,
                Message::SetAutostart
            ),
            form_toggle_row(
                lang.tr("settings_close_to_tray").to_string(),
                state.shell.close_to_tray,
                Message::UpdateCloseToTray
            ),
            form_toggle_row(
                lang.tr("settings_notifications").to_string(),
                state.shell.notifications_enabled,
                Message::UpdateNotificationsEnabled
            ),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("theme").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(theme::SP_MD),
                theme_selector,
            ]
            .align_y(Alignment::Center),
            row![
                text(lang.tr("settings_lang_label").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                pick_list(
                    LANGUAGE_OPTIONS,
                    selected_language,
                    |choice: SettingsChoice| Message::SetLanguage(choice.value.to_string())
                )
                .width(Length::Fixed(120.0))
                .style(form_pick_style),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_SM),
            row![
                text_btn(
                    if state.profile.is_saving_app_settings {
                        lang.tr("settings_saving").to_string()
                    } else {
                        lang.tr("settings_save_btn").to_string()
                    },
                    style_accent,
                    (!state.profile.is_saving_app_settings).then_some(Message::SaveAppSettings)
                ),
                Space::new().width(theme::SP_MD),
                text_btn(
                    if state.shell.is_factory_resetting {
                        lang.tr("settings_reverting").to_string()
                    } else {
                        lang.tr("settings_factory_reset").to_string()
                    },
                    style_danger,
                    (!state.shell.is_factory_resetting)
                        .then_some(Message::RequestConfirmation(ConfirmAction::FactoryReset))
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
