//! Shared labels for DNS settings and observed cache results, with single-pass interpolation.
use crate::text::localize;
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode};
use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue};
use lurker_ui_presentation::copy::TextCatalog;
pub const fn mode_key(mode: DnsEnhancedMode) -> &'static str {
    match mode {
        DnsEnhancedMode::FakeIp => "dns_mode_fakeip",
        DnsEnhancedMode::RedirHost => "dns_mode_redirhost",
        DnsEnhancedMode::Unmapped => "dns_mode_none",
    }
}
pub const fn filter_key(mode: DnsFakeIpFilterMode) -> &'static str {
    match mode {
        DnsFakeIpFilterMode::Blacklist => "dns_filter_blacklist",
        DnsFakeIpFilterMode::Whitelist => "dns_filter_whitelist",
        DnsFakeIpFilterMode::Rules => "dns_filter_rules",
    }
}
pub fn heading(mode: DnsEnhancedMode, count: usize, catalog: &dyn TextCatalog) -> String {
    localize(
        catalog,
        "dns_overview_summary",
        &[
            ("mode", catalog.text(mode_key(mode)).into_owned()),
            ("count", count.to_string()),
        ],
    )
}

/// Stable shared copy identity for every editable field; native localization replays it.
pub const fn field_label_key(field: DnsFormField) -> &'static str {
    match field {
        DnsFormField::Enable => "dns_field_enable",
        DnsFormField::Ipv6 => "dns_field_ipv6",
        DnsFormField::Cache => "dns_field_cache",
        DnsFormField::UseHosts => "dns_field_use_hosts",
        DnsFormField::UseSystemHosts => "dns_field_use_system_hosts",
        DnsFormField::RespectRules => "dns_field_respect_rules",
        DnsFormField::EnhancedMode => "dns_field_enhanced_mode",
        DnsFormField::FilterMode => "dns_field_filter_mode",
        DnsFormField::BootstrapNameserver => "dns_field_bootstrap",
        DnsFormField::Nameserver => "dns_field_nameserver",
        DnsFormField::Fallback => "dns_field_fallback",
        DnsFormField::FallbackGeoip => "dns_field_geoip",
        DnsFormField::FallbackGeoipCode => "dns_field_geoip_code",
        DnsFormField::FallbackTriggerIp => "dns_field_trigger",
        DnsFormField::FakeIpRange => "dns_field_fake_range",
        DnsFormField::FakeIpFilter => "dns_field_fake_filter",
        DnsFormField::ProxyServerNameserver => "dns_field_proxy_nameserver",
        DnsFormField::DirectNameserver => "dns_field_direct_nameserver",
    }
}
pub fn field_label(field: DnsFormField, catalog: &dyn TextCatalog) -> String {
    catalog.text(field_label_key(field)).into_owned()
}
pub fn form_issue(issue: &DnsFormIssue, catalog: &dyn TextCatalog) -> String {
    match issue {
        DnsFormIssue::UnsupportedScheme { field, entry } => localize(
            catalog,
            "dns_form_err_scheme",
            &[("field", field.key().into()), ("entry", entry.clone())],
        ),
        DnsFormIssue::BootstrapNotIp { entry } => localize(
            catalog,
            "dns_form_err_bootstrap",
            &[("entry", entry.clone())],
        ),
        DnsFormIssue::InvalidTriggerCidr { entry } => {
            localize(catalog, "dns_form_err_cidr", &[("entry", entry.clone())])
        }
        DnsFormIssue::InvalidGeoipCode { value } => localize(
            catalog,
            "dns_form_err_geoip_code",
            &[("value", value.clone())],
        ),
    }
}
pub fn form_status(
    issues: &[DnsFormIssue],
    dirty: bool,
    queued: bool,
    host_missing: bool,
    catalog: &dyn TextCatalog,
) -> String {
    if let Some(issue) = issues.first() {
        return localize(
            catalog,
            "dns_edit_invalid",
            &[("reason", form_issue(issue, catalog))],
        );
    }
    catalog
        .text(if host_missing {
            "dns_edit_host_unavailable"
        } else if dirty {
            "dns_edit_dirty"
        } else if queued {
            "dns_edit_queued"
        } else {
            "dns_edit_unchanged"
        })
        .into_owned()
}
