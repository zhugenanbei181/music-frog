//! TUN Multi-Stack Performance Selector & MTU Negotiator component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{BadgeKind, badge};
use crate::view::svg_icons::Icon;
use crate::view::theme::tokens;
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::mtu::{MtuNegotiationSnapshot, MtuProbeState};
use infiltrator_contract::tun::TunStack;
use infiltrator_presentation::tun_stack_options::{TUN_STACK_CHOICES, tun_stack_copy_key};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn tun_stack_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let tun_cfg = &state.runtime.tun_stack_config;

    let probe_btn = button(
        row![
            svg_icons::icon_themed(Icon::Activity, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            theme::role_body_text(lang.tr("tun_mtu_probe_btn").to_string()),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_accent)
    .on_press_maybe((!tun_cfg.is_probing_mtu).then_some(Message::ProbeOptimalMtu));

    let active_stack = TunStack::parse(&tun_cfg.active_stack).unwrap_or_default();

    let mut stack_pills = row![].spacing(theme::SP_XS).align_y(Alignment::Center);
    for choice in TUN_STACK_CHOICES {
        let active = active_stack == choice.stack;
        let pill = button(theme::role_body_text(
            lang.tr(tun_stack_copy_key(choice.stack)).to_string(),
        ))
        .padding([4, 8])
        .style(if active { style_accent } else { style_ghost })
        .on_press_maybe(
            choice
                .selectable
                .then(|| Message::SetTunStack(choice.wire_name.to_owned())),
        );
        stack_pills = stack_pills.push(pill);
    }
    let stack_pills = stack_pills;

    let mtu_val = state
        .runtime
        .mtu
        .tun_mtu
        .unwrap_or(tun_cfg.negotiated_mtu.max(1420));
    let mtu_status = format_mtu_status(&state.runtime.mtu);

    let mtu_row = row![
        theme::role_body_text(format!("Negotiated MTU: {mtu_val} bytes")).width(Length::Fill),
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
                theme::role_body_text(msg.clone()).style(|t: &Theme| text::Style {
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
            theme::role_body_text(lang.tr("tun_stack_desc").to_string()).style(|t: &Theme| {
                text::Style {
                    color: Some(tokens(t).text_secondary),
                }
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
