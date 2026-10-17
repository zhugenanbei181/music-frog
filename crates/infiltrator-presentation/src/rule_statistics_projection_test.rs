//! test-intent: behavior
//! Absent observations stay distinct from measured zero and foreign source rows.

use super::*;
use crate::rule_fixtures::list_document;
use crate::rule_list_editor::RuleListReadback::Observed;
use infiltrator_contract::rule_edit::RuleMoveDirection;
use infiltrator_contract::rule_hit_audit::{RuleDeadEntry, RuleDeadReason};
use infiltrator_domain::rules::RuleEntry;

#[test]
fn absent_and_measured_zero_statistics_have_distinct_localized_copy() {
    let absent = project_statistics(None, "en-US");
    assert_eq!(absent.total_hits, "Not observed");
    assert_eq!(
        absent.summary("en-US"),
        "Local trace statistics · Not observed"
    );
    assert!(!absent.can_clear);
    let observed = project_statistics(Some(&RuleHitAuditSnapshot::default()), "en-US");
    assert_eq!(observed.total_hits, "0");
    assert_eq!(observed.latency, "Not observed");
    assert!(observed.summary("en-US").starts_with("Local trace hits 0"));
    assert!(observed.summary("zh-CN").starts_with("本地追踪命中 0"));
}

#[test]
fn qualified_audit_and_duplicate_requests_cannot_disable_a_different_identical_row() {
    let document = list_document(vec![
        RuleEntry {
            rule: "DOMAIN,example.com,DIRECT".into(),
            enabled: true,
        };
        2
    ]);
    let mut editor = RuleListEditor::default();
    editor.observe_readback(Observed(document.clone()));
    let first = editor.row_id(0).unwrap();
    let second = editor.row_id(1).unwrap();
    let mut audit = RuleHitAuditSnapshot {
        source: Some(document.source.clone()),
        dead_rules: vec![RuleDeadEntry {
            rule_index: Some(1),
            rule_raw: document.rules[1].rule.clone(),
            hit_count: 0,
            reason: RuleDeadReason::ZeroHits,
            shadowed_by: None,
            detail: None,
            last_hit_secs: None,
        }],
        ..RuleHitAuditSnapshot::default()
    };
    assert_eq!(zero_hit_rows(&editor, Some(&audit)), vec![second]);
    assert!(editor.move_rule(second, RuleMoveDirection::Up));
    assert_eq!(zero_hit_rows(&editor, Some(&audit)), vec![second]);
    assert_eq!(
        disable_zero_hit_rows(&mut editor, Some(&audit), &[first, second, second]),
        1
    );
    assert!(!editor.draft[0].enabled);
    assert!(editor.draft[1].enabled);
    assert!(editor.discard());
    assert_eq!(
        disable_zero_hit_rows(&mut editor, Some(&audit), &[second]),
        0
    );
    assert!(editor.draft.iter().all(|rule| rule.enabled));
    audit.source.as_mut().unwrap().profile = "another.yaml".into();
    assert!(zero_hit_rows(&editor, Some(&audit)).is_empty());
}
