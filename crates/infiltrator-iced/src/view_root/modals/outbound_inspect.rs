//! Product outbound inspection rendered only from typed details/probe observations.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::state::proxy_probe::OutboundProbeActionAvailability;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::Lang;
use infiltrator_shared::locales::Localizer;
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_contract::outbound::{
    details::{
        OutboundDetail, OutboundHealthObservation, OutboundObservation,
        OutboundProtocolObservation, OutboundRttStatistics, OutboundUdpSupport,
    },
    identity::OutboundCipherId,
    inventory::OutboundProbeState,
};

pub fn inspect_outbound_modal<'a>(
    state: &'a AppState,
    outbound_id: &'a OutboundId,
) -> Element<'a, Message> {
    let member = state
        .runtime
        .proxy_groups
        .iter()
        .flat_map(|group| &group.proxies)
        .find(|member| member.id.as_ref() == Some(outbound_id));
    let name = member
        .map(|member| member.search_name().to_owned())
        .unwrap_or_else(|| outbound_id.value().to_owned());
    let detail = state.outbound_detail(outbound_id);
    let lang = Lang(&state.shell.lang);
    let (status, facts) = if let Some(detail) = detail.as_ref() {
        (String::new(), detail_rows(detail))
    } else {
        (details_status(state), Vec::new())
    };
    let mut body = column![].spacing(10);
    if !status.is_empty() {
        body = body.push(text(status).size(13));
    }
    for (label, value) in facts {
        body = body.push(
            row![
                text(label).size(12).width(Length::FillPortion(1)),
                text(value)
                    .size(12)
                    .font(MONO)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(12),
        );
    }
    let history = detail
        .as_ref()
        .map(|detail| {
            observation_list(&detail.latency_history, |sample| {
                format!("{} · {} ms", sample.observed_at, sample.delay_ms)
            })
        })
        .unwrap_or_default();
    if !history.is_empty() {
        body = body.push(text("Latency history").size(13).font(FONT_SEMIBOLD));
        body = body.push(text(history).size(12).font(MONO));
    }
    let egress = detail
        .as_ref()
        .map(|detail| {
            observation_list(&detail.historical_egress, |sample| {
                format!(
                    "{}{}",
                    sample.ip,
                    sample
                        .country
                        .as_deref()
                        .map(|country| format!(" · {country}"))
                        .unwrap_or_default()
                )
            })
        })
        .unwrap_or_default();
    if !egress.is_empty() {
        body = body.push(text("Historical egress").size(13).font(FONT_SEMIBOLD));
        body = body.push(text(egress).size(12).font(MONO));
    }

    let probe_failure = state
        .runtime
        .outbound_probe
        .failure()
        .map(|failure| failure.message.clone());
    let probe_result = state
        .runtime
        .outbound_probe_measurement
        .as_ref()
        .filter(|result| result.outbound_id() == outbound_id)
        .map(|result| probe_state_label(result.state()))
        .unwrap_or_default();
    let probe_availability = state.outbound_probe_action_availability(outbound_id);
    let probe_text = probe_availability
        .status_text()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            probe_failure
                .or_else(|| (!probe_result.is_empty()).then_some(probe_result))
                .unwrap_or_default()
        });
    let can_probe = probe_availability == OutboundProbeActionAvailability::Available;
    let pending = state.runtime.outbound_probe.is_running();
    let actions = row![
        button(text(lang.tr("modal_close")))
            .style(style_ghost)
            .on_press(Message::InspectOutbound(None)),
        Space::new().width(Length::Fill),
        button(text(if pending {
            lang.tr("core_control_pending").into_owned()
        } else {
            lang.tr("modal_speed_test_now").into_owned()
        }))
        .style(style_accent)
        .on_press_maybe((can_probe && !pending).then_some(Message::ProbeInspectedOutbound)),
    ]
    .align_y(Alignment::Center)
    .spacing(12);
    let content = column![
        text(lang.tr("proxy_inspection_title").into_owned())
            .size(16)
            .font(FONT_SEMIBOLD),
        text(name).size(14).font(FONT_MEDIUM),
        text(probe_text)
            .size(12)
            .style(|theme: &Theme| text::Style {
                color: Some(tokens(theme).danger),
            }),
        scrollable(body).height(Length::Fixed(390.0)),
        actions,
    ]
    .spacing(12);
    let card: Element<'_, Message> = container(modal_card(content.into(), 560.0))
        .id(InteractionRegion::ProxyInspection.id())
        .into();
    modal_backdrop(card)
}

