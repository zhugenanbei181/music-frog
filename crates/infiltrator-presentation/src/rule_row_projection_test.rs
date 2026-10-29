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
fn the_count_and_the_status_are_two_chips_of_their_own() {
    // The count chip carries the count and nothing else.
    let observed = RuleRowChips::of(Some(12), true, false);
    assert_eq!(observed.hits, RuleRowHits::Observed { count: 12 });
    assert_eq!(observed.status, RuleRowStatus::Enabled);
    assert_eq!(
        row_hits_chip(observed.hits, &Lang("en-US")),
        "12 local trace hits"
    );
    assert_eq!(
        row_hits_chip(observed.hits, &Lang("zh-CN")),
        "12 次本地追踪命中"
    );

    // An unobserved row says so, and says nothing else.
    let unobserved = RuleRowChips::of(None, true, false);
    assert_eq!(unobserved.hits, RuleRowHits::Unobserved);
    assert_eq!(
        row_hits_chip(unobserved.hits, &Lang("en-US")),
        "Not observed"
    );

    // The status chip is its own fact: a plain row has none, a disabled or
    // shadowed row names itself.
    assert_eq!(
        row_status_chip(RuleRowStatus::Enabled, &Lang("en-US")),
        None
    );
    assert_eq!(
        row_status_chip(RuleRowStatus::Disabled, &Lang("en-US")),
        Some("Disabled".to_string())
    );
    assert_eq!(
        row_status_chip(RuleRowStatus::Shadowed, &Lang("zh-CN")),
        Some("被遮蔽".to_string())
    );

    // A disabled row is disabled first; a shadowed row is only shadowed.
    let disabled = RuleRowChips::of(Some(3), false, true);
    assert_eq!(disabled.status, RuleRowStatus::Disabled);
    let shadowed = RuleRowChips::of(Some(3), true, true);
    assert_eq!(shadowed.status, RuleRowStatus::Shadowed);

    // The keys are the shared fact: a surface that renders a chip live (the
    // Bevy row restamps its text) reads the same key the shared copy uses,
    // so no surface names a key of its own.
    assert_eq!(observed.hits.key(), "rule_hits_count");
    assert_eq!(observed.hits.count(), Some(12));
    assert_eq!(unobserved.hits.key(), "rule_hits_unobserved");
    assert_eq!(unobserved.hits.count(), None);
    assert_eq!(RuleRowStatus::Enabled.key(), None);
    assert_eq!(RuleRowStatus::Disabled.key(), Some("rule_draft_disabled"));
    assert_eq!(RuleRowStatus::Shadowed.key(), Some("rule_status_shadowed"));
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

#[test]
fn rule_row_chips_volume_formatting() {
    let chips = RuleRowChips::of_with_volume(Some(42), Some(1024 * 1024 * 5), true, false);
    assert_eq!(chips.hits.count(), Some(42));
    assert_eq!(chips.volume, Some(5 * 1024 * 1024));
    let vol_label = chips.volume_label().unwrap();
    assert!(vol_label.contains("MB") || vol_label.contains("M"));

    let empty = RuleRowChips::of(Some(10), true, false);
    assert!(empty.volume_label().is_none());
}
