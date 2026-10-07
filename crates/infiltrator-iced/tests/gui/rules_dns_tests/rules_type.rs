//! Behavior cases for rules type.
//! test-intent: behavior

use super::*;
use infiltrator_application::rule_list_fixtures::list_document;
use infiltrator_domain::rules::edit::{CUSTOM_RULE_TYPE_CHOICES, DEFAULT_RULE_TARGET};
use infiltrator_domain::rules::logical::{
    LOGICAL_OPERATOR_CHOICES, build_logical_rule, default_logical_draft, draft_expression,
};
use infiltrator_domain::rules::matrix::{RULE_TYPE_MATRIX, RuleTypeFamily, matrix_family};
use infiltrator_domain::rules::view::DEFAULT_RULE_PAGE_SIZE;
use infiltrator_domain::sub_rules::validate_logical_rule_syntax;

#[test]
fn test_rules_type_matrix_and_logical_builder_delegate_to_shared() {
    let (mut state, _) = AppState::new();
    let types = [
        "DOMAIN",
        "DOMAIN-SUFFIX",
        "DOMAIN-KEYWORD",
        "DOMAIN-REGEX",
        "GEOSITE",
        "IP-CIDR",
        "IP-CIDR6",
        "IP-SUFFIX",
        "IP-ASN",
        "GEOIP",
        "SRC-GEOIP",
        "SRC-IP-CIDR",
        "SRC-IP-ASN",
        "DST-PORT",
        "SRC-PORT",
        "IN-PORT",
        "IN-TYPE",
        "IN-NAME",
        "IN-USER",
        "PROCESS-PATH",
        "PROCESS-PATH-REGEX",
        "PROCESS-NAME",
        "PROCESS-NAME-REGEX",
        "NETWORK",
        "DSCP",
        "UID",
        "PACKAGE-NAME",
        "RULE-SET",
    ];
    let rules: Vec<RuleEntry> = types
        .iter()
        .map(|rule_type| RuleEntry {
            rule: format!("{rule_type},payload,TARGET"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(rules))));
    let rendered: Vec<String> = state
        .editor
        .rules_render_cache
        .iter()
        .map(|item| item.rule_type.clone())
        .collect();
    assert_eq!(rendered.len(), types.len());
    for rule_type in types {
        assert!(rendered.iter().any(|item| item == rule_type), "{rule_type}");
    }

    // DUAL-11-01: every catalogue spelling renders the shared display label and
    // resolves to a known semantic family (no per-surface spelling list).
    for spec in RULE_TYPE_MATRIX.iter() {
        assert_eq!(display_rule_type(spec.name), spec.label, "{}", spec.name);
        assert_eq!(matrix_family(spec.name), spec.family);
        assert!(matrix_family(spec.name) != RuleTypeFamily::Unknown);
        assert_eq!(
            semantic_badge_kind(spec.name, RuleBadgeKind::Other),
            match spec.family {
                RuleTypeFamily::Host => BadgeKind::Accent,
                RuleTypeFamily::Address => BadgeKind::Warning,
                _ => BadgeKind::Neutral,
            },
            "{}",
            spec.name
        );
    }
    // Separator/case tolerant lookup survives the delegation.
    assert_eq!(display_rule_type("domainsuffix"), "DomainSuffix");
    assert_eq!(display_rule_type("CUSTOM"), "CUSTOM");

    // DUAL-11-02: the surface relies on the shared logical-rule syntax gate.
    assert!(validate_logical_rule_syntax("AND,((DOMAIN,a.com),(DST-PORT,443)),T").is_ok());
    assert!(validate_logical_rule_syntax("AND((DOMAIN,a.com),T").is_err());

    // DUAL-11-02: the builder draft is the shared `LogicalDraft` and the insert
    // builds through the shared reduction, so the persisted expression is the
    // canonical `OP((cond),(cond),TARGET)` form the parser accepts.
    let draft = default_logical_draft(DEFAULT_RULE_TARGET);
    let built = build_logical_rule(&draft).unwrap();
    assert_eq!(
        built.rule,
        "AND,((DOMAIN-SUFFIX,company.com),(NETWORK,TCP)),PROXY"
    );
    assert_eq!(draft_expression(&draft), built.rule);
    assert_eq!(state.editor.subrule_draft, draft);
    assert_eq!(LOGICAL_OPERATOR_CHOICES.len(), 4);

    // DUAL-11-15: the shared view/edit reductions are reachable from the
    // surface and use the same constants the Bevy page consumes.
    assert_eq!(DEFAULT_RULE_PAGE_SIZE, 200);
    assert_eq!(CUSTOM_RULE_TYPE_CHOICES.len(), 12);
}
