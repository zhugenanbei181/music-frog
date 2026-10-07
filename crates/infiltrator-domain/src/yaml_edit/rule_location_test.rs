//! Qualified scalar replacement preserves sibling rules, comments, quotes and physical bytes.
use super::*;
#[test]
fn only_the_confirmed_named_scalar_changes_in_crlf_and_quoted_tables() {
    let content = "\u{feff}mode: rule\r\n# keep\r\nrules:\r\n  - DOMAIN,example.com,DIRECT\r\nsub-rules:\r\n  'media: one': # table note\r\n    - 'DOMAIN,example.com,DIRECT'  # retain note\r\n    - MATCH,REJECT\r\n  sibling:\r\n    - DOMAIN,example.com,DIRECT\r\n# tail\r\nipv6: true\r\n";
    let location = RuleLocation::named("media: one".into(), 0);
    let updated = replace_rule_at_location(
        content,
        &location,
        "DOMAIN,example.com,DIRECT",
        "DOMAIN,example.com,REJECT",
    )
    .unwrap();
    assert_eq!(
        updated,
        content.replacen(
            "'DOMAIN,example.com,DIRECT'  # retain",
            "'DOMAIN,example.com,REJECT'  # retain",
            1
        )
    );
    assert!(
        replace_rule_at_location(
            content,
            &location,
            "DOMAIN,stale.com,DIRECT",
            "DOMAIN,example.com,REJECT"
        )
        .is_err()
    );
    assert!(
        replace_rule_at_location(
            content,
            &RuleLocation::named("missing".into(), 0),
            "DOMAIN,example.com,DIRECT",
            "DOMAIN,example.com,REJECT"
        )
        .is_err()
    );
}
#[test]
fn double_quotes_and_indentless_root_rules_are_preserved_and_alias_side_effects_are_rejected() {
    let content = "rules:\n- \"DOMAIN-REGEX,^a{1,2},DIRECT\" # keep\nsub-rules:\n  one: &rules\n    - DOMAIN,example.com,DIRECT\n  two: *rules\n";
    let updated = replace_rule_at_location(
        content,
        &RuleLocation::root(0),
        "DOMAIN-REGEX,^a{1,2},DIRECT",
        "DOMAIN-REGEX,^a{1,2},REJECT",
    )
    .unwrap();
    assert_eq!(
        updated,
        content.replacen("^a{1,2},DIRECT", "^a{1,2},REJECT", 1)
    );
    assert!(
        replace_rule_at_location(
            content,
            &RuleLocation::named("one".into(), 0),
            "DOMAIN,example.com,DIRECT",
            "DOMAIN,example.com,REJECT"
        )
        .is_err()
    );
}
