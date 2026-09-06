//! Network Interface Roaming and Gateway Recovery component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, style_accent, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::network_roaming::{NetworkRoamingEvent, NetworkRoamingStatus};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn net_roam_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let roam = &state.runtime.network_roaming;

    let reconnect_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("net_roam_btn_reconnect").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent);
    let reconnect_btn = if matches!(
        &roam.status,
        NetworkRoamingStatus::Unsupported { .. } | NetworkRoamingStatus::Unknown
    ) {
        reconnect_btn
    } else {
        reconnect_btn.on_press(Message::ForceGatewayReconnect)
    };

    let active_iface = roam.active_interface.as_deref().unwrap_or("—");
    let gateway = roam.default_gateway.as_deref().unwrap_or("—");
    let mtu = roam
        .physical_mtu
        .map(|physical| {
            format!(
                "physical {physical} → TUN {} · MSS {}",
                roam.recommended_tun_mtu.unwrap_or_default(),
                roam.tcp_mss.unwrap_or_default()
            )
        })
        .unwrap_or_else(|| "—".to_owned());
    let status = format_status(&roam.status);

    let details_row = row![
        column![
            text(lang.tr("net_roam_active_iface").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            row![
                badge(active_iface.to_string(), BadgeKind::Accent),
                Space::new().width(theme::SP_XS),
                badge("Active".to_string(), BadgeKind::Success),
            ].align_y(Alignment::Center),
        ].width(Length::Fill),
        column![
            text(lang.tr("net_roam_gateway").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(gateway).size(13).font(MONO),
        ].width(Length::Fill),
        column![
            text(lang.tr("net_roam_mtu").to_string()).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(2.0),
            text(mtu).size(13).font(MONO),
        ].width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let event_feedback: Element<'_, Message> = if let Some(ev) = &roam.last_event {
        container(
            row![
                svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(format_event(ev)).size(11).style(|t: &Theme| text::Style { color: Some(tokens(t).success) }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("net_roam_title").to_string()),
        column![
            text(lang.tr("net_roam_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            text(status).size(11).font(MONO).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }),
            Space::new().height(theme::SP_XS),
            details_row,
            Space::new().height(theme::SP_XS),
            row![
                event_feedback,
                Space::new().width(Length::Fill),
                reconnect_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}

fn format_status(status: &NetworkRoamingStatus) -> String {
    match status {
        NetworkRoamingStatus::Unknown => "未探测".to_owned(),
        NetworkRoamingStatus::Stable => "链路稳定".to_owned(),
        NetworkRoamingStatus::Recovering => "正在修复 TUN 路由".to_owned(),
        NetworkRoamingStatus::Degraded { reason } => format!("降级 · {reason}"),
        NetworkRoamingStatus::Unsupported { reason } => format!("宿主不支持 · {reason}"),
        NetworkRoamingStatus::Failed { failure } => format!("修复失败 · {}", failure.message),
    }
}

fn format_event(event: &NetworkRoamingEvent) -> String {
    match event {
        NetworkRoamingEvent::InitialObservation {
            interface,
            gateway_ip,
        } => format!(
            "已观测 {} / {}",
            interface.as_deref().unwrap_or("—"),
            gateway_ip.as_deref().unwrap_or("—")
        ),
        NetworkRoamingEvent::GatewayChanged {
            old_interface,
            new_interface,
            old_gateway_ip,
            new_gateway_ip,
        } => format!(
            "网关切换 {} → {} ({} → {})",
            old_interface.as_deref().unwrap_or("—"),
            new_interface.as_deref().unwrap_or("—"),
            old_gateway_ip.as_deref().unwrap_or("—"),
            new_gateway_ip.as_deref().unwrap_or("—")
        ),
        NetworkRoamingEvent::InterfaceAddressChanged { interface } => {
            format!("地址变化 · {interface}")
        }
        NetworkRoamingEvent::RoutesRepaired {
            physical_interface,
            tun_interface,
            detail,
        } => format!("路由已修复 · {physical_interface} → {tun_interface} · {detail}"),
        NetworkRoamingEvent::RepairSkipped { reason } => format!("未修复 · {reason}"),
        NetworkRoamingEvent::RepairFailed { failure } => format!("修复失败 · {}", failure.message),
    }
}
