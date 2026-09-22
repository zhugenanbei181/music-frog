//! Iced DNS workbench extras (DUAL-14-06 / 14-10 / 14-11).
//!
//! The Fake-IP mapping pool, the honest latency-probe policy and the
//! `dns.hosts` editor. Every value rendered here comes from the shared
//! `DnsPageSnapshot`; the hosts draft is the shared `Vec<DnsHostEntry>` and
//! the submitted patch is the shared `DnsSettingsPatch`.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, form_field_label, form_input_style, icon_button, row_card_surface,
    style_accent, style_ghost, text_btn,
};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::dns::{DnsHostsIssue, FakeIpMappingSource};
use infiltrator_contract::dns_latency::{DnsLatencyReport, DnsLatencySummary, DnsProbeOutcome};
use infiltrator_contract::dns_self_heal::{DnsSelfHealKind, DnsSelfHealSnapshot, DnsSelfHealState};
use infiltrator_shared::locales::{Lang, Localizer};

/// DUAL-14-10: the localized per-server result line of the last real probe.
pub(crate) fn latency_result_lines<'a>(
    report: &DnsLatencyReport,
    lang: &Lang<'_>,
) -> Vec<Element<'a, Message>> {
    report
        .results
        .iter()
        .map(|result| {
            let tier = if result.is_fallback {
                lang.tr("dns_latency_fallback")
            } else {
                lang.tr("dns_latency_primary")
            };
            let outcome = match &result.outcome {
                DnsProbeOutcome::Measured { rtt_ms } => lang
                    .tr("dns_latency_measured")
                    .replace("{ms}", &rtt_ms.to_string()),
                DnsProbeOutcome::TimedOut => lang.tr("dns_latency_timeout").to_string(),
                DnsProbeOutcome::InvalidResponse { reason } => lang
                    .tr("dns_latency_invalid_response")
                    .replace("{reason}", reason),
                DnsProbeOutcome::Failed { message } => {
                    lang.tr("dns_latency_failed").replace("{reason}", message)
                }
                DnsProbeOutcome::NotProbed { reason } => lang
                    .tr("dns_latency_not_probed")
                    .replace("{reason}", reason),
            };
            let color = match &result.outcome {
                DnsProbeOutcome::Measured { rtt_ms } if *rtt_ms <= 100 => COLOR_FAST,
                DnsProbeOutcome::Measured { .. } => COLOR_SLOW,
                _ => COLOR_UNREACHABLE,
            };
            row![
                text(result.address.clone())
                    .size(12)
                    .font(MONO)
                    .width(Length::Fill),
                text(format!("{tier} · ")).size(11),
                text(outcome.to_string())
                    .size(12)
                    .style(move |_: &Theme| text::Style { color: Some(color) }),
            ]
            .align_y(Alignment::Center)
            .into()
        })
        .collect()
}

/// Latency tier inks (the same thresholds the Bevy server row uses).
const COLOR_FAST: iced::Color = iced::Color::from_rgb(0.20, 0.70, 0.42);
const COLOR_SLOW: iced::Color = iced::Color::from_rgb(0.86, 0.65, 0.20);
const COLOR_UNREACHABLE: iced::Color = iced::Color::from_rgb(0.85, 0.33, 0.33);

/// DUAL-14-10: the honest per-nameserver latency policy line and results.
pub(crate) fn latency_policy_line<'a>(
    report: &DnsLatencyReport,
    on_probe: Option<Message>,
    probing: bool,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let (label, kind) = match report.summary() {
        DnsLatencySummary::AllMeasured { .. } => (
            lang.tr("dns_latency_all_measured").to_string(),
            BadgeKind::Success,
        ),
        DnsLatencySummary::Partial { measured, total } => (
            lang.tr("dns_latency_partial")
                .replace("{measured}", &measured.to_string())
                .replace("{total}", &total.to_string()),
            BadgeKind::Warning,
        ),
        DnsLatencySummary::NoneReachable { total } => (
            lang.tr("dns_latency_none_reachable")
                .replace("{total}", &total.to_string()),
            BadgeKind::Danger,
        ),
        DnsLatencySummary::NotProbed => (
            lang.tr("dns_latency_not_probed_yet").to_string(),
            BadgeKind::Neutral,
        ),
        DnsLatencySummary::Unsupported { reason } => (
            format!("{} ({reason})", lang.tr("dns_latency_unsupported")),
            BadgeKind::Neutral,
        ),
    };
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
    for line in latency_result_lines(report, lang) {
        body = body.push(line);
    }
    body.into()
}

fn self_heal_state_label(state: DnsSelfHealState, lang: &Lang<'_>) -> (String, BadgeKind) {
    match state {
        DnsSelfHealState::Healthy => (
            lang.tr("dns_self_heal_healthy").to_string(),
            BadgeKind::Success,
        ),
        DnsSelfHealState::Warning => (
            lang.tr("dns_self_heal_warning").to_string(),
            BadgeKind::Warning,
        ),
        DnsSelfHealState::Critical => (
            lang.tr("dns_self_heal_critical").to_string(),
            BadgeKind::Danger,
        ),
        DnsSelfHealState::Unknown => (
            lang.tr("dns_self_heal_unknown").to_string(),
            BadgeKind::Neutral,
        ),
    }
}

fn self_heal_kind_label(kind: DnsSelfHealKind, lang: &Lang<'_>) -> String {
    match kind {
        DnsSelfHealKind::ListenPort => lang.tr("dns_self_heal_listen_port").to_string(),
        DnsSelfHealKind::UpstreamResolution => lang.tr("dns_self_heal_upstream").to_string(),
        DnsSelfHealKind::Topology => lang.tr("dns_self_heal_topology").to_string(),
    }
}

