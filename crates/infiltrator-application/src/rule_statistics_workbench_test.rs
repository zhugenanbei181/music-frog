//! test-intent: behavior
//! Confirmations and reset receipts bind actual row and source identities.
use super::*;
use crate::rule_list_fixtures::list_document;
use crate::rule_statistics_inspector_projection::project_inspector;
use infiltrator_contract::rule_edit::RuleMoveDirection;
use infiltrator_contract::rule_hit_audit::{RuleDeadEntry, RuleDeadReason};
use infiltrator_domain::rules::RuleEntry;

fn page(revision: u64) -> PageData<RulesPageSnapshot> {
    let document = list_document(vec![
        RuleEntry {
            rule: "DOMAIN,example.com,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,REJECT".into(),
            enabled: true,
        },
    ]);
    PageData::ready(RulesPageSnapshot {
        document: Some(document.clone()),
        total_rules: 2,
        default_action: "REJECT".into(),
        providers: Vec::new(),
        rules: Vec::new(),
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        rule_publish_limit: 0,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
        hit_audit: Some(RuleHitAuditSnapshot {
            source: Some(document.source),
            revision,
            total_hits: 1,
            can_clear: true,
            dead_rules: vec![RuleDeadEntry {
                rule_index: Some(0),
                rule_raw: "DOMAIN,example.com,DIRECT".into(),
                hit_count: 0,
                reason: RuleDeadReason::ZeroHits,
                shadowed_by: None,
                detail: None,
                last_hit_secs: None,
            }],
            ..Default::default()
        }),
    })
}

#[test]
fn cleanup_cancel_changes_nothing_and_confirm_follows_stable_identity_after_reorder() {
    let page = page(2);
    let mut editor = RuleListEditor::default();
    editor.observe_page(&page);
    let target = editor.row_id(0).unwrap();
    let mut model = RuleStatisticsWorkbench::default();
    model.observe(&page);
    assert!(model.inspect(&editor));
    assert!(model.prepare_cleanup(&editor));
    let projection = project_inspector(&model, &editor, "en-US");
    assert_eq!(
        projection.confirmation_summary,
        format!(
            "Profile: {} · 1 rows",
            editor.base.as_ref().unwrap().source.profile
        )
    );
    let before = editor.draft.clone();
    assert!(model.cancel_cleanup());
    assert_eq!(editor.draft, before);
    assert!(!editor.dirty());
    assert!(model.prepare_cleanup(&editor));
    assert!(editor.move_rule(target, RuleMoveDirection::Down));
    assert!(model.can_confirm_cleanup(&editor));
    assert_eq!(model.confirm_cleanup(&mut editor), Some(1));
    assert!(editor.draft[0].enabled);
    assert!(!editor.draft[1].enabled);
    assert!(editor.dirty());
    assert_eq!(model.confirm_cleanup(&mut editor), None);
}

#[test]
fn failed_reads_retain_values_as_stale_and_new_sources_retire_them_without_cancelling_review() {
    let first = page(4);
    let mut editor = RuleListEditor::default();
    editor.observe_page(&first);
    let mut model = RuleStatisticsWorkbench::default();
    model.observe(&first);
    assert!(model.inspect(&editor));
    assert!(model.prepare_cleanup(&editor));
    let confirmation = model.confirmation.clone();
    let failure = Failure::new(ErrorCode::Permission, "cannot read source", false);
    model.observe(&PageData::failed(failure.clone()));
    assert_eq!(model.audit, first.data.as_ref().unwrap().hit_audit);
    assert_eq!(model.read_failure, Some(failure));
    assert!(!model.can_confirm_cleanup(&editor));
    assert!(!model.can_reset());
    let mut changed = first.clone();
    changed
        .data
        .as_mut()
        .unwrap()
        .document
        .as_mut()
        .unwrap()
        .source
        .profile = "other.yaml".into();
    changed.data.as_mut().unwrap().hit_audit = None;
    editor.observe_page(&changed);
    model.observe(&changed);
    assert_eq!(model.audit, None);
    assert_eq!(model.confirmation, confirmation);
    assert!(!model.can_confirm_cleanup(&editor));
    assert!(model.cancel_cleanup());
    assert!(editor.draft.iter().all(|row| row.enabled));
}

