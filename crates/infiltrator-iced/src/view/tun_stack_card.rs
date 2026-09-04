//! TUN Multi-Stack Performance Selector & MTU Negotiator component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn tun_stack_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let tun_cfg = &state.runtime.tun_stack_config;

    let probe_btn = button(
        row![
            svg_icons::icon_themed(Icon::Activity, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("tun_mtu_probe_btn").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_accent)
    .on_press_maybe((!tun_cfg.is_probing_mtu).then_some(Message::ProbeOptimalMtu));

    let active_stack = if tun_cfg.active_stack.is_empty() {
        "gvisor"
    } else {
        &tun_cfg.active_stack
    };

    let stack_pills = row![
        button(text(lang.tr("tun_stack_gvisor").to_string()).size(11))
            .padding([4, 8])
            .style(if active_stack == "gvisor" { style_accent } else { style_ghost })
            .on_press(Message::SelectTunStack("gvisor".to_string())),
        Space::new().width(theme::SP_XS),
        button(text(lang.tr("tun_stack_system").to_string()).size(11))
            .padding([4, 8])
            .style(if active_stack == "system" { style_accent } else { style_ghost })
            .on_press(Message::SelectTunStack("system".to_string())),
        Space::new().width(theme::SP_XS),
        button(text(lang.tr("tun_stack_mixed").to_string()).size(11))
            .padding([4, 8])
            .style(if active_stack == "mixed" { style_accent } else { style_ghost })
            .on_press(Message::SelectTunStack("mixed".to_string())),
    ]
    .align_y(Alignment::Center);

    let mtu_val = if tun_cfg.negotiated_mtu == 0 {
        1420
    } else {
        tun_cfg.negotiated_mtu
    };

    let mtu_row = row![
        text(format!("Negotiated MTU: {mtu_val} bytes")).size(12).font(MONO).width(Length::Fill),
        badge(format!("Driver: {active_stack}"), BadgeKind::Accent),
    ]
    .align_y(Alignment::Center);

    let feedback: Element<'_, Message> = if let Some(msg) = &tun_cfg.probe_result_summary {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(msg.clone()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("tun_stack_title").to_string()),
        column![
            text(lang.tr("tun_stack_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            stack_pills,
            Space::new().height(theme::SP_XS),
            mtu_row,
            feedback,
            Space::new().height(theme::SP_XS),
            row![
                Space::new().width(Length::Fill),
                probe_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}
