//! Locked-kernel target positions, leaf parameters and bitwise address suffixes.
use crate::rules::rewrite_rule_target;
use crate::rules::tracer::TrafficContext;
use crate::rules::tracer::evaluation::evaluate_rule;
use crate::rules::types::{RuleType, parse_rule_condition, parse_rule_str};
use infiltrator_contract::rule_condition::{ConditionIssue, ConditionOutcome, TrafficField};
#[test]
fn native_parameters_do_not_replace_outbound_or_become_part_of_the_payload() {
    let rule = parse_rule_str("IP-CIDR,192.0.2.0/24,Proxy,src,no-resolve").unwrap();
    assert_eq!(rule.target, "Proxy");
    assert_eq!(rule.rule_type, RuleType::IpCidr("192.0.2.0/24".into()));
    assert!(rule.source_ip);
    assert!(rule.no_resolve);
    let context = TrafficContext::from_ip("198.51.100.2".parse().unwrap())
        .with_src_ip("192.0.2.7".parse().unwrap());
    assert_eq!(evaluate_rule(&rule, &context), ConditionOutcome::Matched);
    let leaf = parse_rule_condition("IP-CIDR,192.0.2.0/24,no-resolve,src").unwrap();
    assert_eq!(leaf.target, "");
    assert!(leaf.source_ip && leaf.no_resolve);
    assert_eq!(evaluate_rule(&leaf, &context), ConditionOutcome::Matched);
    let regex = parse_rule_str("DOMAIN-REGEX,^a{1,2}\\.example$,RegexProxy").unwrap();
    assert_eq!(regex.target, "RegexProxy");
    assert_eq!(
        regex.rule_type,
        RuleType::DomainRegex("^a{1,2}\\.example$".into())
    );
    assert_eq!(
        evaluate_rule(&regex, &TrafficContext::from_domain("aa.example")),
        ConditionOutcome::Matched
    );
    let ignored = parse_rule_str("DOMAIN,example.com,Proxy,no-resolve").unwrap();
    assert_eq!(ignored.target, "Proxy");
    assert!(!ignored.no_resolve);
}
#[test]
fn leaf_no_resolve_works_inside_logic_and_root_parameters_are_rejected() {
    let rule = parse_rule_str("NOT,((IP-CIDR,192.0.2.0/24,no-resolve)),DIRECT").unwrap();
    assert_eq!(
        evaluate_rule(&rule, &TrafficContext::from_domain("example.com")),
        ConditionOutcome::Matched
    );
    let resolving = parse_rule_str("NOT,((IP-CIDR,192.0.2.0/24)),DIRECT").unwrap();
    assert_eq!(
        evaluate_rule(&resolving, &TrafficContext::from_domain("example.com")),
        ConditionOutcome::Unresolved(ConditionIssue::MissingInput(TrafficField::DestinationIp))
    );
    assert!(parse_rule_str("AND,((DOMAIN,example.com)),DIRECT,no-resolve").is_err());
    assert!(parse_rule_str("MATCH").is_err());
    assert!(parse_rule_condition("MATCH").is_err());
}
#[test]
fn ip_suffix_uses_low_address_bits_not_text_suffixes_and_respects_family_source_and_unknown() {
    let context = TrafficContext::from_ip("192.0.2.129".parse().unwrap())
        .with_src_ip("203.0.113.128".parse().unwrap());
    for (raw, expected) in [
        ("IP-SUFFIX,0.0.0.1/1,DIRECT", ConditionOutcome::Matched),
        ("IP-SUFFIX,0.0.0.2/2,DIRECT", ConditionOutcome::NotMatched),
        ("SRC-IP-SUFFIX,0.0.0.0/1,DIRECT", ConditionOutcome::Matched),
        ("IP-SUFFIX,0.0.0.0/1,DIRECT,src", ConditionOutcome::Matched),
        ("IP-SUFFIX,::1/1,DIRECT", ConditionOutcome::NotMatched),
        ("IP-SUFFIX,0.0.0.129/8,DIRECT", ConditionOutcome::Matched),
    ] {
        assert_eq!(
            evaluate_rule(&parse_rule_str(raw).unwrap(), &context),
            expected,
            "{raw}"
        );
    }
    let v6 = TrafficContext::from_ip("2001:db8::1234:81".parse().unwrap());
    assert_eq!(
        evaluate_rule(&parse_rule_str("IP-SUFFIX,::1/1,DIRECT").unwrap(), &v6),
        ConditionOutcome::Matched
    );
    let unknown = TrafficContext::from_domain("example.com");
    assert_eq!(
        evaluate_rule(
            &parse_rule_str("IP-SUFFIX,0.0.0.1/1,DIRECT").unwrap(),
            &unknown
        ),
        ConditionOutcome::Unresolved(ConditionIssue::MissingInput(TrafficField::DestinationIp))
    );
    assert_eq!(
        evaluate_rule(
            &parse_rule_str("IP-SUFFIX,0.0.0.1/1,DIRECT,no-resolve").unwrap(),
            &unknown
        ),
        ConditionOutcome::NotMatched
    );
    assert!(matches!(
        evaluate_rule(&parse_rule_str("IP-SUFFIX,.129,DIRECT").unwrap(), &context),
        ConditionOutcome::Unresolved(ConditionIssue::InvalidRule { .. })
    ));
    assert!(matches!(
        evaluate_rule(
            &parse_rule_str("IP-SUFFIX,0.0.0.1/33,DIRECT").unwrap(),
            &context
        ),
        ConditionOutcome::Unresolved(ConditionIssue::InvalidRule { .. })
    ));
}

#[test]
fn reverse_apply_changes_only_native_target_and_preserves_all_parameters_and_regex_commas() {
    for (original, expected) in [
        (
            "IP-CIDR,192.0.2.0/24, Proxy ,src,no-resolve",
            "IP-CIDR,192.0.2.0/24, REJECT ,src,no-resolve",
        ),
        (
            "DOMAIN-REGEX,^a{1,2}\\.example$,Proxy",
            "DOMAIN-REGEX,^a{1,2}\\.example$,REJECT",
        ),
        (
            "AND,((IP-CIDR,192.0.2.0/24,no-resolve),(DOMAIN,example.com)),Proxy",
            "AND,((IP-CIDR,192.0.2.0/24,no-resolve),(DOMAIN,example.com)),REJECT",
        ),
        ("MATCH,Proxy", "MATCH,REJECT"),
    ] {
        assert_eq!(
            rewrite_rule_target(original, "REJECT").as_deref(),
            Some(expected)
        );
        let rewritten = parse_rule_str(expected).unwrap();
        assert_eq!(rewritten.target, "REJECT");
        let previous = parse_rule_str(original).unwrap();
        match (&rewritten.rule_type, &previous.rule_type) {
            (RuleType::Logical(new), RuleType::Logical(old)) => {
                assert_eq!(new.payload, old.payload)
            }
            (new, old) => assert_eq!(new, old),
        }
        assert_eq!(rewritten.source_ip, previous.source_ip);
        assert_eq!(rewritten.no_resolve, previous.no_resolve);
    }
    assert!(rewrite_rule_target("AND,((DOMAIN,example.com)),Proxy,no-resolve", "REJECT").is_none());
}
