//! TUN mode card.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, form_input_style, form_toggle_row, icon_button, segmented_control,
    style_ghost, text_btn,
};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_shared::locales::{Lang, Localizer};

use super::format::format_service_mode;
use super::integration::secondary_text;

pub(super) fn tun_card<'a>(
    state: &'a AppState,
    lang: &Lang<'a>,
    _is_en: bool,
) -> Element<'a, Message> {
    let (tun_status_text, tun_status_kind) = match state.runtime.tun_service_status {
        Some(TunServiceStatus::InstalledAndRunning) => (
            lang.tr("settings_status_running").to_string(),
            BadgeKind::Success,
        ),
        Some(TunServiceStatus::InstalledStopped) => (
            lang.tr("settings_status_stopped").to_string(),
            BadgeKind::Warning,
        ),
        Some(TunServiceStatus::MissingPrivilege) => (
            lang.tr("settings_status_no_perm").to_string(),
            BadgeKind::Danger,
        ),
        Some(TunServiceStatus::Unsupported) => (
            lang.tr("settings_status_unsupported").to_string(),
            BadgeKind::Neutral,
        ),
        Some(TunServiceStatus::NotInstalled) => (
            lang.tr("settings_status_uninstalled").to_string(),
            BadgeKind::Neutral,
        ),
        None => (
            lang.tr("settings_status_undetected").to_string(),
            BadgeKind::Neutral,
        ),
    };

    let stack_options = vec![
        "gVisor".to_string(),
        "Mixed".to_string(),
        "System".to_string(),
    ];
    let current_stack_index = if state.editor.tun_stack.eq_ignore_ascii_case("mixed")
        || state.editor.tun_form.stack.eq_ignore_ascii_case("mixed")
    {
        1
    } else if state.editor.tun_stack.eq_ignore_ascii_case("system")
        || state.editor.tun_form.stack.eq_ignore_ascii_case("system")
    {
        2
    } else {
        0
    };
    let tun_stack_selector = segmented_control(&stack_options, current_stack_index, |index| {
        let stack_str = match index {
            1 => "mixed",
            2 => "system",
            _ => "gvisor",
        };
        Message::SetTunStack(stack_str.to_string())
    });

    let dns_hijack_active = !state.editor.tun_form.dns_hijack.trim().is_empty();
    let auto_route = if state.runtime.runtime.is_some() {
        state.editor.tun_auto_route
    } else {
        state.editor.tun_form.auto_route
    };
    let strict_route = if state.runtime.runtime.is_some() {
        state.editor.tun_strict_route
    } else {
        state.editor.tun_form.strict_route
    };
    let service_mode = format_service_mode(&state.runtime.service_mode);

    card(
        Some(lang.tr("tun_mode").to_string()),
        column![
            row![
                text(lang.tr("settings_tun_service_status").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(theme::SP_MD),
                badge(
                    tun_status_text,
                    if state.runtime.is_installing_tun_service {
                        BadgeKind::Warning
                    } else {
                        tun_status_kind
                    }
                ),
                Space::new().width(Length::Fill),
                icon_button(Icon::RefreshCw, 14.0, Message::RefreshTunServiceStatus),
                Space::new().width(theme::SP_SM),
                text_btn(
                    if state.runtime.is_installing_tun_service {
                        lang.tr("settings_tun_preparing").to_string()
                    } else {
                        lang.tr("settings_tun_prepare_btn").to_string()
                    },
                    style_ghost,
                    (!state.runtime.is_installing_tun_service)
                        .then_some(Message::InstallTunService)
                ),
            ]
            .align_y(Alignment::Center),
            row![
                text("Service mode")
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                secondary_text(service_mode),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                text(lang.tr("tun_stack").to_string())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                tun_stack_selector,
            ]
            .align_y(Alignment::Center),
            row![
                text("MTU").size(13).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary)
                }),
                Space::new().width(Length::Fill),
                text_input("1500", &state.editor.tun_form.mtu)
                    .on_input(Message::UpdateTunFormMtu)
                    .width(Length::Fixed(120.0))
                    .padding([6, 10])
                    .size(12)
                    .font(MONO)
                    .style(form_input_style),
            ]
            .align_y(Alignment::Center),
            form_toggle_row(
                lang.tr("tun_auto_route").to_string(),
                auto_route,
                Message::SetTunAutoRoute
            ),
            form_toggle_row(
                lang.tr("tun_strict_route").to_string(),
                strict_route,
                Message::SetTunStrictRoute
            ),
            form_toggle_row(
                lang.tr("settings_ipv6_routing").to_string(),
                state.runtime.ipv6_routing.enabled,
                Message::SetIpv6Routing,
            ),
            secondary_text(lang.tr("settings_ipv6_routing_desc").to_string()),
            form_toggle_row(
                lang.tr("settings_dns_hijack").to_string(),
                dns_hijack_active,
                |on| Message::UpdateTunFormDnsHijack(if on {
                    "any:53".to_string()
                } else {
                    String::new()
                })
            ),
        ]
        .spacing(theme::SP_SM),
    )
}