fn details_status(state: &AppState) -> String {
    use lurker_core_contract::page::PageStatus;
    match &state.runtime.outbound_details.status {
        PageStatus::Loading => "Outbound details are loading".to_owned(),
        PageStatus::Ready | PageStatus::Empty => "Outbound details are unknown".to_owned(),
        PageStatus::Unavailable { failure } | PageStatus::Failed { failure } => {
            format!("{:?}: {}", failure.code, failure.message)
        }
    }
}

fn detail_rows(detail: &OutboundDetail) -> Vec<(String, String)> {
    let protocol = match &detail.protocol {
        OutboundProtocolObservation::Unknown => "Unknown".to_owned(),
        OutboundProtocolObservation::Unsupported => "Unsupported".to_owned(),
        OutboundProtocolObservation::Reported(protocol) => detail
            .protocol_display_name
            .clone()
            .unwrap_or_else(|| protocol.value().to_owned()),
    };
    let cipher = observation_value(&detail.cipher, |cipher: &OutboundCipherId| {
        detail
            .cipher_display_name
            .clone()
            .unwrap_or_else(|| cipher.value().to_owned())
    });
    let endpoint = observation_value(&detail.endpoint, |endpoint| {
        format!(
            "{}:{}",
            endpoint.host,
            observation_value(&endpoint.port, |port| port.to_string())
        )
    });
    let latency = observation_value(&detail.current_latency_ms, |latency| {
        format!("{latency} ms")
    });
    let rtt = observation_value(&detail.rtt, rtt_label);
    vec![
        ("Protocol".into(), protocol),
        ("Cipher".into(), cipher),
        ("Endpoint".into(), endpoint),
        ("UDP".into(), udp_label(detail.udp)),
        ("Health".into(), health_label(detail.health)),
        ("Latency".into(), latency),
        ("RTT".into(), rtt),
    ]
}

fn observation_value<T>(
    observation: &OutboundObservation<T>,
    display: impl FnOnce(&T) -> String,
) -> String {
    match observation {
        OutboundObservation::Unknown => "Unknown".to_owned(),
        OutboundObservation::Unsupported => "Unsupported".to_owned(),
        OutboundObservation::Observed(value) => display(value),
    }
}

fn observation_list<T>(
    observation: &OutboundObservation<Vec<T>>,
    display: impl Fn(&T) -> String,
) -> String {
    match observation {
        OutboundObservation::Unknown => "Unknown".to_owned(),
        OutboundObservation::Unsupported => "Unsupported".to_owned(),
        OutboundObservation::Observed(values) if values.is_empty() => "Empty".to_owned(),
        OutboundObservation::Observed(values) => {
            values.iter().map(display).collect::<Vec<_>>().join("\n")
        }
    }
}

fn udp_label(udp: OutboundUdpSupport) -> String {
    match udp {
        OutboundUdpSupport::Unknown => "Unknown",
        OutboundUdpSupport::Supported => "Supported",
        OutboundUdpSupport::Unsupported => "Unsupported",
    }
    .to_owned()
}

fn health_label(health: OutboundHealthObservation) -> String {
    match health {
        OutboundHealthObservation::Unknown => "Unknown",
        OutboundHealthObservation::Healthy => "Healthy",
        OutboundHealthObservation::Unhealthy => "Unhealthy",
        OutboundHealthObservation::Unsupported => "Unsupported",
    }
    .to_owned()
}

fn rtt_label(stats: &OutboundRttStatistics) -> String {
    format!(
        "min {} · avg {} · max {} · {} samples",
        stats
            .min_ms
            .map_or_else(|| "Unknown".into(), |value| format!("{value} ms")),
        stats
            .avg_ms
            .map_or_else(|| "Unknown".into(), |value| format!("{value} ms")),
        stats
            .max_ms
            .map_or_else(|| "Unknown".into(), |value| format!("{value} ms")),
        stats.valid_count,
    )
}

fn probe_state_label(state: &OutboundProbeState) -> String {
    match state {
        OutboundProbeState::Unknown => "Probe result unknown".into(),
        OutboundProbeState::Probing => "Probe pending".into(),
        OutboundProbeState::Measured { delay_ms } => format!("{delay_ms} ms"),
        OutboundProbeState::TimedOut => "Probe timed out".into(),
        OutboundProbeState::Unsupported => "Probe unsupported".into(),
        OutboundProbeState::Failed { failure } => failure.message.clone(),
    }
}
