use super::*;

#[test]
fn test_rebuild_status_badge_kinds() {
    let lang_zh = Lang("zh-CN");
    let lang_en = Lang("en-US");
    let _ = rebuild_status_badge(&lang_zh, &RebuildFlowState::Idle, "DNS", false, true);
    let _ = rebuild_status_badge(&lang_en, &RebuildFlowState::Idle, "DNS", false, true);
    let _ = rebuild_status_badge(&lang_zh, &RebuildFlowState::Idle, "DNS", true, false);
    let saving = RebuildFlowState::Saving { label: "DNS".into() };
    let _ = rebuild_status_badge(&lang_zh, &saving, "DNS", false, false);
    let rebuilding = RebuildFlowState::Rebuilding { label: "DNS".into() };
    let _ = rebuild_status_badge(&lang_zh, &rebuilding, "DNS", false, false);
    let done = RebuildFlowState::Done { label: "DNS".into() };
    let _ = rebuild_status_badge(&lang_zh, &done, "DNS", false, false);
    let failed = RebuildFlowState::Failed { label: "DNS".into(), error: "error".into() };
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
    assert_eq!(appended, "223.5.5.5, https://doh.pub/dns-query, tls://223.5.5.5:853");

    let no_dup = append_item_to_list(&appended, "223.5.5.5");
    assert_eq!(no_dup, appended);
}

#[test]
fn test_domain_mapping_and_filter_mode_controls() {
    let _ = domain_mapping_mode_control("fake-ip", &Lang("zh-CN"));
    let _ = domain_mapping_mode_control("redir-host", &Lang("en-US"));
    let _ = domain_mapping_mode_control("none", &Lang("zh-CN"));
    let _ = filter_mode_control(&Lang("zh-CN"));
    let _ = filter_mode_control(&Lang("en-US"));
}
