//! TUN mode card.

use super::integration::secondary_text;
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{
    form_input_style, form_toggle_row, responsive_form_row,
    responsive_form_toggle_row_with_actions, style_ghost, text_btn,
};
use crate::view::components::{BadgeKind, badge, icon_button, segmented_control_with_actions};
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{MONO, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::settings_status_projection::format_service_mode;
use infiltrator_contract::runtime_control::RuntimeControlStatus;
use infiltrator_ports::host_runtime::TunServiceStatus;
use infiltrator_shared::locales::{Lang, Localizer};

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
    let observed = &state.runtime.runtime_control;
    let available = observed.status == RuntimeControlStatus::Ready
        && state.runtime.pending_runtime_patch.is_none();
    let stack_available = available && observed.tun_stack.is_some();
    let routes_available =
        available && observed.tun_auto_route.is_some() && observed.tun_strict_route.is_some();
    let ipv6_available = available && observed.ipv6_routing.is_some();
    let current_stack_index = observed
        .tun_stack
        .as_deref()
        .and_then(|value| {
            ["gvisor", "mixed", "system"]
                .iter()
                .position(|stack| value.eq_ignore_ascii_case(stack))
        })
        .unwrap_or(usize::MAX);
    let tun_stack_selector =
        segmented_control_with_actions(&stack_options, current_stack_index, move |index| {
            let stack = match index {
                1 => "mixed",
                2 => "system",
                _ => "gvisor",
            };
            stack_available.then(|| Message::SetTunStack(stack.to_string()))
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
    let service_mode = format_service_mode(&state.runtime.service_mode, lang.0);

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
                text(lang.tr("settings_service_mode_title").into_owned())
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                secondary_text(service_mode),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            responsive_form_row(
                state.shell.viewport.tier,
                lang.tr("tun_stack").to_string(),
                None::<&str>,
                tun_stack_selector,
            ),
            responsive_form_row(
                state.shell.viewport.tier,
                "MTU",
                None::<&str>,
                text_input("1500", &state.editor.tun_form.mtu)
                    .on_input(Message::UpdateTunFormMtu)
                    .width(if state.shell.viewport.tier.is_compact() {
                        Length::Fill
                    } else {
                        Length::Fixed(120.0)
                    })
                    .padding([6, 10])
                    .size(12)
                    .font(MONO)
                    .style(form_input_style),
            ),
            responsive_form_toggle_row_with_actions(
                state.shell.viewport.tier,
                lang.tr("tun_auto_route").to_string(),
                None::<&str>,
                auto_route,
                move |value| routes_available.then_some(Message::SetTunAutoRoute(value)),
            ),
            responsive_form_toggle_row_with_actions(
                state.shell.viewport.tier,
                lang.tr("tun_strict_route").to_string(),
                None::<&str>,
                strict_route,
                move |value| routes_available.then_some(Message::SetTunStrictRoute(value)),
            ),
            responsive_form_toggle_row_with_actions(
                state.shell.viewport.tier,
                lang.tr("settings_ipv6_routing").to_string(),
                Some(lang.tr("settings_ipv6_routing_desc").to_string()),
                state.runtime.ipv6_routing.enabled,
                move |value| ipv6_available.then_some(Message::SetIpv6Routing(value)),
            ),
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
