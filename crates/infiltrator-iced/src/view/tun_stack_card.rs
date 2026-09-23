//! TUN Multi-Stack Performance Selector & MTU Negotiator component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{BadgeKind, badge, card};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState};
use infiltrator_contract::tun::TunStack;
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

    let active_stack = TunStack::parse(&tun_cfg.active_stack).unwrap_or_default();

    let stack_pills = row![
        button(text(lang.tr("tun_stack_gvisor").to_string()).size(11))
            .padding([4, 8])
            .style(if active_stack == TunStack::Gvisor {
                style_accent
            } else {
                style_ghost
            })
            .on_press(Message::SetTunStack(TunStack::Gvisor.as_str().to_owned())),
        Space::new().width(theme::SP_XS),
        button(text(lang.tr("tun_stack_system").to_string()).size(11))
            .padding([4, 8])
            .style(if active_stack == TunStack::System {
                style_accent
            } else {
                style_ghost
            })
            .on_press(Message::SetTunStack(TunStack::System.as_str().to_owned())),
        Space::new().width(theme::SP_XS),
        button(text(lang.tr("tun_stack_mixed").to_string()).size(11))
            .padding([4, 8])
            .style(if active_stack == TunStack::Mixed {
                style_accent
            } else {
                style_ghost
            })
            .on_press(Message::SetTunStack(TunStack::Mixed.as_str().to_owned())),
        Space::new().width(theme::SP_XS),
        button(text("LWIP (Reference-only)").size(11))
            .padding([4, 8])
            .style(style_ghost)
            .on_press_maybe(
                TunStack::Lwip
                    .is_live_supported()
                    .then_some(Message::SetTunStack(TunStack::Lwip.as_str().to_owned()))
            ),
    ]
    .align_y(Alignment::Center);

    let mtu_val = state
        .runtime
        .mtu
        .tun_mtu
        .unwrap_or(tun_cfg.negotiated_mtu.max(1420));
    let mtu_status = format_mtu_status(&state.runtime.mtu);

    let mtu_row = row![
        text(format!("Negotiated MTU: {mtu_val} bytes"))
            .size(12)
            .font(MONO)
            .width(Length::Fill),
        badge(
            format!("Driver: {}", active_stack.as_str()),
            BadgeKind::Accent
        ),
        badge(mtu_status, BadgeKind::Neutral),
    ]
    .align_y(Alignment::Center);

    let feedback: Element<'_, Message> = if let Some(msg) = &tun_cfg.probe_result_summary {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(msg.clone()).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success)
                }),
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
            text(lang.tr("tun_stack_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().height(theme::SP_XS),
            stack_pills,
            Space::new().height(theme::SP_XS),
            mtu_row,
            feedback,
            Space::new().height(theme::SP_XS),
            row![Space::new().width(Length::Fill), probe_btn,].align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}

fn format_mtu_status(snapshot: &MtuNegotiationSnapshot) -> String {
    match &snapshot.state {
        MtuProbeState::Unknown => "MTU: not probed".to_owned(),
        MtuProbeState::Probing => "MTU: probing".to_owned(),
        MtuProbeState::Ready => snapshot.physical_interface.as_deref().map_or_else(
            || "MTU: negotiated".to_owned(),
            |name| {
                if snapshot.applied_tun_mtu == snapshot.tun_mtu {
                    format!("MTU: {name} / applied")
                } else {
                    format!("MTU: {name} / pending")
                }
            },
        ),
        MtuProbeState::Unsupported => "MTU: unsupported".to_owned(),
        MtuProbeState::Failed { .. } => "MTU: failed".to_owned(),
    }
}
