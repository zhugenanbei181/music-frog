use super::*;
use crate::view::dns_form_panel::{
    dns_form_field_widget, domain_mapping_mode_control, filter_mode_control, flush_outcome_label,
    form_issue_banner, localized_form_issue, token_row,
};
use infiltrator_contract::dns::{DnsEnhancedMode, DnsFakeIpFilterMode};
use infiltrator_contract::dns_form::DnsWorkbenchForm;

#[test]
fn test_rebuild_status_badge_kinds() {
    let lang_zh = Lang("zh-CN");
    let lang_en = Lang("en-US");
    let _ = rebuild_status_badge(&lang_zh, &RebuildFlowState::Idle, "DNS", false, true);
    let _ = rebuild_status_badge(&lang_en, &RebuildFlowState::Idle, "DNS", false, true);
    let _ = rebuild_status_badge(&lang_zh, &RebuildFlowState::Idle, "DNS", true, false);
    let saving = RebuildFlowState::Saving {
        label: "DNS".into(),
    };
    let _ = rebuild_status_badge(&lang_zh, &saving, "DNS", false, false);
    let rebuilding = RebuildFlowState::Rebuilding {
        label: "DNS".into(),
    };
    let _ = rebuild_status_badge(&lang_zh, &rebuilding, "DNS", false, false);
    let done = RebuildFlowState::Done {
        label: "DNS".into(),
    };
    let _ = rebuild_status_badge(&lang_zh, &done, "DNS", false, false);
    let failed = RebuildFlowState::Failed {
        label: "DNS".into(),
        error: "error".into(),
    };
    let _ = rebuild_status_badge(&lang_zh, &failed, "DNS", false, false);
    let _ = rebuild_status_badge(&lang_zh, &RebuildFlowState::Idle, "DNS", false, false);
}

#[test]
fn test_validation_error_banner() {
    let lang_zh = Lang("zh-CN");
    let lang_en = Lang("en-US");
    let _ = validation_error_banner("Invalid DNS IP", &lang_zh);
    let _ = validation_error_banner("Invalid DNS IP", &lang_en);
}

#[test]
fn test_save_button_states() {
    let _ = save_button(false, true, Message::SaveDns, "Save");
    let _ = save_button(true, false, Message::SaveDns, "Save");
    let _ = save_button(false, false, Message::SaveDns, "Save");
}

#[test]
fn test_dns_protocol_chip_mappings() {
    assert_eq!(dns_protocol_chip("https://dns.google/dns-query"), "DoH");
    assert_eq!(dns_protocol_chip("http://127.0.0.1/dns-query"), "DoH");
    assert_eq!(dns_protocol_chip("h3://dns.google/dns-query"), "DoH3");
    assert_eq!(dns_protocol_chip("tls://223.5.5.5:853"), "DoT");
    assert_eq!(dns_protocol_chip("quic://dns.adguard.com"), "DoQ");
    assert_eq!(dns_protocol_chip("doq://dns.adguard.com"), "DoQ");
    assert_eq!(dns_protocol_chip("dhcp://en0"), "DHCP");
    assert_eq!(dns_protocol_chip("tcp://1.1.1.1"), "TCP");
    assert_eq!(dns_protocol_chip("system"), "System");
    assert_eq!(dns_protocol_chip("223.5.5.5"), "UDP");
    assert_eq!(dns_protocol_chip(""), "DNS");
}

#[test]
fn test_item_list_mutation() {
    let raw = "223.5.5.5, 119.29.29.29\nhttps://doh.pub/dns-query";
    let parsed = parse_item_list(raw);
    assert_eq!(parsed.len(), 3);
    assert_eq!(parsed[0], "223.5.5.5");

    let removed = remove_item_from_list(raw, 1);
    assert_eq!(removed, "223.5.5.5, https://doh.pub/dns-query");

    let appended = append_item_to_list(&removed, "tls://223.5.5.5:853");
    assert_eq!(
        appended,
        "223.5.5.5, https://doh.pub/dns-query, tls://223.5.5.5:853"
    );

    let no_dup = append_item_to_list(&appended, "223.5.5.5");
    assert_eq!(no_dup, appended);
}

#[test]
fn test_domain_mapping_and_filter_mode_controls() {
    let _ = domain_mapping_mode_control(DnsEnhancedMode::FakeIp, &Lang("zh-CN"));
    let _ = domain_mapping_mode_control(DnsEnhancedMode::RedirHost, &Lang("en-US"));
    let _ = domain_mapping_mode_control(DnsEnhancedMode::Unmapped, &Lang("zh-CN"));
    let _ = filter_mode_control(DnsFakeIpFilterMode::Blacklist, &Lang("zh-CN"));
    let _ = filter_mode_control(DnsFakeIpFilterMode::Whitelist, &Lang("zh-CN"));
    let _ = filter_mode_control(DnsFakeIpFilterMode::Rules, &Lang("en-US"));
}

