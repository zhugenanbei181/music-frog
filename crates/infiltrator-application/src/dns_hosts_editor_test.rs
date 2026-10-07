//! test-intent: behavior
use super::*;
fn entry(domain: &str, address: &str) -> DnsHostEntry {
    DnsHostEntry {
        domain: domain.into(),
        address: address.into(),
    }
}
fn observed(name: &str) -> PageData<DnsHostsProfile> {
    PageData::ready(DnsHostsProfile {
        profile: name.into(),
        entries: vec![entry("live.test", "9.9.9.9")],
        legacy_entries: vec![entry("old.test", "1.1.1.1")],
    })
}
#[test]
fn stable_row_edits_cancel_migration_failure_retry_and_stale_ack_preserve_real_applied_facts() {
    let original = observed("work");
    let mut editor = DnsHostsEditor::default();
    editor.observe(&original);
    editor.show();
    let first = editor.rows[0].id;
    editor.select(first);
    editor.edit_address("8.8.8.8".into());
    assert!(editor.commit_row());
    assert_eq!(editor.rows[0].id, first);
    assert_eq!(
        editor.applied.as_ref().unwrap().entries[0].address,
        "9.9.9.9"
    );
    editor.import_legacy();
    assert_eq!(editor.rows.len(), 2);
    assert!(editor.importing_legacy);
    assert!(editor.cancel());
    assert_eq!(editor.rows.len(), 1);
    assert!(!editor.importing_legacy);
    assert_eq!(editor.rows[0].entry.address, "9.9.9.9");
    editor.show();
    editor.import_legacy();
    let pending = editor.begin().unwrap();
    assert_eq!(pending.patch.expected_profile.as_deref(), Some("work"));
    assert!(pending.patch.remove_legacy_hosts);
    assert!(!editor.cancel());
    assert!(!editor.finish(pending.token + 1, Ok(())));
    assert!(editor.pending.is_some());
    editor.edit_address("discarded input while saving".into());
    assert!(editor.address.is_empty());
    let failure = Failure::new(ErrorCode::Permission, "write denied", true);
    assert!(editor.finish(pending.token, Err(failure.clone())));
    assert_eq!(editor.failure, Some(failure));
    assert_eq!(editor.applied.as_ref().unwrap().entries.len(), 1);
    assert_eq!(editor.rows.len(), 2);
    assert!(editor.importing_legacy);
    let retry = editor.begin().unwrap();
    assert!(editor.finish(retry.token, Ok(())));
    assert_eq!(editor.applied.as_ref().unwrap().entries.len(), 2);
    assert!(editor.applied.as_ref().unwrap().legacy_entries.is_empty());
    editor.observe(&original);
    assert_eq!(editor.applied.as_ref().unwrap().entries.len(), 2);
    assert!(editor.cancel());
    assert_eq!(
        editor.rows.len(),
        2,
        "a stale reader cannot undo a committed write when closing"
    );
}
#[test]
fn failed_loading_changed_profile_and_changed_mapping_never_erase_a_draft_or_allow_stale_apply() {
    let mut editor = DnsHostsEditor::default();
    editor.observe(&observed("one"));
    editor.show();
    editor.edit_address("1.2.3.4".into());
    editor.edit_domain("draft.test".into());
    assert!(editor.commit_row());
    let rows = editor.rows.clone();
    let failure = Failure::new(ErrorCode::Storage, "read failed", true);
    editor.observe(&PageData::failed(failure.clone()));
    assert_eq!(editor.rows, rows);
    assert_eq!(editor.read_failure, Some(failure));
    assert!(!editor.can_apply());
    editor.observe(&PageData::loading());
    assert_eq!(editor.rows, rows);
    assert!(!editor.can_apply());
    editor.observe(&observed("two"));
    assert_eq!(editor.rows, rows);
    assert!(!editor.can_apply());
    assert_eq!(editor.applied.as_ref().unwrap().profile, "one");
    editor.cancel();
    editor.observe(&observed("two"));
    assert!(editor.ready);
    assert_eq!(editor.applied.as_ref().unwrap().profile, "two");
    editor.show();
    editor.edit_address("1.2.3.4".into());
    editor.edit_domain("draft.test".into());
    editor.commit_row();
    let mut changed = observed("two");
    changed.data.as_mut().unwrap().entries[0].address = "8.8.8.8".into();
    editor.observe(&changed);
    assert!(!editor.can_apply());
    assert_eq!(editor.rows[0].entry.address, "9.9.9.9");
}
#[test]
fn a_profile_switch_during_pending_write_cannot_publish_old_rows_into_the_new_profile() {
    let mut editor = DnsHostsEditor::default();
    editor.observe(&observed("old"));
    editor.show();
    editor.import_legacy();
    let pending = editor.begin().unwrap();
    editor.observe(&observed("new"));
    assert_eq!(editor.applied.as_ref().unwrap().profile, "old");
    assert!(!editor.ready);
    editor.finish(pending.token, Ok(()));
    assert_eq!(editor.applied.as_ref().unwrap().profile, "old");
    editor.cancel();
    editor.observe(&observed("new"));
    assert_eq!(editor.applied.as_ref().unwrap().profile, "new");
}
#[test]
fn invalid_row_input_deletion_and_restore_reject_old_row_identities_and_real_empty_patch_clears_root_key()
 {
    let mut editor = DnsHostsEditor::default();
    editor.observe(&observed("work"));
    editor.show();
    editor.edit_address("1.2.3.4".into());
    assert!(!editor.commit_row());
    assert!(!editor.issues.is_empty());
    assert_eq!(editor.rows.len(), 1);
    editor.edit_domain("new.test".into());
    assert!(editor.commit_row());
    let new = editor.rows[1].id;
    editor.remove(new);
    editor.select(new);
    assert_eq!(editor.editing, None);
    let original = editor.rows[0].id;
    editor.remove(original);
    assert!(editor.rows.is_empty());
    let pending = editor.begin().unwrap();
    assert!(pending.patch.clear_hosts);
    assert!(pending.patch.hosts.is_none());
    editor.finish(
        pending.token,
        Err(Failure::new(ErrorCode::Storage, "denied", true)),
    );
    editor.cancel();
    editor.observe(&observed("work"));
    assert_eq!(editor.rows.len(), 1);
    assert_ne!(editor.rows[0].id, original);
    editor.remove(original);
    assert_eq!(editor.rows.len(), 1);
}
