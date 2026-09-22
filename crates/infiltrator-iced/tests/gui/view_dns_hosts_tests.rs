use super::*;

// ===========================================================================
// DUAL-14-06 / 14-10 / 14-11: Fake-IP pool, latency policy & hosts editor
// ===========================================================================

fn fake_ip_pool() -> infiltrator_contract::dns::FakeIpMappingPool {
    use infiltrator_contract::dns::{FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource};
    FakeIpMappingPool {
        source: FakeIpMappingSource::LiveConnections,
        range: "198.18.0.1/16".to_owned(),
        total: 2,
        entries: vec![
            FakeIpMappingEntry {
                domain: "music.example.org".to_owned(),
                address: "198.18.0.5".to_owned(),
            },
            FakeIpMappingEntry {
                domain: "cdn.example.net".to_owned(),
                address: "198.18.0.7".to_owned(),
            },
        ],
    }
}

fn host_row(domain: &str, address: &str) -> infiltrator_contract::dns::DnsHostEntry {
    infiltrator_contract::dns::DnsHostEntry {
        domain: domain.to_owned(),
        address: address.to_owned(),
    }
}

#[test]
fn test_dns_fake_ip_pool_panel_filters_the_observed_subset() {
    let (mut state, _) = AppState::new();
    state.editor.dns_fake_ip_pool = fake_ip_pool();
    state.editor.dns_fake_ip_query = "cdn".to_owned();
    let matches = state
        .editor
        .dns_fake_ip_pool
        .filter(&state.editor.dns_fake_ip_query);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].address, "198.18.0.7");
    // Both locales render the panel (the copy is localized, not hardcoded).
    let _ = fake_ip_pool_panel(&state, &Lang("zh-CN"));
    let _ = fake_ip_pool_panel(&state, &Lang("en-US"));

    // An unavailable host never renders a fabricated binding.
    state.editor.dns_fake_ip_pool = Default::default();
    let _ = fake_ip_pool_panel(&state, &Lang("zh-CN"));
}

#[test]
fn test_dns_latency_policy_line_is_localized_and_honest() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    let unsupported_zh = zh.tr("dns_latency_unsupported").to_string();
    let unsupported_en = en.tr("dns_latency_unsupported").to_string();
    assert!(unsupported_zh.contains("不填充假延迟"), "{unsupported_zh}");
    assert!(
        unsupported_en.contains("none is invented"),
        "{unsupported_en}"
    );
    let _ = latency_policy_line(DnsLatencyStatus::Unsupported, &zh);
    let _ = latency_policy_line(DnsLatencyStatus::Unsupported, &en);
    let _ = latency_policy_line(DnsLatencyStatus::Ready, &zh);
}

#[test]
fn test_dns_hosts_panel_add_remove_rows_uses_the_shared_draft() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateDnsHostsAddress("192.168.1.1".to_owned()));
    let _ = state.update(Message::UpdateDnsHostsDomain("router.lan".to_owned()));
    let _ = state.update(Message::AddDnsHostRow);
    assert_eq!(
        state.editor.dns_hosts,
        vec![host_row("router.lan", "192.168.1.1")]
    );
    assert!(state.editor.dns_hosts_dirty);

    // An exact duplicate row is not added twice.
    let _ = state.update(Message::UpdateDnsHostsAddress("192.168.1.1".to_owned()));
    let _ = state.update(Message::UpdateDnsHostsDomain("router.lan".to_owned()));
    let _ = state.update(Message::AddDnsHostRow);
    assert_eq!(state.editor.dns_hosts.len(), 1);

    // The add form is cleared after a successful insert.
    assert!(state.editor.dns_hosts_address.is_empty());
    assert!(state.editor.dns_hosts_domain.is_empty());

    let _ = state.update(Message::RemoveDnsHostRow(0));
    assert!(state.editor.dns_hosts.is_empty());

    // Both locales render the editor and the shared validation copy.
    state.editor.dns_hosts = vec![host_row("bad domain", "nope")];
    let issues = infiltrator_contract::dns::validate_hosts(&state.editor.dns_hosts);
    assert_eq!(issues.len(), 2);
    let _ = hosts_panel(&state, &Lang("zh-CN"));
    let _ = hosts_panel(&state, &Lang("en-US"));
    for issue in &issues {
        let zh = Lang("zh-CN");
        let en = Lang("en-US");
        let _ = hosts_issue_line(issue, &zh);
        let _ = hosts_issue_line(issue, &en);
    }
}

#[test]
fn test_dns_hosts_issue_copy_is_localized() {
    use infiltrator_contract::dns::DnsHostsIssue;
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    let address_zh = zh.tr("dns_hosts_issue_address").replace("{value}", "nope");
    let address_en = en.tr("dns_hosts_issue_address").replace("{value}", "nope");
    assert!(address_zh.contains("别名域名"), "{address_zh}");
    assert!(address_en.contains("alias domain"), "{address_en}");
    assert!(
        !address_en
            .chars()
            .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
    );
    let _ = hosts_issue_line(
        &DnsHostsIssue::InvalidDomain {
            domain: "bad domain".to_owned(),
        },
        &zh,
    );
}
