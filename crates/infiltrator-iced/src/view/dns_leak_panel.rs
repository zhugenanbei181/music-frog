//! DUAL-14-08: the Iced DNS leak cross-source panel.
//!
//! The panel renders the shared `DnsLeakReport` verbatim: one row per
//! configured probe source with the identity the authority observed, and the
//! typed conclusion of the comparison. A host without a fact source renders
//! the typed unsupported state — there is no country, ISP or leak verdict to
//! invent (the old hardcoded `country`/`isp` values are gone).

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card, style_accent, text_btn};
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::dns_leak::{DnsLeakConclusion, DnsLeakObservationOutcome, DnsLeakReport};
use infiltrator_shared::locales::{Lang, Localizer};

/// The typed conclusion heading both locales render.
pub(crate) fn leak_conclusion_copy(report: &DnsLeakReport, lang: &Lang<'_>) -> (String, BadgeKind) {
    match report.conclusion() {
        DnsLeakConclusion::Consistent { identity, facts } => (
            lang.tr("dns_leak_consistent")
                .replace("{count}", &facts.len().to_string())
                .replace("{identity}", &identity),
            BadgeKind::Success,
        ),
        DnsLeakConclusion::Divergent { facts } => {
            let identities: Vec<&str> = facts
                .iter()
                .map(|fact| fact.identity.as_str())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            (
                lang.tr("dns_leak_divergent")
                    .replace("{count}", &identities.len().to_string()),
                BadgeKind::Danger,
            )
        }
        DnsLeakConclusion::Unsupported { reason } => (
            lang.tr("dns_leak_unsupported").replace("{reason}", &reason),
            BadgeKind::Neutral,
        ),
        DnsLeakConclusion::Failed { reason } => (
            lang.tr("dns_leak_failed").replace("{reason}", &reason),
            BadgeKind::Warning,
        ),
        DnsLeakConclusion::Unknown => (lang.tr("dns_leak_unknown").to_string(), BadgeKind::Neutral),
    }
}

/// One localized row per attempted source, with the real outcome.
pub(crate) fn leak_observation_lines<'a>(
    report: &DnsLeakReport,
    lang: &Lang<'_>,
) -> Vec<Element<'a, Message>> {
    report
        .observations
        .iter()
        .map(|observation| {
            let outcome = match &observation.outcome {
                DnsLeakObservationOutcome::Observed { identity } => {
                    lang.tr("dns_leak_observed").replace("{identity}", identity)
                }
                DnsLeakObservationOutcome::TimedOut => lang.tr("dns_latency_timeout").to_string(),
                DnsLeakObservationOutcome::InvalidResponse { reason } => lang
                    .tr("dns_latency_invalid_response")
                    .replace("{reason}", reason),
                DnsLeakObservationOutcome::Failed { message } => {
                    lang.tr("dns_latency_failed").replace("{reason}", message)
                }
                DnsLeakObservationOutcome::NotProbed { reason } => lang
                    .tr("dns_latency_not_probed")
                    .replace("{reason}", reason),
            };
            let color = match &observation.outcome {
                DnsLeakObservationOutcome::Observed { .. } => COLOR_OBSERVED,
                _ => COLOR_NOT_OBSERVED,
            };
            row![
                text(format!(
                    "{} → {}",
                    observation.resolver, observation.authority
                ))
                .size(12)
                .font(MONO)
                .width(Length::Fill),
                text(outcome.to_string())
                    .size(12)
                    .style(move |_: &Theme| text::Style { color: Some(color) }),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect()
}

const COLOR_OBSERVED: iced::Color = iced::Color::from_rgb(0.20, 0.70, 0.42);
const COLOR_NOT_OBSERVED: iced::Color = iced::Color::from_rgb(0.85, 0.33, 0.33);

/// The DNS leak cross-source card.
pub(crate) fn leak_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let report = &state.editor.dns_leak;
    let probing = state.diag.is_probing_dns_leak;
    let (conclusion, kind) = leak_conclusion_copy(report, lang);
    let source_count = report.sources.len();

    let mut body = column![
        row![
            text(lang.tr("dns_leak_probe_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                })
                .width(Length::Fill),
            text_btn(
                if probing {
                    lang.tr("dns_leak_probing").to_string()
                } else {
                    lang.tr("dns_leak_btn_run").to_string()
                },
                style_accent,
                (!probing).then_some(Message::RunDnsLeakProbe),
            ),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        row![
            badge(conclusion, kind),
            Space::new().width(Length::Fill),
            text(
                lang.tr("dns_leak_sources")
                    .replace("{count}", &source_count.to_string())
            )
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_XS);
    for line in leak_observation_lines(report, lang) {
        body = body.push(line);
    }

    card(Some(lang.tr("dns_leak_probe_title").to_string()), body)
}

#[cfg(test)]
#[path = "../../tests/gui/view_dns_leak_tests.rs"]
mod tests;
