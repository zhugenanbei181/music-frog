//! Iced DNS workbench form panel (DUAL-14-04 / 14-05 / 14-14).
//!
//! The panel is assembled from the shared [`DnsFormField::ALL`] set, so a
//! shared field cannot render on Bevy and be missing here. Upstream lists,
//! the fallback Filter trigger fields and the local validation banner all use
//! the shared contract codec/validator.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{
    banner_alert, form_field_label, form_input_style, form_toggle_row, row_card_surface,
    style_ghost, text_btn,
};
use crate::view::components::{BadgeKind, chip, icon_button, segmented_control};
use crate::view::dns::{
    append_item_to_list, dns_protocol_chip, parse_item_list, remove_item_from_list,
    server_tag_label,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode, DnsServerTag};
use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue, DnsWorkbenchForm};
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) fn token_row<'a>(
    item: &str,
    idx: usize,
    raw_list: &'a str,
    is_domain: bool,
    lang: &Lang<'_>,
    on_update: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let tag: Element<'a, Message> = if is_domain {
        svg_icons::icon_themed(Icon::Globe, 12.0, |t: &Theme| tokens(t).text_tertiary)
    } else {
        chip(dns_protocol_chip(item))
    };
    let mut meta = row![tag].spacing(4).align_y(Alignment::Center);
    if !is_domain {
        for server_tag in DnsServerTag::classify(item, false) {
            meta = meta.push(chip(server_tag_label(server_tag, lang)));
        }
    }
    let address = text(item.to_string())
        .size(12)
        .font(MONO)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_primary),
        });
    let delete_btn = icon_button(
        Icon::Trash2,
        12.0,
        on_update(remove_item_from_list(raw_list, idx)),
    );

    container(
        row![
            meta,
            Space::new().width(theme::SP_SM),
            address,
            Space::new().width(Length::Fill),
            delete_btn
        ]
        .align_y(Alignment::Center),
    )
    .padding([5, 8])
    .style(row_card_surface)
    .into()
}

pub(crate) fn quick_template_chips<'a>(
    templates: &[&'static str],
    current_raw: &'a str,
    on_update: impl Fn(String) -> Message + 'a + Copy,
) -> Element<'a, Message> {
    let current_items = parse_item_list(current_raw);
    let chips: Vec<Element<'a, Message>> = templates
        .iter()
        .map(|&tpl| {
            let added = current_items.iter().any(|x| x.eq_ignore_ascii_case(tpl));
            let on_press = if added {
                None
            } else {
                Some(on_update(append_item_to_list(current_raw, tpl)))
            };
            text_btn(format!("+ {}", tpl), style_ghost, on_press)
        })
        .collect();
    row(chips).spacing(theme::SP_XS).wrap().into()
}

pub(crate) fn dynamic_token_section<'a>(
    label: &str,
    raw_list: &'a str,
    placeholder: &str,
    templates: &[&'static str],
    is_domain: bool,
    lang: &Lang<'_>,
    on_update: impl Fn(String) -> Message + 'a + Copy,
) -> Element<'a, Message> {
    let items = parse_item_list(raw_list);
    let mut col = column![form_field_label(label.to_string())].spacing(theme::SP_XS);
    if !templates.is_empty() {
        col = col.push(quick_template_chips(templates, raw_list, on_update));
    }
    if !items.is_empty() {
        let mut list_col = column![].spacing(4);
        for (idx, item) in items.iter().enumerate() {
            list_col = list_col.push(token_row(item, idx, raw_list, is_domain, lang, on_update));
        }
        col = col.push(list_col);
    }
    col.push(
        text_input(placeholder, raw_list)
            .on_input(on_update)
            .padding([8, 12])
            .size(12)
            .font(MONO)
            .style(form_input_style),
    )
    .into()
}

pub(crate) fn domain_mapping_mode_control(
    mode: DnsEnhancedMode,
    lang: &Lang<'_>,
) -> Element<'static, Message> {
    let mode_labels = vec![
        lang.tr("dns_mode_fakeip").to_string(),
        lang.tr("dns_mode_redirhost").to_string(),
        lang.tr("dns_mode_none").to_string(),
    ];
    let ctrl = segmented_control(&mode_labels, mode.to_index(), |idx| {
        Message::UpdateDnsFormEnhancedMode(DnsEnhancedMode::from_index(idx))
    });
    let lbl = lang.tr("dns_mode_label").to_string();
    column![form_field_label(lbl), ctrl]
        .spacing(theme::SP_XS)
        .into()
}

