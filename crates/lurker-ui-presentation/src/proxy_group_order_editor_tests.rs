use super::*;
use lurker_core_contract::identity::EngineId;

fn id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::new("mihomo").unwrap(), value).unwrap()
}

#[test]
fn draft_moves_reset_cancel_and_correlated_receipts_keep_uncommitted_order_private() {
    let original = vec![id("B"), id("A"), id("C")];
    let default_order = vec![id("A"), id("B"), id("C")];
    let revision = SnapshotRevision::new(12);
    let mut editor = ProxyGroupOrderEditor::default();
    editor
        .open(original.clone(), default_order.clone(), revision)
        .unwrap();

    assert!(!editor.move_group(&id("B"), GroupMove::Up));
    assert!(editor.move_group(&id("C"), GroupMove::Up));
    assert_eq!(editor.draft(), [id("B"), id("C"), id("A")]);
    assert_eq!(editor.baseline(), Some(original.as_slice()));
    editor.cancel();
    assert_eq!(editor.draft(), original);

    editor.reset_draft();
    assert_eq!(editor.draft(), default_order);
    assert_eq!(editor.operation(), GroupOrderOperation::ResetToInventory);
    let pending = editor.begin().unwrap();
    assert_eq!(pending.operation, GroupOrderOperation::ResetToInventory);
    assert!(!editor.move_group(&id("C"), GroupMove::Up));
    assert_eq!(
        editor.finish(pending.token + 1, Ok(())),
        GroupOrderFinish::Stale
    );
    assert!(editor.is_pending());
    assert_eq!(
        editor.finish(
            pending.token,
            Err(Failure::new(ErrorCode::Storage, "write failed", true))
        ),
        GroupOrderFinish::Failed
    );
    assert_eq!(editor.baseline(), Some(original.as_slice()));
    assert_eq!(editor.draft(), default_order);
    assert!(editor.failure().is_some());

    let retry = editor.begin().unwrap();
    assert!(retry.token > pending.token);
    assert_eq!(
        editor.finish(retry.token, Ok(())),
        GroupOrderFinish::Applied
    );
    assert_eq!(editor.baseline(), Some(default_order.as_slice()));
    assert_eq!(
        editor.finish(pending.token, Ok(())),
        GroupOrderFinish::Stale
    );
}

#[test]
fn read_failure_cancel_and_changed_inventory_keep_the_revision_fence() {
    let groups = vec![id("A"), id("B")];
    let revision = SnapshotRevision::new(3);
    let mut editor = ProxyGroupOrderEditor::default();
    editor
        .open(groups.clone(), groups.clone(), revision)
        .unwrap();
    assert!(!editor.can_apply());
    assert!(editor.move_group(&id("B"), GroupMove::Up));

    editor.observe(Err(Failure::new(ErrorCode::Network, "read failed", true)));
    assert!(!editor.can_apply());
    assert!(editor.observation_failure().is_some());
    editor.cancel();
    assert!(
        !editor.can_apply(),
        "cancel must not erase a failed read fence"
    );

    editor.move_group(&id("B"), GroupMove::Up);
    editor.observe(Ok((groups.clone(), revision)));
    assert!(editor.can_apply());
    editor.observe(Ok((vec![id("A"), id("C")], revision)));
    assert!(!editor.can_apply());
    assert!(editor.begin().is_err());

    editor.observe(Ok((groups, SnapshotRevision::new(4))));
    assert!(
        !editor.can_apply(),
        "revision changes fence writes even if IDs match"
    );
    assert!(editor.observation_failure().is_some());
}

#[test]
fn duplicate_and_foreign_order_payloads_are_rejected_before_a_write() {
    let mut editor = ProxyGroupOrderEditor::default();
    let revision = SnapshotRevision::new(8);
    assert!(
        editor
            .open(vec![id("A"), id("A")], vec![id("A"), id("A")], revision)
            .is_err()
    );
    assert!(
        editor
            .open(vec![id("A"), id("B")], vec![id("A"), id("C")], revision)
            .is_err()
    );
    editor
        .open(vec![id("A"), id("B")], vec![id("A"), id("B")], revision)
        .unwrap();
    assert!(editor.move_group(&id("B"), GroupMove::Up));
    assert!(validate_order(&[id("A"), id("A")]).is_err());
}

#[test]
fn token_exhaustion_is_a_typed_rejection_without_a_partial_pending_transition() {
    let groups = vec![id("A"), id("B")];
    let mut editor = ProxyGroupOrderEditor::default();
    editor
        .open(groups.clone(), groups, SnapshotRevision::new(1))
        .unwrap();
    editor.move_group(&id("B"), GroupMove::Up);
    editor.next_token = u64::MAX;

    assert_eq!(editor.begin().unwrap_err().code, ErrorCode::InvalidState);
    assert!(!editor.is_pending());
    assert_eq!(editor.failure().unwrap().code, ErrorCode::InvalidState);
}