/// DUAL-14-13: the shared DNS self-heal observation, one row per check.
pub(crate) fn self_heal_panel<'a>(
    snapshot: &DnsSelfHealSnapshot,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let (overall, kind) = self_heal_state_label(snapshot.overall_state(), lang);
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

    if snapshot.checks.is_empty() {
        body = body.push(
            text(lang.tr("dns_self_heal_empty").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }
    for check in &snapshot.checks {
        let (label, kind) = self_heal_state_label(check.state, lang);
        let mut row_content = row![
            text(self_heal_kind_label(check.kind, lang))
                .size(12)
                .width(Length::Fixed(150.0)),
            badge(label, kind),
            Space::new().width(theme::SP_SM),
            text(check.detail.clone())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
        ]
        .align_y(Alignment::Center);
        if let Some(fix) = check.fix {
            row_content = row_content
                .push(Space::new().width(Length::Fill))
                .push(text(lang.tr("dns_self_heal_fix").replace("{fix}", fix.key())).size(11));
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
    let pool = &state.editor.dns_fake_ip_pool;
    let matches = pool.filter(&state.editor.dns_fake_ip_query);

    let source_line: Element<'_, Message> = match &pool.source {
        FakeIpMappingSource::LiveConnections => badge(
            lang.tr("dns_fakeip_pool_observed").to_string(),
            BadgeKind::Accent,
        ),
        FakeIpMappingSource::Unsupported { reason }
        | FakeIpMappingSource::Unavailable { reason } => text(format!(
            "{} ({reason})",
            lang.tr("dns_fakeip_pool_unsupported")
        ))
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        })
        .into(),
    };

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

    if pool.is_observed_subset() {
        body = body.push(
            text(
                lang.tr("dns_fakeip_pool_total")
                    .replace("{shown}", &matches.len().to_string())
                    .replace("{total}", &pool.total.to_string()),
            )
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        );
    }

    if matches.is_empty() {
        body = body.push(
            text(lang.tr("dns_fakeip_pool_empty").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    } else {
        let mut rows = column![].spacing(4);
        for entry in matches {
            rows = rows.push(
                container(
                    row![
                        text(entry.address.clone())
                            .size(12)
                            .font(MONO)
                            .width(Length::Fixed(150.0)),
                        text("↔").size(12),
                        Space::new().width(theme::SP_SM),
                        text(entry.domain.clone()).size(12),
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

fn hosts_issue_line(issue: &DnsHostsIssue, lang: &Lang<'_>) -> Element<'static, Message> {
    let (key, token) = match issue {
        DnsHostsIssue::InvalidAddress { address } => ("dns_hosts_issue_address", address),
        DnsHostsIssue::InvalidDomain { domain } => ("dns_hosts_issue_domain", domain),
    };
    text(lang.tr(key).replace("{value}", token))
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).danger),
        })
        .into()
}

/// DUAL-14-11: the graphical `dns.hosts` mapping editor.
pub(crate) fn hosts_panel<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let mut body = column![
        text(lang.tr("dns_hosts_desc").to_string())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        form_field_label(lang.tr("dns_hosts_address").to_string()),
        text_input("192.168.1.1", &state.editor.dns_hosts_address)
            .on_input(Message::UpdateDnsHostsAddress)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(form_input_style),
        form_field_label(lang.tr("dns_hosts_domain").to_string()),
        text_input("router.lan", &state.editor.dns_hosts_domain)
            .on_input(Message::UpdateDnsHostsDomain)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(form_input_style),
    ]
    .spacing(theme::SP_XS);

    body = body.push(
        row![
            text_btn(
                lang.tr("dns_hosts_add").to_string(),
                style_ghost,
                Some(Message::AddDnsHostRow)
            ),
            Space::new().width(Length::Fill),
            text_btn(
                lang.tr("dns_hosts_apply").to_string(),
                style_accent,
                Some(Message::SaveDnsHosts)
            ),
        ]
        .align_y(Alignment::Center),
    );

    if state.editor.dns_hosts.is_empty() {
        body = body.push(text(lang.tr("dns_hosts_empty").to_string()).size(12).style(
            |t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            },
        ));
    } else {
        let mut rows = column![].spacing(4);
        for (index, entry) in state.editor.dns_hosts.iter().enumerate() {
            rows = rows.push(
                container(
                    row![
                        text(entry.address.clone())
                            .size(12)
                            .font(MONO)
                            .width(Length::Fixed(150.0)),
                        text(entry.domain.clone()).size(12),
                        Space::new().width(Length::Fill),
                        icon_button(Icon::Trash2, 12.0, Message::RemoveDnsHostRow(index),),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding([5, 8])
                .style(row_card_surface),
            );
        }
        body = body.push(rows);
    }

    for issue in infiltrator_contract::dns::validate_hosts(&state.editor.dns_hosts) {
        body = body.push(hosts_issue_line(&issue, lang));
    }

    let status_key = if state.editor.is_saving_dns_hosts {
        "dns_hosts_saving"
    } else if state.editor.dns_hosts_dirty {
        "dns_hosts_pending"
    } else {
        "dns_hosts_saved"
    };
    body = body.push(
        text(lang.tr(status_key).to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
    );

    if let Some(error) = &state.editor.advanced_validation.dns_hosts {
        body = body.push(text(error.clone()).size(11).style(|t: &Theme| text::Style {
            color: Some(tokens(t).danger),
        }));
    }

    card(
        Some(lang.tr("dns_hosts_title").to_string()),
        body.spacing(theme::SP_SM),
    )
}

#[cfg(test)]
#[path = "../../tests/gui/view_dns_hosts_tests.rs"]
mod tests;