pub(crate) fn filter_mode_control(
    mode: DnsFakeIpFilterMode,
    lang: &Lang<'_>,
) -> Element<'static, Message> {
    let labels = vec![
        lang.tr("dns_filter_blacklist").to_string(),
        lang.tr("dns_filter_whitelist").to_string(),
        lang.tr("dns_filter_rules").to_string(),
    ];
    let ctrl = segmented_control(&labels, mode.to_index(), |idx| {
        Message::UpdateDnsFormFilterMode(DnsFakeIpFilterMode::from_index(idx))
    });
    let lbl = lang.tr("dns_filter_label").to_string();
    column![form_field_label(lbl), ctrl]
        .spacing(theme::SP_XS)
        .into()
}

/// One workbench field widget, selected by the shared field enum.
pub(crate) fn dns_form_field_widget<'a>(
    field: DnsFormField,
    form: &'a DnsWorkbenchForm,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    match field {
        DnsFormField::Enable => {
            form_toggle_row("enable", form.switches.enable, Message::UpdateDnsFormEnable)
        }
        DnsFormField::Ipv6 => {
            form_toggle_row("ipv6", form.switches.ipv6, Message::UpdateDnsFormIpv6)
        }
        DnsFormField::Cache => {
            form_toggle_row("cache", form.switches.cache, Message::UpdateDnsFormCache)
        }
        DnsFormField::UseHosts => form_toggle_row(
            "use_hosts",
            form.switches.use_hosts,
            Message::UpdateDnsFormUseHosts,
        ),
        DnsFormField::UseSystemHosts => form_toggle_row(
            "use_system_hosts",
            form.switches.use_system_hosts,
            Message::UpdateDnsFormUseSystemHosts,
        ),
        DnsFormField::RespectRules => form_toggle_row(
            "respect_rules",
            form.switches.respect_rules,
            Message::UpdateDnsFormRespectRules,
        ),
        DnsFormField::EnhancedMode => domain_mapping_mode_control(form.enhanced_mode, lang),
        DnsFormField::FilterMode => filter_mode_control(form.filter_mode, lang),
        DnsFormField::BootstrapNameserver => dynamic_token_section(
            "default_nameserver (bootstrap, pure IP)",
            &form.bootstrap_nameserver,
            "223.5.5.5, 119.29.29.29",
            &["223.5.5.5", "119.29.29.29"],
            false,
            lang,
            Message::UpdateDnsFormBootstrapNameserver,
        ),
        DnsFormField::Nameserver => dynamic_token_section(
            "nameserver (DoH/DoT/DoQ/UDP)",
            &form.nameserver,
            "https://dns.google/dns-query, 1.1.1.1",
            &[
                "tls://223.5.5.5:853",
                "https://doh.pub/dns-query",
                "223.5.5.5",
                "119.29.29.29",
            ],
            false,
            lang,
            Message::UpdateDnsFormNameserver,
        ),
        DnsFormField::Fallback => dynamic_token_section(
            "fallback",
            &form.fallback,
            "https://1.0.0.1/dns-query",
            &[
                "https://1.0.0.1/dns-query",
                "8.8.8.8",
                "1.1.1.1",
                "tls://1.0.0.1:853",
            ],
            false,
            lang,
            Message::UpdateDnsFormFallback,
        ),
        DnsFormField::FallbackGeoip => form_toggle_row(
            "fallback_filter.geoip",
            form.fallback_policy.geoip,
            Message::UpdateDnsFormFallbackGeoip,
        ),
        DnsFormField::FallbackGeoipCode => column![
            form_field_label("fallback_filter.geoip_code (ISO country)"),
            text_input("CN", &form.fallback_policy.geoip_code)
                .on_input(Message::UpdateDnsFormFallbackGeoipCode)
                .padding([8, 12])
                .size(12)
                .font(MONO)
                .style(form_input_style),
        ]
        .spacing(theme::SP_XS)
        .into(),
        DnsFormField::FallbackTriggerIp => dynamic_token_section(
            "fallback_filter.ipcidr (GEOIP trigger)",
            &form.fallback_policy.trigger_ipcidr,
            "240.0.0.0/4, 192.168.0.0/16",
            &["240.0.0.0/4", "192.168.0.0/16", "10.0.0.0/8"],
            true,
            lang,
            Message::UpdateDnsFormFallbackTrigger,
        ),
        DnsFormField::FakeIpRange => column![
            form_field_label("fake_ip_range".to_string()),
            text_input("198.18.0.1/16", &form.fake_ip_range)
                .on_input(Message::UpdateDnsFormFakeIpRange)
                .padding([8, 12])
                .size(12)
                .font(MONO)
                .style(form_input_style),
        ]
        .spacing(theme::SP_XS)
        .into(),
        DnsFormField::FakeIpFilter => dynamic_token_section(
            "fake_ip_filter",
            &form.fake_ip_filter,
            "*.lan, localhost.ptlogin2.qq.com",
            &["*.lan", "localhost.ptlogin2.qq.com", "*.local"],
            true,
            lang,
            Message::UpdateDnsFormFakeIpFilter,
        ),
        DnsFormField::ProxyServerNameserver => dynamic_token_section(
            "proxy_server_nameserver",
            &form.proxy_server_nameserver,
            "tls://223.5.5.5:853",
            &[
                "tls://223.5.5.5:853",
                "https://doh.pub/dns-query",
                "https://dns.alidns.com/dns-query",
            ],
            false,
            lang,
            Message::UpdateDnsFormProxyServerNameserver,
        ),
        DnsFormField::DirectNameserver => dynamic_token_section(
            "direct_nameserver",
            &form.direct_nameserver,
            "system",
            &["system", "223.5.5.5"],
            false,
            lang,
            Message::UpdateDnsFormDirectNameserver,
        ),
    }
}

