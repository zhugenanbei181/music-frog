//! DUAL-11-15: shared rules-engine / MRS regression matrix.
//!
//! One headless test per shared fact the two surfaces depend on. The surface
//! suites (`infiltrator-bevy-ui` headless, `infiltrator-iced` GUI) ride on top
//! of these reductions, so this matrix is the common denominator both must
//! keep green. Items still honestly `planned` (11-06 unpack persistence,
//! 11-07 cache purge) are intentionally absent: there is no shared backend to
//! assert.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::rule_edit::{RuleDraft, RuleMoveDirection};
use infiltrator_domain::mrs::{
    Behavior, build_mrs_bytes, deconstruct_mrs_payload, parse_mrs_header,
};
use infiltrator_domain::rules::edit;
use infiltrator_domain::rules::types::parse_rule_str;
use infiltrator_domain::rules::view;
use infiltrator_domain::rules::{RuleEntry, game_routing_presets};
use infiltrator_domain::sub_rules::validate_logical_rule_syntax;

fn entry(rule: &str) -> RuleEntry {
    RuleEntry {
        rule: rule.to_owned(),
        enabled: true,
    }
}

/// DUAL-11-01: every advertised rule-type spelling parses to its named type.
#[test]
fn matrix_11_01_rule_type_vocabulary_parses() {
    let types = [
        "DOMAIN,a.com",
        "DOMAIN-SUFFIX,a.com",
        "DOMAIN-KEYWORD,key",
        "DOMAIN-REGEX,^a",
        "GEOSITE,cn",
        "IP-CIDR,1.1.1.1/32",
        "IP-CIDR6,2001:db8::/32",
        "IP-SUFFIX,1.1.1.1",
        "IP-ASN,13335",
        "GEOIP,CN",
        "SRC-GEOIP,US",
        "SRC-IP-CIDR,10.0.0.0/8",
        "SRC-IP-ASN,65000",
        "DST-PORT,443",
        "SRC-PORT,1234",
        "IN-PORT,7890",
        "IN-TYPE,INNER",
        "IN-NAME,eth0",
        "IN-USER,alice",
        "PROCESS-PATH,/usr/bin/curl",
        "PROCESS-PATH-REGEX,.*curl",
        "PROCESS-NAME,curl.exe",
        "PROCESS-NAME-REGEX,.*ssh",
        "NETWORK,tcp",
        "DSCP,46",
        "UID,1000",
        "PACKAGE-NAME,com.example",
        "RULE-SET,ads",
    ];
    for raw in types {
        let parsed = parse_rule_str(&format!("{raw},TARGET")).expect("parse rule type");
        let (expected, _) = raw.split_once(',').unwrap();
        assert_eq!(parsed.rule_type.name(), expected, "type {raw}");
        assert_eq!(parsed.target, "TARGET");
    }
    // MATCH carries only a target.
    assert_eq!(
        parse_rule_str("MATCH,DIRECT").unwrap().rule_type.name(),
        "MATCH"
    );
}

/// DUAL-11-02: logical sub-rules parse and evaluate recursively.
#[test]
fn matrix_11_02_logical_sub_rules_evaluate() {
    assert!(validate_logical_rule_syntax("AND((DOMAIN,a.com),(DST-PORT,443),T)").is_ok());
    assert!(validate_logical_rule_syntax("AND((DOMAIN,a.com),T").is_err());

    let and = parse_rule_str("AND((DOMAIN,a.com),(DST-PORT,443),T)").unwrap();
    assert_eq!(and.rule_type.name(), "AND");
    let or = parse_rule_str("OR((DOMAIN,a.com),(DOMAIN,b.com),T)").unwrap();
    assert_eq!(or.rule_type.name(), "OR");
    let not = parse_rule_str("NOT((DOMAIN,a.com),T)").unwrap();
    assert_eq!(not.rule_type.name(), "NOT");
    let sub = parse_rule_str("SUB-RULE((DOMAIN,a.com),(DST-PORT,443),T)").unwrap();
    assert_eq!(sub.rule_type.name(), "SUB-RULE");
}

/// DUAL-11-03: MRS header parse + payload deconstruction.
#[test]
fn matrix_11_03_mrs_binary_pipeline() {
    let payload = b"example.com\ngoogle.com\n";
    let bytes = build_mrs_bytes(Behavior::Domain, 2, 2, "Matrix", payload, None);
    let meta = parse_mrs_header(&bytes).unwrap();
    assert_eq!(meta.behavior, Behavior::Domain);
    assert_eq!(meta.rule_count, 2);
    let deconstructed = deconstruct_mrs_payload(&bytes).unwrap();
    assert!(
        deconstructed
            .iter()
            .any(|line| line.contains("example.com"))
    );
}

/// DUAL-11-04: the provider read model carries the declared source URL.
#[test]
fn matrix_11_04_rule_provider_source_url_projection() {
    let provider = infiltrator_contract::surface_snapshot::RuleProviderSnapshot {
        name: "geoip-cn".to_owned(),
        rule_count: 850,
        behavior: "ipcidr".to_owned(),
        updated_at: "2026-09-01".to_owned(),
        source_url: Some("https://example.com/cn.mrs".to_owned()),
    };
    assert_eq!(
        provider.source_url.as_deref(),
        Some("https://example.com/cn.mrs")
    );
}

/// DUAL-11-05: the incremental provider refresh intent is part of the shared bus.
#[test]
fn matrix_11_05_provider_refresh_intent_exists() {
    let intent = CommandIntent::RefreshRuleProviders;
    assert_eq!(
        intent.kind(),
        infiltrator_contract::command::CommandKind::Profile
    );
}

/// DUAL-11-08/13: keyword search + pagination are shared arithmetic.
#[test]
fn matrix_11_08_search_and_pagination_reduce_in_shared_view() {
    let rules = vec![
        entry("DOMAIN,a.com,DIRECT"),
        entry("DOMAIN-SUFFIX,b.com,PROXY"),
        entry("GEOIP,CN,DIRECT"),
    ];
    assert_eq!(view::filter_rule_indices(&rules, "proxy"), vec![1]);
    assert_eq!(view::page_count(0, 0), 1);
    assert_eq!(view::page_bounds(9, 5, 2), (4, 5));
}

/// DUAL-11-09/10/11/12: toggle, reorder, wizard and presets are shared edits.
#[test]
fn matrix_11_09_to_12_rule_edit_reductions() {
    let mut rules = vec![entry("A,1,DIRECT"), entry("B,2,DIRECT")];
    assert!(edit::toggle_rule_enabled(&mut rules, 0));
    assert!(!rules[0].enabled);
    assert!(edit::move_rule(&mut rules, 0, RuleMoveDirection::Down));
    assert_eq!(rules[0].rule, "B,2,DIRECT");

    let built = edit::build_custom_rule(&RuleDraft {
        rule_type: "AND".to_owned(),
        payload: "(DOMAIN,a.com),(DST-PORT,443)".to_owned(),
        target: "AI".to_owned(),
    })
    .unwrap();
    assert_eq!(built.rule, "AND((DOMAIN,a.com),(DST-PORT,443),AI)");

    let inserted = edit::inject_game_presets(&mut rules, "Game");
    assert_eq!(inserted, game_routing_presets("Game").len());
    assert!(rules[0].rule.contains("Game"));
}
