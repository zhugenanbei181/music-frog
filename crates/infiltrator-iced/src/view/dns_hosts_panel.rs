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
use infiltrator_contract::dns::{DnsHostsIssue, DnsLatencyStatus, FakeIpMappingSource};
use infiltrator_shared::locales::{Lang, Localizer};

/// DUAL-14-10: the honest per-nameserver latency policy line.
pub(crate) fn latency_policy_line<'a>(
    status: DnsLatencyStatus,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let (label, kind) = match status {
        DnsLatencyStatus::Ready => (lang.tr("dns_latency_ready"), BadgeKind::Success),
        DnsLatencyStatus::Unsupported => (lang.tr("dns_latency_unsupported"), BadgeKind::Neutral),
    };
    row![
        text(lang.tr("dns_latency_title").to_string())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_SM),
        badge(label.to_string(), kind),
    ]
    .align_y(Alignment::Center)
    .into()
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