#[test]
fn reset_waits_for_matching_receipt_and_rejects_old_readback_and_foreign_completions() {
    let first = page(5);
    let mut model = RuleStatisticsWorkbench::default();
    model.observe(&first);
    let request = model.begin_reset().unwrap();
    assert!(model.begin_reset().is_none());
    let receipt = RuleStatisticsResetReceipt {
        source: request.source.clone(),
        revision: 8,
        removed_hits: 1,
        removed_rows: 1,
    };
    let mut other = request.clone();
    other.operation += 1;
    assert!(!model.finish_reset(&other, Ok(receipt.clone())));
    assert!(model.busy());
    assert!(model.finish_reset(&request, Ok(receipt)));
    assert!(model.busy());
    assert!(!model.current());
    model.observe(&first);
    assert!(model.busy());
    assert_eq!(model.audit.as_ref().unwrap().revision, 5);
    let mut fresh = page(8);
    let audit = fresh.data.as_mut().unwrap().hit_audit.as_mut().unwrap();
    audit.total_hits = 0;
    audit.can_clear = false;
    model.observe(&fresh);
    assert!(!model.busy());
    assert!(model.current());
    assert_eq!(model.audit.as_ref().unwrap().total_hits, 0);
    model.observe(&first);
    assert_eq!(model.audit.as_ref().unwrap().revision, 8);
    assert_eq!(model.audit.as_ref().unwrap().total_hits, 0);
}

#[test]
fn unqualified_counts_cannot_become_current_or_enable_cleanup_and_reset() {
    let mut observed = page(2);
    let data = observed.data.as_mut().unwrap();
    data.document = None;
    data.hit_audit.as_mut().unwrap().source = None;
    let mut model = RuleStatisticsWorkbench::default();
    model.observe(&observed);
    assert!(!model.current());
    assert!(model.audit.is_none());
    assert!(model.begin_reset().is_none());
    assert!(!model.inspect(&RuleListEditor::default()));
}

#[test]
fn invalid_reset_receipts_preserve_counts_and_retry_allocates_a_new_operation() {
    for invalid in [0, 1, 2] {
        let first = page(5);
        let mut model = RuleStatisticsWorkbench::default();
        model.observe(&first);
        let request = model.begin_reset().unwrap();
        let mut receipt = RuleStatisticsResetReceipt {
            source: request.source.clone(),
            revision: 8,
            removed_hits: 1,
            removed_rows: 1,
        };
        match invalid {
            0 => receipt.revision = 0,
            1 => receipt.source.profile = "foreign.yaml".into(),
            _ => receipt.removed_rows = 2,
        }
        assert!(model.finish_reset(&request, Ok(receipt)));
        assert_eq!(
            model.clear_failure.as_ref().unwrap().code,
            ErrorCode::InvalidState
        );
        assert_eq!(model.audit, first.data.as_ref().unwrap().hit_audit);
        assert!(model.awaiting_revision.is_none());
        let retry = model.begin_reset().unwrap();
        assert_ne!(request.operation, retry.operation);
        assert!(!model.finish_reset(
            &request,
            Err(Failure::new(
                ErrorCode::Permission,
                "late old failure",
                false
            ))
        ));
        assert_eq!(model.clear_pending, Some(retry));
    }
}

#[test]
fn fresh_readback_before_terminal_receipt_completes_without_waiting_for_another_refresh() {
    let mut model = RuleStatisticsWorkbench::default();
    model.observe(&page(5));
    let request = model.begin_reset().unwrap();
    let mut fresh = page(8);
    let audit = fresh.data.as_mut().unwrap().hit_audit.as_mut().unwrap();
    audit.total_hits = 0;
    audit.can_clear = false;
    model.observe(&fresh);
    assert!(model.clear_pending.is_some());
    assert!(model.finish_reset(
        &request,
        Ok(RuleStatisticsResetReceipt {
            source: request.source.clone(),
            revision: 8,
            removed_hits: 1,
            removed_rows: 1
        })
    ));
    assert!(!model.busy());
    assert_eq!(model.clear_failure, None);
    assert_eq!(model.audit.as_ref().unwrap().total_hits, 0);
    model.observe(&page(5));
    assert_eq!(model.audit.as_ref().unwrap().revision, 8);
}
