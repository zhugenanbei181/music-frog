//! test-intent: behavior
//! Rule rows preserve native syntax while sharing copy for observed hit counts.

use super::*;
use infiltrator_shared::locales::Lang;

#[test]
fn unknown_zero_disabled_and_shadowed_local_trace_counts_have_shared_copy() {
    assert_eq!(
        row_hits_copy(None, true, false, &Lang("en-US")),
        "Not observed"
    );
    assert_eq!(
        row_hits_copy(None, false, false, &Lang("en-US")),
        "Not observed · Disabled"
    );
    assert_eq!(
        row_hits_copy(None, true, true, &Lang("zh-CN")),
        "未观测 · 被遮蔽"
    );

    assert_eq!(
        row_hits_copy(Some(0), true, false, &Lang("en-US")),
        "0 local trace hits"
    );
    assert_eq!(
        row_hits_copy(Some(5), false, false, &Lang("en-US")),
        "5 local trace hits · Disabled"
    );
    assert_eq!(
        row_hits_copy(Some(12), true, true, &Lang("zh-CN")),
        "12 次本地追踪命中 · 被遮蔽"
    );
}

#[test]
fn regex_commas_logical_payloads_and_native_parameters_keep_exact_semantics() {
    for (raw, payload, target, source, resolve) in [
        (
            "DOMAIN-REGEX,^a{1,2}\\.example$,PROXY",
            "^a{1,2}\\.example$",
            "PROXY",
            false,
            false,
        ),
        (
            "AND,((DOMAIN,example.com),(DST-PORT,443)),DIRECT",
            "AND,((DOMAIN,example.com),(DST-PORT,443))",
            "DIRECT",
            false,
            false,
        ),
        (
            "IP-CIDR,192.0.2.0/24,REJECT,src,no-resolve",
            "192.0.2.0/24",
            "REJECT",
            true,
            true,
        ),
    ] {
        let row = rule_row(raw);
        assert_eq!(row.payload, payload);
        assert_eq!(row.target, target);
        assert_eq!(row.source_ip, source);
        assert_eq!(row.no_resolve, resolve);
        assert!(row.failure.is_none());
    }
    let invalid = rule_row("AND((DOMAIN,example.com),DIRECT)");
    assert!(invalid.rule_type.starts_with("AND("));
    let invalid = rule_row("AND,((DOMAIN,example.com)),DIRECT,no-resolve");
    assert!(invalid.failure.is_some());
    assert!(invalid.target.is_empty());
    assert_eq!(
        invalid.payload,
        "AND,((DOMAIN,example.com)),DIRECT,no-resolve"
    );
}