#[test]
fn test_server_tag_labels_are_localized() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    assert_eq!(server_tag_label(DnsServerTag::Domestic, &zh), "国内");
    assert_eq!(server_tag_label(DnsServerTag::Encrypted, &en), "Encrypted");
    assert_eq!(server_tag_label(DnsServerTag::Fallback, &en), "Fallback");
    assert_eq!(server_tag_label(DnsServerTag::Plain, &zh), "明文");
    let _ = token_row(
        "https://doh.pub/dns-query",
        0,
        "https://doh.pub/dns-query",
        false,
        &zh,
        Message::UpdateDnsFormNameserver,
    );
}

// ===========================================================================
// DUAL-14-04 / 14-05 / 14-07 / 14-14: shared workbench form parity
// ===========================================================================

#[test]
fn test_dns_form_field_widgets_cover_every_shared_field() {
    let form = DnsWorkbenchForm::default();
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    for field in DnsFormField::ALL {
        let _ = dns_form_field_widget(field, &form, &zh);
        let _ = dns_form_field_widget(field, &form, &en);
    }
    assert_eq!(DnsFormField::ALL.len(), 18);
}

#[test]
fn test_dns_form_panel_renders_the_shared_draft() {
    let (mut state, _) = AppState::new();
    state.editor.dns_form = DnsWorkbenchForm {
        nameserver: "https://doh.pub/dns-query".to_owned(),
        fallback_policy: infiltrator_contract::dns_form::DnsFallbackPolicyDraft {
            geoip: true,
            geoip_code: "CN".to_owned(),
            trigger_ipcidr: "240.0.0.0/4".to_owned(),
        },
        ..DnsWorkbenchForm::default()
    };
    let _ = dns_form_panel(&state, &Lang("zh-CN"));
    let _ = dns_form_panel(&state, &Lang("en-US"));
}

#[test]
fn test_dns_form_validation_issues_localize_in_both_locales() {
    let mut form = DnsWorkbenchForm {
        nameserver: "ftp://dns.example".to_owned(),
        bootstrap_nameserver: "doh.pub".to_owned(),
        ..DnsWorkbenchForm::default()
    };
    form.fallback_policy.trigger_ipcidr = "192.168.0.0/33".to_owned();
    form.fallback_policy.geoip_code = "CHN".to_owned();
    let issues = form.validate();
    assert!(!issues.is_empty());
    let zh = localized_form_issue(&issues[0], &Lang("zh-CN"));
    let en = localized_form_issue(&issues[0], &Lang("en-US"));
    assert!(zh.contains("字段"), "{zh}");
    assert!(en.contains("nameserver"), "{en}");
    assert!(!en.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)));
    for issue in &issues {
        let _ = localized_form_issue(issue, &Lang("zh-CN"));
        let _ = localized_form_issue(issue, &Lang("en-US"));
    }
    let _ = form_issue_banner(&issues, &Lang("zh-CN"));
    let _ = form_issue_banner(&issues, &Lang("en-US"));
}

#[test]
fn test_dns_cache_flush_outcome_labels_are_localized() {
    use infiltrator_contract::dns::DnsFlushOutcome;
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    assert_eq!(
        flush_outcome_label(&DnsFlushOutcome::NotRequested, &zh),
        "尚未执行"
    );
    assert_eq!(
        flush_outcome_label(&DnsFlushOutcome::Flushed, &en),
        "flushed"
    );
    let unsupported = flush_outcome_label(
        &DnsFlushOutcome::Unsupported {
            reason: "no adapter".to_owned(),
        },
        &zh,
    );
    assert!(unsupported.contains("宿主不支持"), "{unsupported}");
    let failed = flush_outcome_label(
        &DnsFlushOutcome::Failed {
            message: "exit 1".to_owned(),
        },
        &en,
    );
    assert!(failed.contains("flush failed"), "{failed}");
}

#[test]
fn test_dns_form_patch_uses_the_shared_workbench_mapping() {
    let (mut state, _) = AppState::new();
    state.editor.dns_form = DnsWorkbenchForm {
        nameserver: "https://dns.google/dns-query, quic://dns.adguard.com".to_owned(),
        fallback: "8.8.8.8".to_owned(),
        fallback_policy: infiltrator_contract::dns_form::DnsFallbackPolicyDraft {
            geoip: true,
            geoip_code: "CN".to_owned(),
            trigger_ipcidr: "240.0.0.0/4".to_owned(),
        },
        fake_ip_range: String::new(),
        ..DnsWorkbenchForm::default()
    };
    let domain_patch = state
        .dns_patch_from_form()
        .expect("shared workbench patch mapping");
    assert_eq!(
        domain_patch.nameserver.as_deref(),
        Some(
            &[
                "https://dns.google/dns-query".to_owned(),
                "quic://dns.adguard.com".to_owned()
            ][..]
        )
    );
    assert_eq!(
        domain_patch.fallback.as_deref(),
        Some(&["8.8.8.8".to_owned()][..])
    );
    let filter = domain_patch
        .fallback_filter_partial
        .expect("fallback filter partial merge");
    assert_eq!(filter.geoip, Some(true));
    assert_eq!(filter.geoip_code.as_deref(), Some("CN"));
    assert!(domain_patch.clear_fake_ip_range);
    assert!(state.dns_form_issue().is_none());
}
