//! DUAL-14-09 (re-scoped): the Iced STUN UDP-egress panel.
//!
//! The panel renders the shared `StunProbeReport` verbatim: the configured
//! server, the typed status and, when the server answered, the public UDP
//! mapping (IP:port) it observed. The comparison against the expected proxied
//! egress is a fact comparison, never a leak verdict. A host without a prober
//! renders the typed unsupported state.
//!
//! **Honest boundary, rendered on the panel itself:** this is the host /
//! process's own UDP egress mapping as seen by a STUN server, not a browser
//! WebRTC traversal result.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, text_btn};
use crate::view::components::{BadgeKind, badge};
use crate::view::theme;
use crate::view::theme::{SP_XS, tokens};
use iced::widget::{Space, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::dns_observation_projection::DnsObservationTone;
use infiltrator_application::stun_projection::project_stun;
use infiltrator_shared::locales::{Lang, Localizer};

/// The STUN UDP-egress card.
pub(crate) fn stun_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let report = &state.editor.dns_stun;
    let probing = state.diag.is_probing_stun;
    let display = project_stun(report, lang.0);
    let status = display.status;
    let kind = match display.tone {
        DnsObservationTone::Neutral => BadgeKind::Neutral,
        DnsObservationTone::Success => BadgeKind::Success,
        DnsObservationTone::Warning => BadgeKind::Warning,
        DnsObservationTone::Danger => BadgeKind::Danger,
    };
    let comparison = display.comparison;
    let server = display.server;

    let body = column![
        text(lang.tr("dns_stun_probe_desc").to_string())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        row![
            badge(status, kind),
            Space::new().width(Length::Fill),
            text_btn(
                if probing {
                    lang.tr("dns_stun_probing").to_string()
                } else {
                    lang.tr("dns_stun_btn_run").to_string()
                },
                style_accent,
                (!probing).then_some(Message::RunStunProbe),
            ),
        ]
        .align_y(Alignment::Center),
        text(server)
            .size(11)
            .font(theme::MONO)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        text(comparison).size(12),
        text(display.boundary)
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
    ]
    .spacing(SP_XS);

    card(Some(lang.tr("dns_stun_probe_title").to_string()), body)
}

#[cfg(test)]
#[path = "../../tests/gui/view_stun_probe_tests.rs"]
mod tests;
