//! Shared labels for DNS settings and observed cache results, with single-pass interpolation.
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode};
use infiltrator_contract::dns_cache::{DnsCacheFlushReport, DnsFlushOutcome};
use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue};
use infiltrator_shared::i18n_interpolator::localize;
use infiltrator_shared::locales::{Lang, Localizer};
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
pub fn flush_outcome(outcome: &DnsFlushOutcome, code: &str) -> String {
    match outcome {
        DnsFlushOutcome::NotRequested => Lang(code).tr("dns_flush_not_requested").into_owned(),
        DnsFlushOutcome::Flushed => Lang(code).tr("dns_flush_flushed").into_owned(),
        DnsFlushOutcome::Unsupported { reason } => localize(
            code,
            "dns_flush_unsupported_detail",
            &[("reason", reason.clone())],
        ),
        DnsFlushOutcome::Failed { failure } => localize(
            code,
            "dns_flush_failed_detail",
            &[("reason", failure.message.clone())],
        ),
    }
}
pub fn flush_summary(report: &DnsCacheFlushReport, code: &str) -> String {
    localize(
        code,
        "dns_flush_summary",
        &[
            ("fakeip", flush_outcome(&report.fake_ip, code)),
            ("os", flush_outcome(&report.os_cache, code)),
        ],
    )
}
pub fn heading(mode: DnsEnhancedMode, count: usize, code: &str) -> String {
    localize(
        code,
        "dns_overview_summary",
        &[
            ("mode", Lang(code).tr(mode_key(mode)).into_owned()),
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
pub fn field_label(field: DnsFormField, code: &str) -> String {
    Lang(code).tr(field_label_key(field)).into_owned()
}
pub fn form_issue(issue: &DnsFormIssue, code: &str) -> String {
    match issue {
        DnsFormIssue::UnsupportedScheme { field, entry } => localize(
            code,
            "dns_form_err_scheme",
            &[("field", field.key().into()), ("entry", entry.clone())],
        ),
        DnsFormIssue::BootstrapNotIp { entry } => {
            localize(code, "dns_form_err_bootstrap", &[("entry", entry.clone())])
        }
        DnsFormIssue::InvalidTriggerCidr { entry } => {
            localize(code, "dns_form_err_cidr", &[("entry", entry.clone())])
        }
        DnsFormIssue::InvalidGeoipCode { value } => {
            localize(code, "dns_form_err_geoip_code", &[("value", value.clone())])
        }
    }
}
pub fn form_status(
    issues: &[DnsFormIssue],
    dirty: bool,
    queued: bool,
    host_missing: bool,
    code: &str,
) -> String {
    if let Some(issue) = issues.first() {
        return localize(
            code,
            "dns_edit_invalid",
            &[("reason", form_issue(issue, code))],
        );
    }
    Lang(code)
        .tr(if host_missing {
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

#[cfg(test)]
mod tests {
    use super::{field_label, form_issue, form_status};
    use infiltrator_contract::dns_form::{DnsFormField, DnsFormIssue};

    #[test]
    fn dns_issue_interpolation_keeps_user_placeholders_and_unicode_opaque_in_both_languages() {
        let entry = "ftp://{field}/{reason}/中文🙂";
        let issue = DnsFormIssue::UnsupportedScheme {
            field: DnsFormField::Nameserver,
            entry: entry.into(),
        };
        assert_eq!(
            form_issue(&issue, "en-US"),
            format!("Unsupported upstream scheme in nameserver: {entry}")
        );
        assert_eq!(
            form_issue(&issue, "zh-CN"),
            format!("字段 nameserver 的上游协议不受支持: {entry}")
        );
        assert_eq!(
            form_status(&[issue], false, true, false, "en-US"),
            format!("Local validation failed: Unsupported upstream scheme in nameserver: {entry}")
        );
        assert_eq!(
            field_label(DnsFormField::BootstrapNameserver, "en-US"),
            "default_nameserver (bootstrap, pure IP)"
        );
        assert_eq!(
            form_status(&[], false, true, true, "en-US"),
            "Command service is not composed; DNS patch was not submitted"
        );
        assert_eq!(
            form_status(&[], false, true, false, "en-US"),
            "DNS workbench patch queued; awaiting actual result"
        );
    }
}
