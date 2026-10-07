//! Iced DNS workbench extras (DUAL-14-06 / 14-10 / 14-11).
//!
//! Both native surfaces replay application projections; the Hosts launcher
//! reads the shared editor and its independently owned root-level mapping.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{form_input_style, row_card_surface, style_accent, text_btn};
use crate::view::components::{BadgeKind, badge};
use crate::view::theme;
use crate::view::theme::{MONO, tokens};
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::dns_health_projection::project_dns_health;
use infiltrator_application::dns_hosts_projection::{feedback, summary};
use infiltrator_application::dns_latency_projection::{DnsLatencyRow, project_dns_latency};
use infiltrator_application::dns_mapping_projection::project_mappings;
use infiltrator_application::dns_observation_projection::DnsObservationTone;
use infiltrator_contract::dns_latency::DnsLatencyReport;
use infiltrator_contract::dns_self_heal::DnsSelfHealSnapshot;
use infiltrator_shared::locales::{Lang, Localizer};

/// DUAL-14-10: the localized per-server result line of the last real probe.
pub(crate) fn latency_result_lines<'a>(rows: Vec<DnsLatencyRow>) -> Vec<Element<'a, Message>> {
    rows.into_iter()
        .map(|result| {
            let tone = result.tone;
            row![
                text(result.address).size(12).font(MONO).width(Length::Fill),
                text(format!("{} · ", result.tier)).size(11),
                text(result.outcome)
                    .size(12)
                    .style(move |t: &Theme| text::Style {
                        color: Some(match tone {
                            DnsObservationTone::Success => tokens(t).success,
                            DnsObservationTone::Warning => tokens(t).warning,
                            DnsObservationTone::Danger => tokens(t).danger,
                            DnsObservationTone::Neutral => tokens(t).text_tertiary,
                        }),
                    }),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect()
}

fn observation_badge(tone: DnsObservationTone) -> BadgeKind {
    match tone {
        DnsObservationTone::Success => BadgeKind::Success,
        DnsObservationTone::Warning => BadgeKind::Warning,
        DnsObservationTone::Danger => BadgeKind::Danger,
        DnsObservationTone::Neutral => BadgeKind::Neutral,
    }
}

/// DUAL-14-10: the honest per-nameserver latency policy line and results.
pub(crate) fn latency_policy_line<'a>(
    report: &DnsLatencyReport,
    on_probe: Option<Message>,
    probing: bool,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let display = project_dns_latency(report, lang.0);
    let label = display.summary;
    let kind = observation_badge(display.tone);
    let mut body = column![
        row![
            text(lang.tr("dns_latency_title").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().width(theme::SP_SM),
            badge(label.to_string(), kind),
            Space::new().width(Length::Fill),
            text_btn(
                if probing {
                    lang.tr("dns_latency_probing").to_string()
                } else {
                    lang.tr("dns_latency_run").to_string()
                },
                style_accent,
                (!probing).then_some(on_probe).flatten(),
            ),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_XS);
    for line in latency_result_lines(display.rows) {
        body = body.push(line);
    }
    body.into()
}

/// DUAL-14-13: the shared DNS self-heal observation, one row per check.
pub(crate) fn self_heal_panel<'a>(
    snapshot: &DnsSelfHealSnapshot,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let display = project_dns_health(snapshot, lang.0);
    let overall = display.overall.clone();
    let kind = observation_badge(display.tone);
    let mut body = column![
        row![
            text(lang.tr("dns_self_heal_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                })
                .width(Length::Fill),
            badge(overall, kind),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_XS);

    if display.rows.is_empty() {
        body = body.push(text(display.empty).size(12).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        }));
    }
    for check in display.rows {
        let label = check.state;
        let kind = observation_badge(check.tone);
        let mut row_content = row![
            text(check.kind).size(12).width(Length::Shrink),
            Space::new().width(theme::SP_SM),
            badge(label, kind),
            Space::new().width(theme::SP_SM),
            text(check.detail.clone())
                .size(11)
                .width(Length::Fill)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .align_y(Alignment::Center);
        if let Some(fix) = check.fix {
            row_content = row_content
                .push(Space::new().width(Length::Fill))
                .push(text(fix).size(11));
        }
        body = body.push(
            container(row_content)
                .padding([5, 8])
                .style(row_card_surface),
        );
    }

    card(
        Some(lang.tr("dns_self_heal_title").to_string()),
        body.spacing(theme::SP_SM),
    )
}

/// DUAL-14-06: the observed Fake-IP binding table with a local search box.
pub(crate) fn fake_ip_pool_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let display = project_mappings(
        &state.editor.dns_fake_ip_pool,
        &state.editor.dns_fake_ip_query,
        lang.0,
    );
    let source_line: Element<'_, Message> = text(display.source.clone()).size(11).into();

    let mut body = column![
        row![
            text(lang.tr("dns_fakeip_pool_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                })
                .width(Length::Fill),
            source_line,
        ]
        .align_y(Alignment::Center),
        text_input(
            lang.tr("dns_fakeip_pool_search").as_ref(),
            &state.editor.dns_fake_ip_query
        )
        .on_input(Message::UpdateDnsFakeIpQuery)
        .padding([8, 12])
        .size(12)
        .font(MONO)
        .style(form_input_style),
    ]
    .spacing(theme::SP_XS);

    body = body.push(text(display.count).size(11).style(|t: &Theme| text::Style {
        color: Some(tokens(t).text_tertiary),
    }));

    if display.rows.is_empty() {
        body = body.push(text(display.empty).size(12).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        }));
    } else {
        let mut rows = column![].spacing(4);
        for entry in display.rows {
            rows = rows.push(
                container(
                    row![
                        text(entry.address.clone())
                            .size(12)
                            .font(MONO)
                            .width(Length::FillPortion(2)),
                        text("↔").size(12),
                        Space::new().width(theme::SP_SM),
                        text(entry.domain.clone())
                            .size(12)
                            .width(Length::FillPortion(3)),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([5, 8])
                .style(row_card_surface),
            );
        }
        body = body.push(rows);
    }

    card(
        Some(lang.tr("dns_fakeip_pool_title").to_string()),
        body.spacing(theme::SP_SM),
    )
}

/// Applied facts and the native editor launcher remain visible on the DNS page.
pub(crate) fn hosts_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let editor = &state.editor.dns_hosts_editor;
    card(
        Some(lang.tr("dns_hosts_title").into_owned()),
        column![
            text(summary(editor, lang.0)).size(12),
            text(feedback(editor, lang.0)).size(11),
            text_btn(
                lang.tr("dns_hosts_open").into_owned(),
                style_accent,
                Some(Message::OpenDnsHostsEditor)
            ),
        ]
        .spacing(theme::SP_SM),
    )
}

#[cfg(test)]
#[path = "../../tests/gui/view_dns_hosts_tests.rs"]
mod tests;