/// Shared local validation issues rendered as a warning banner.
pub(crate) fn form_issue_banner(
    issues: &[DnsFormIssue],
    lang: &Lang<'_>,
) -> Element<'static, Message> {
    let detail = issues
        .iter()
        .map(|issue| localized_form_issue(issue, lang))
        .collect::<Vec<_>>()
        .join(" · ");
    banner_alert(BadgeKind::Warning, lang.tr("dns_form_issues"), detail, None)
}

pub(crate) fn localized_form_issue(issue: &DnsFormIssue, lang: &Lang<'_>) -> String {
    match issue {
        DnsFormIssue::UnsupportedScheme { field, entry } => lang
            .tr("dns_form_err_scheme")
            .replace("{field}", field.key())
            .replace("{entry}", entry),
        DnsFormIssue::BootstrapNotIp { entry } => {
            lang.tr("dns_form_err_bootstrap").replace("{entry}", entry)
        }
        DnsFormIssue::InvalidTriggerCidr { entry } => {
            lang.tr("dns_form_err_cidr").replace("{entry}", entry)
        }
        DnsFormIssue::InvalidGeoipCode { value } => {
            lang.tr("dns_form_err_geoip_code").replace("{value}", value)
        }
    }
}

/// Honest last DNS cache flush status for the Fake-IP panel.
pub(crate) fn dns_cache_flush_status<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let report = &state.diag.dns_cache_flush;
    let line = format!(
        "{}: {} · {}: {}",
        lang.tr("dns_flush_target_fakeip"),
        flush_outcome_label(&report.fake_ip, lang),
        lang.tr("dns_flush_target_os"),
        flush_outcome_label(&report.os_cache, lang),
    );
    text(line)
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
        .into()
}

pub(crate) fn flush_outcome_label(
    outcome: &infiltrator_contract::dns::DnsFlushOutcome,
    lang: &Lang<'_>,
) -> String {
    use infiltrator_contract::dns::DnsFlushOutcome;
    match outcome {
        DnsFlushOutcome::NotRequested => lang.tr("dns_flush_not_requested").to_string(),
        DnsFlushOutcome::Flushed => lang.tr("dns_flush_flushed").to_string(),
        DnsFlushOutcome::Unsupported { reason } => {
            format!("{} ({reason})", lang.tr("dns_flush_unsupported"))
        }
        DnsFlushOutcome::Failed { message } => {
            format!("{} ({message})", lang.tr("dns_flush_failed"))
        }
    }
}
