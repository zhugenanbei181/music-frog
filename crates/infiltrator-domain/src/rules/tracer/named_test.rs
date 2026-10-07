//! Ordered named-table descent, fallthrough and uncertainty preserve qualified source identity.
use super::*;
fn entries(raw: &[&str]) -> Vec<RuleEntry> {
    raw.iter()
        .map(|rule| RuleEntry {
            rule: (*rule).into(),
            enabled: true,
        })
        .collect()
}
#[test]
fn named_descent_returns_the_real_leaf_and_qualified_path() {
    let rules = entries(&["SUB-RULE,(DOMAIN-SUFFIX,example.com),media", "MATCH,DIRECT"]);
    let tables = BTreeMap::from([
        (
            "media".into(),
            entries(&["SUB-RULE,(DST-PORT,443),secure", "MATCH,REJECT"]),
        ),
        (
            "secure".into(),
            entries(&["DOMAIN-SUFFIX,example.com,PROXY", "MATCH,REJECT"]),
        ),
    ]);
    let matched = trace_named_rules(
        &rules,
        &tables,
        &TrafficContext::from_domain("www.example.com").with_port(443),
    )
    .unwrap()
    .unwrap();
    assert_eq!(matched.target, "PROXY");
    assert_eq!(matched.raw, "DOMAIN-SUFFIX,example.com,PROXY");
    assert_eq!(matched.location, RuleLocation::named("secure".into(), 0));
    assert_eq!(
        matched.path,
        vec![
            RulePathEntry {
                location: RuleLocation::root(0),
                raw: rules[0].rule.clone()
            },
            RulePathEntry {
                location: RuleLocation::named("media".into(), 0),
                raw: tables["media"][0].rule.clone()
            },
            RulePathEntry {
                location: RuleLocation::named("secure".into(), 0),
                raw: tables["secure"][0].rule.clone()
            },
        ]
    );
}
#[test]
fn empty_and_nonmatching_tables_fall_through_but_unknown_and_cycles_do_not() {
    let rules = entries(&["SUB-RULE,(DOMAIN,example.com),list", "MATCH,DIRECT"]);
    let mut tables = BTreeMap::from([("list".into(), Vec::new())]);
    let context = TrafficContext::from_domain("example.com");
    assert_eq!(
        trace_named_rules(&rules, &tables, &context)
            .unwrap()
            .unwrap()
            .target,
        "DIRECT"
    );
    tables.insert("list".into(), entries(&["DOMAIN,other.com,REJECT"]));
    assert_eq!(
        trace_named_rules(&rules, &tables, &context)
            .unwrap()
            .unwrap()
            .location,
        RuleLocation::root(1)
    );
    tables.insert("list".into(), entries(&["NETWORK,udp,REJECT"]));
    assert!(matches!(
        trace_named_rules(&rules, &tables, &context)
            .unwrap_err()
            .issue,
        ConditionIssue::MissingInput(_)
    ));
    tables.insert(
        "list".into(),
        entries(&["SUB-RULE,(DOMAIN,example.com),list"]),
    );
    assert!(
        matches!(trace_named_rules(&rules, &tables, &context).unwrap_err().issue, ConditionIssue::InvalidRule { reason } if reason.contains("Cyclic"))
    );
    tables.clear();
    assert!(
        matches!(trace_named_rules(&rules, &tables, &context).unwrap_err().issue, ConditionIssue::InvalidRule { reason } if reason.contains("Missing sub-rule table"))
    );
    assert_eq!(
        trace_named_rules(&rules, &tables, &TrafficContext::from_domain("other.com"))
            .unwrap()
            .unwrap()
            .target,
        "DIRECT"
    );
}
