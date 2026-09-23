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
use crate::view::components::{BadgeKind, badge, card, style_accent, text_btn};
use crate::view::theme::{self, SP_XS, tokens};
use iced::widget::{Space, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::stun_probe::{StunEgressComparison, StunProbeReport, StunProbeStatus};
use infiltrator_shared::locales::{Lang, Localizer};

/// The typed status heading both locales render.
pub(crate) fn stun_status_copy(report: &StunProbeReport, lang: &Lang<'_>) -> (String, BadgeKind) {
    match &report.status {
        StunProbeStatus::Observed => {
            let mapping = report
                .mapping()
                .map(|mapping| mapping.display())
                .unwrap_or_default();
            (
                lang.tr("dns_stun_observed").replace("{mapping}", &mapping),
                BadgeKind::Success,
            )
        }
        StunProbeStatus::TimedOut => (
            lang.tr("dns_stun_timed_out").to_string(),
            BadgeKind::Warning,
        ),
        StunProbeStatus::Failed { message } => (
            lang.tr("dns_stun_failed").replace("{reason}", message),
            BadgeKind::Danger,
        ),
        StunProbeStatus::Unsupported { reason } => (
            lang.tr("dns_stun_unsupported").replace("{reason}", reason),
            BadgeKind::Neutral,
        ),
        StunProbeStatus::Unknown => (lang.tr("dns_stun_unknown").to_string(), BadgeKind::Neutral),
    }
}

/// The comparison line: consistent / divergent facts / unknown, never a leak
/// verdict.
pub(crate) fn stun_comparison_copy(report: &StunProbeReport, lang: &Lang<'_>) -> String {
    match report.comparison() {
        StunEgressComparison::Consistent => lang.tr("dns_stun_consistent").to_string(),
        StunEgressComparison::Divergent => {
            let observed = report
                .mapping()
                .map(|mapping| mapping.display())
                .unwrap_or_default();
            let expected = report.expected_egress_ip().unwrap_or_default();
            lang.tr("dns_stun_divergent")
                .replace("{observed}", &observed)
                .replace("{expected}", expected)
        }
        StunEgressComparison::Unknown { reason } => lang
            .tr("dns_stun_unknown_comparison")
            .replace("{reason}", &reason),
    }
}

/// The STUN UDP-egress card.
pub(crate) fn stun_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let report = &state.editor.dns_stun;
    let probing = state.diag.is_probing_stun;
    let (status, kind) = stun_status_copy(report, lang);
    let comparison = stun_comparison_copy(report, lang);
    let server = lang
        .tr("dns_stun_server")
        .replace("{server}", &report.server);

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
        text(lang.tr("dns_stun_not_webrtc").to_string())
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
