//! DUAL-14-08: the Iced DNS leak cross-source panel.
//!
//! The panel renders the shared `DnsLeakReport` verbatim: one row per
//! configured probe source with the identity the authority observed, and the
//! typed conclusion of the comparison. A host without a fact source renders
//! the typed unsupported state — there is no country, ISP or leak verdict to
//! invent (the old hardcoded `country`/`isp` values are gone).

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, text_btn};
use crate::view::components::{BadgeKind, badge};
use crate::view::theme;
use crate::view::theme::{MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::dns_leak_projection::{LeakDisplay, LeakTone, project_leak};
use infiltrator_contract::dns_leak::DnsLeakOperation;
#[cfg(test)]
use infiltrator_contract::dns_leak::DnsLeakReport;
use infiltrator_shared::locales::{Lang, Localizer};

fn badge_kind(tone: LeakTone) -> BadgeKind {
    match tone {
        LeakTone::Neutral => BadgeKind::Neutral,
        LeakTone::Success => BadgeKind::Success,
        LeakTone::Warning => BadgeKind::Warning,
        LeakTone::Danger => BadgeKind::Danger,
    }
}
#[cfg(test)]
pub(crate) fn leak_conclusion_copy(report: &DnsLeakReport, lang: &Lang<'_>) -> (String, BadgeKind) {
    let display = project_leak(report, lang.0);
    (display.conclusion, badge_kind(display.tone))
}
#[cfg(test)]
pub(crate) fn leak_observation_lines<'a>(
    report: &DnsLeakReport,
    lang: &Lang<'_>,
) -> Vec<Element<'a, Message>> {
    observation_rows(&project_leak(report, lang.0))
}
fn observation_rows<'a>(display: &LeakDisplay) -> Vec<Element<'a, Message>> {
    display
        .rows
        .iter()
        .map(|observation| {
            let tone = observation.tone;
            column![
                text(format!(
                    "{} → {}",
                    observation.resolver, observation.authority
                ))
                .size(12)
                .font(MONO),
                text(observation.outcome.clone())
                    .size(12)
                    .style(move |theme: &Theme| text::Style {
                        color: Some(match tone {
                            LeakTone::Neutral => tokens(theme).text_secondary,
                            LeakTone::Success => tokens(theme).success,
                            LeakTone::Warning => tokens(theme).warning,
                            LeakTone::Danger => tokens(theme).danger,
                        })
                    }),
            ]
            .spacing(4)
            .into()
        })
        .collect()
}

/// The DNS leak cross-source card.
pub(crate) fn leak_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let report = &state.editor.dns_leak;
    let probing =
        state.diag.is_probing_dns_leak || matches!(report.operation, DnsLeakOperation::Running);
    let display = project_leak(report, lang.0);
    let kind = badge_kind(display.tone);

    let mut body = column![
        row![
            text(lang.tr("dns_leak_probe_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                })
                .width(Length::Fill),
            container(text_btn(
                if probing {
                    lang.tr("dns_leak_probing").to_string()
                } else {
                    lang.tr("dns_leak_btn_run").to_string()
                },
                style_accent,
                (!probing).then_some(Message::RunDnsLeakProbe),
            ))
            .id(InteractionRegion::DnsLeakRun.id()),
        ]
        .align_y(Alignment::Center),
        Space::new().height(theme::SP_XS),
        row![
            badge(display.conclusion.clone(), kind),
            Space::new().width(Length::Fill),
            text(display.sources.clone())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_XS);
    if let Some(feedback) = &display.feedback {
        body = body.push(text(feedback.clone()).size(12));
    }
    if let Some(failure) = &state.diag.dns_leak_action.failure {
        body = body.push(text(failure.message.clone()).size(12));
    }
    if state.diag.dns_leak_action.can_retry() {
        body = body.push(text_btn(
            lang.tr("dns_leak_btn_retry").into_owned(),
            style_accent,
            Some(Message::RetryDnsLeakProbe),
        ));
    }
    for line in observation_rows(&display) {
        body = body.push(line);
    }

    container(card(
        Some(lang.tr("dns_leak_probe_title").to_string()),
        body,
    ))
    .id(InteractionRegion::DnsLeakCard.id())
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_dns_leak_tests.rs"]
mod tests;
