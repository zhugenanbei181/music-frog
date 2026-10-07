//! test-intent: behavior
use super::*;

#[test]
fn hosts_editor_round_trips_rows_without_ambiguity() {
    let raw = "127.0.0.1 localhost\n# comment\n192.168.1.1 router.lan *.router.lan";
    let entries = parse_hosts_editor(raw).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].address, "127.0.0.1");
    assert_eq!(entries[2].domain, "*.router.lan");
    let text = hosts_editor_text(&entries);
    assert_eq!(parse_hosts_editor(&text).unwrap(), entries);
    // A single-line surface separates rows with `;` and parses identically.
    assert_eq!(
        parse_hosts_editor("1.1.1.1 a.com; 8.8.8.8 b.com").unwrap(),
        vec![
            DnsHostEntry {
                domain: "a.com".to_owned(),
                address: "1.1.1.1".to_owned()
            },
            DnsHostEntry {
                domain: "b.com".to_owned(),
                address: "8.8.8.8".to_owned()
            },
        ]
    );
    let appended = append_host_row(&text, "10.0.0.1", "nas.lan").unwrap();
    assert_eq!(
        remove_host_at(&appended, 0).unwrap(),
        "192.168.1.1 router.lan\n192.168.1.1 *.router.lan\n10.0.0.1 nas.lan"
    );
    assert_eq!(
        append_host_row(&appended, "10.0.0.1", "nas.lan").unwrap(),
        appended
    );
    assert_eq!(append_host_row(text.as_str(), " ", "x").unwrap(), text);
}

#[test]
fn hosts_validation_accepts_host_grammar_only() {
    assert!(is_valid_hosts_address("1.2.3.4"));
    assert!(is_valid_hosts_address("2001:db8::1"));
    assert!(is_valid_hosts_address("lan"));
    assert!(is_valid_hosts_address("target.example.com"));
    assert!(!is_valid_hosts_address("notadomain"));
    assert!(!is_valid_hosts_address("has space"));
    assert!(!is_valid_hosts_address(""));
    assert!(is_valid_hosts_domain("*.example.com"));
    assert!(!is_valid_hosts_domain("bad domain"));
    assert!(!is_valid_hosts_domain("host:53"));

    let issues = validate_hosts(&[
        DnsHostEntry {
            domain: "ok.example.com".to_owned(),
            address: "1.2.3.4".to_owned(),
        },
        DnsHostEntry {
            domain: "bad domain".to_owned(),
            address: "nope".to_owned(),
        },
    ]);
    assert_eq!(issues.len(), 2);
    assert!(issues.contains(&DnsHostsIssue::InvalidDomain {
        domain: "bad domain".to_owned()
    }));
    assert!(issues.contains(&DnsHostsIssue::InvalidAddress {
        address: "nope".to_owned()
    }));
    assert_eq!(issues[0].token(), "bad domain");
}
#[test]
fn incomplete_and_invalid_text_never_turn_into_an_empty_clear_patch() {
    for raw in [
        "1.1.1.1",
        "1.1.1.1 # missing domain",
        "1.1.1.1 valid.test; 8.8.8.8",
        "nope valid.test",
    ] {
        assert!(!parse_hosts_editor(raw).unwrap_err().is_empty(), "{raw}");
        assert!(
            remove_host_at(raw, 0).is_err(),
            "invalid input must survive for correction"
        );
        assert!(append_host_row(raw, "1.2.3.4", "valid.test").is_err());
    }
    assert_eq!(
        parse_hosts_editor(" ; # comments only\n ").unwrap(),
        Vec::<DnsHostEntry>::new()
    );
    assert_eq!(
        parse_hosts_editor("1.1.1.1").unwrap_err(),
        vec![DnsHostsIssue::MissingDomain {
            address: "1.1.1.1".into()
        }]
    );
}

#[test]
fn locked_kernel_multi_value_and_alias_cycle_rules_are_not_silently_written_as_invalid_maps() {
    assert!(parse_hosts_editor("target.test source.test; 1.1.1.1 source.test").unwrap_err().iter().any(|issue| matches!(issue, DnsHostsIssue::MixedAddressList { domain } if domain == "source.test")));
    assert!(
        parse_hosts_editor("b.test a.test; a.test b.test")
            .unwrap_err()
            .iter()
            .any(|issue| matches!(issue, DnsHostsIssue::AliasCycle { .. }))
    );
    assert!(parse_hosts_editor("1.1.1.1 a.test; 8.8.8.8 a.test").is_ok());
    assert!(parse_hosts_editor("1.1.1.1 end.test; end.test alias.test").is_ok());
    for raw in [
        "LAN host.test",
        "1.1.1.1 bad..test",
        "1.1.1.1 trailing.test.",
    ] {
        assert!(parse_hosts_editor(raw).is_err(), "{raw}");
    }
    for domain in ["*.test", "+.test", ".test", "one.*.test"] {
        assert!(is_valid_hosts_domain(domain), "{domain}");
    }
}

#[test]
fn wildcard_alias_cycles_are_rejected_but_exact_terminal_overrides_and_zoned_ipv6_are_valid() {
    let row = |domain: &str, address: &str| DnsHostEntry {
        domain: domain.into(),
        address: address.into(),
    };
    for key in [
        "*.example.test",
        ".example.test",
        "+.example.test",
        "sub.*.test",
    ] {
        let alias = if key == "sub.*.test" {
            "sub.example.test"
        } else {
            "node.example.test"
        };
        assert!(
            validate_hosts(&[row(key, alias)])
                .iter()
                .any(|issue| matches!(issue, DnsHostsIssue::AliasCycle { .. })),
            "{key}"
        );
    }
    assert!(
        validate_hosts(&[
            row("*.example.test", "node.example.test"),
            row("node.example.test", "1.1.1.1")
        ])
        .is_empty()
    );
    assert!(validate_hosts(&[row("zoned.test", "fe80::1%eth0")]).is_empty());
    assert!(
        validate_hosts(&[
            row("zoned.test", "fe80::1%eth0"),
            row("zoned.test", "fe80::2%eth0")
        ])
        .is_empty()
    );
    assert!(!is_valid_hosts_address("fe80::1%"));
}

#[test]
fn expanded_patterns_cannot_depend_on_random_kernel_map_insertion_order() {
    let row = |domain: &str, address: &str| DnsHostEntry {
        domain: domain.into(),
        address: address.into(),
    };
    for key in ["example.test", ".example.test"] {
        assert!(
            validate_hosts(&[row("+.example.test", "1.1.1.1"), row(key, "2.2.2.2")])
                .iter()
                .any(|issue| matches!(issue, DnsHostsIssue::PatternConflict { .. }))
        );
        assert!(
            validate_hosts(&[row("+.example.test", "1.1.1.1"), row(key, "1.1.1.1")]).is_empty()
        );
    }
}
