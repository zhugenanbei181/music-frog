//! Settings page: shell exports, system proxy, TUN, kernel management,
//! hotkeys and admin surfaces.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, banner_alert, card, form_input_style, form_toggle_row, modern_scrollable,
    segmented_control, status_dot, style_accent, style_ghost, text_btn,
};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_shared::locales::{Lang, Localizer};

use integration::{CORE_CHANNEL_OPTIONS, LANGUAGE_OPTIONS, secondary_text};

mod format;
mod hotkeys;
mod integration;
mod kernel;
mod tun;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let is_en = state.shell.lang.starts_with("en");
    let selected_language = LANGUAGE_OPTIONS
        .iter()
        .find(|option| option.value == state.shell.lang)
        .copied();
    let selected_core_channel = CORE_CHANNEL_OPTIONS
        .iter()
        .find(|option| option.value == state.runtime.core_channel)
        .copied();
    let tun_service_ready = state.shell.is_admin
        || matches!(
            state.runtime.tun_service_status,
            Some(TunServiceStatus::InstalledAndRunning)
        );

    let header = text(lang.tr("nav_settings").to_string())
        .size(24)
        .font(FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        });

    let uac_banner = if !tun_service_ready {
        let uac_title = lang.tr("admin_status").to_string();
        let uac_desc = if cfg!(windows) {
            lang.tr("settings_uac_desc").to_string()
        } else if is_en {
            "Configuring platform permissions is required before enabling TUN mode; restart or prepare permissions below.".to_string()
        } else {
            lang.tr("settings_tun_perm_hint").to_string()
        };
        let uac_btn_label = if cfg!(windows) {
            lang.tr("settings_uac_request").to_string()
        } else if is_en {
            "Prepare TUN Privilege".to_string()
        } else {
            lang.tr("settings_tun_prepare_perm_btn").to_string()
        };
        let action_btn = text_btn(
            uac_btn_label,
            style_accent,
            Some(Message::RequestAdminPrivilege),
        );
        Some(banner_alert(
            BadgeKind::Warning,
            uac_title,
            uac_desc,
            Some(action_btn),
        ))
    } else {
        None
    };

    let theme_labels = vec![
        lang.tr("theme_light").to_string(),
        lang.tr("theme_dark").to_string(),
        lang.tr("theme_forest").to_string(),
        "AMOLED".to_string(),
    ];
    let current_theme_index = if crate::view::theme::is_amoled(&state.shell.theme) {
        3
    } else if crate::view::theme::is_forest(&state.shell.theme) {
        2
    } else if state.shell.theme == Theme::Light {
        0
    } else {
        1
    };
    let theme_selector = segmented_control(&theme_labels, current_theme_index, |index| {
        let name = match index {
            0 => "light",
            2 => "forest",
            3 => "amoled",
            _ => "dark",
        };
        Message::SetTheme(name.to_string())
    });

    let system_proxy_section = integration::system_proxy_card(state, &lang, is_en);
    let system_section = integration::system_integration_card(
        state,
        &lang,
        is_en,
        theme_selector,
        selected_language,
    );
    let tun_section = tun::tun_card(state, &lang, is_en);

    let sniffer_section = card(
        Some(lang.tr("settings_sniffer").to_string()),
        column![
            secondary_text(lang.tr("settings_sniffer_desc")),
            form_toggle_row(
                lang.tr("settings_sniffer").to_string(),
                state.editor.sniffer_enabled,
                Message::SetSnifferEnabled
            ),
        ]
        .spacing(theme::SP_SM),
    );

    let editor_section = card(
        Some("External Editor".to_string()),
        column![
            secondary_text("Set a preferred editor executable path (optional)."),
            text_input(
                "e.g. C:\\Program Files\\Sublime Text\\subl.exe",
                &state.editor.editor_path_setting
            )
            .on_input(Message::UpdateEditorPathSetting)
            .padding([8, 12])
            .size(13)
            .style(form_input_style),
            row![
                if state.profile.is_saving_app_settings {
                    text_btn("Saving...", style_ghost, None)
                } else {
                    text_btn(
                        "Save Editor Path",
                        style_ghost,
                        Some(Message::SaveAppSettings),
                    )
                },
                Space::new().width(theme::SP_SM),
                text_btn(
                    "Reset",
                    style_ghost,
                    Some(Message::UpdateEditorPathSetting(String::new()))
                ),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    );

    let admin_running = state.shell.admin_server.is_running();
    let admin_section = card(
        Some(lang.tr("settings_admin_web").to_string()),
        column![
            secondary_text(lang.tr("settings_admin_desc")),
            form_toggle_row(
                lang.tr("settings_admin_enable").to_string(),
                state.shell.admin_enabled,
                Message::SetAdminEnabled
            ),
            row![
                text(lang.tr("settings_admin_port").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(theme::SP_MD),
                text_input("25210", &state.shell.admin_port_input)
                    .on_input(Message::UpdateAdminPort)
                    .width(Length::Fixed(120.0))
                    .padding([8, 12])
                    .size(13)
                    .font(MONO)
                    .style(form_input_style),
                Space::new().width(theme::SP_MD),
                text_btn(
                    lang.tr("settings_admin_apply"),
                    style_ghost,
                    Some(Message::ApplyAdminSettings)
                ),
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            row![
                status_dot(admin_running),
                Space::new().width(theme::SP_SM),
                text(if admin_running {
                    lang.tr("settings_admin_running")
                } else {
                    lang.tr("settings_admin_stopped")
                })
                .size(12)
                .style(move |t: &Theme| text::Style {
                    color: Some(if admin_running {
                        tokens(t).success
                    } else {
                        tokens(t).text_secondary
                    })
                }),
                Space::new().width(theme::SP_MD),
                if admin_running {
                    text(state.shell.admin_server.url().unwrap_or_default())
                        .size(12)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        })
                        .into()
                } else {
                    Element::from(Space::new().width(Length::Shrink))
                },
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    );

    let mut content = column![header, Space::new().height(theme::SP_LG)].spacing(10);
    if let Some(banner) = uac_banner {
        content = content.push(banner).push(Space::new().height(10));
    }

    content = content
        .push(system_proxy_section)
        .push(Space::new().height(10))
        .push(crate::view::pac_card::pac_card(state, &lang))
        .push(Space::new().height(10))
        .push(system_section)
        .push(Space::new().height(10))
        .push(integration::inbounds_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::lan_sharing_card::lan_sharing_card(
            state, &lang,
        ))
        .push(Space::new().height(10))
        .push(crate::view::lan_security_card::lan_security_card(
            state, &lang,
        ))
        .push(Space::new().height(10))
        .push(integration::shell_export_card(&lang))
        .push(Space::new().height(10))
        .push(tun_section)
        .push(Space::new().height(10))
        .push(crate::view::net_roam_card::net_roam_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::vpn_card::vpn_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::privileged_network_card::privileged_network_card(state, &lang))
        .push(Space::new().height(10))
        .push(sniffer_section)
        .push(Space::new().height(10))
        .push(editor_section)
        .push(Space::new().height(10))
        .push(admin_section)
        .push(Space::new().height(10))
        .push(crate::view::web_dash_card::web_dash_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::apply_guard_card::apply_guard_card(
            state, &lang,
        ))
        .push(Space::new().height(10))
        .push(crate::view::doctor::section(state))
        .push(Space::new().height(10))
        .push(hotkeys::hotkeys_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::geodata_card::geodata_card(state, &lang))
        .push(Space::new().height(10))
        .push(crate::view::uwp_card::uwp_card(state, &lang))
        .push(Space::new().height(10))
        .push(kernel::kernel_management_card(
            state,
            &lang,
            is_en,
            selected_core_channel,
        ))
        .push(Space::new().height(40));

    modern_scrollable(content).height(Length::Fill).into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_settings_tests.rs"]
mod tests;
