//! test-intent: behavior
use super::*;

const URI: &str = "vless://b831381d-6324-4d53-ad4f-8cda48b30811@edge.example.test:443#Edge";

fn draft() -> ProtocolDraft {
    draft_from_uri(URI).unwrap()
}

#[test]
fn import_requires_matching_terminal_and_newer_matching_owner_readback() {
    let mut operation = ProtocolEditorOperation::Idle;
    let expected = draft();
    operation
        .begin_import(RequestId::new(7), URI.to_owned(), 3, 20)
        .unwrap();

    assert_eq!(
        operation.finish_import(RequestId::new(6), URI, Ok(())),
        ProtocolEditorEffect::Ignored
    );
    assert!(matches!(
        operation,
        ProtocolEditorOperation::ImportAwaitingTerminal { .. }
    ));
    assert_eq!(
        operation.finish_import(RequestId::new(7), URI, Ok(())),
        ProtocolEditorEffect::ImportAwaitingReadback
    );

    let matching =
        ProtocolStudioSnapshot::from_draft(expected.clone(), Some(URI.to_owned()), Vec::new());
    assert_eq!(
        operation.observe_owner_snapshot(3, 20, &matching),
        ProtocolEditorEffect::Ignored
    );
    let mut foreign = matching.clone();
    foreign.draft = Some(ProtocolDraft::new("ss"));
    assert_eq!(
        operation.observe_owner_snapshot(3, 21, &foreign),
        ProtocolEditorEffect::Ignored
    );
    assert_eq!(
        operation.observe_owner_snapshot(3, 21, &matching),
        ProtocolEditorEffect::Imported(matching)
    );
    assert!(operation.is_idle());
}

#[test]
fn failed_import_is_terminal_and_cannot_publish_a_draft() {
    let mut operation = ProtocolEditorOperation::Idle;
    operation
        .begin_import(RequestId::new(8), URI.to_owned(), 1, 4)
        .unwrap();
    let failure = Failure::new(ErrorCode::InvalidInput, "rejected URI", false);

    assert_eq!(
        operation.finish_import(RequestId::new(8), URI, Err(failure.clone())),
        ProtocolEditorEffect::Failed(failure)
    );
    assert!(operation.is_idle());
}

#[test]
fn save_receipt_is_fenced_by_request_and_exact_draft() {
    let mut operation = ProtocolEditorOperation::Idle;
    let draft = draft();
    operation
        .begin_save(RequestId::new(9), draft.clone())
        .unwrap();
    assert!(operation.is_saving());

    assert_eq!(
        operation.finish_save(RequestId::new(8), &draft, Ok(())),
        ProtocolEditorEffect::Ignored
    );
    assert!(operation.is_saving());

    let failure = Failure::new(ErrorCode::InvalidInput, "save rejected", false);
    assert_eq!(
        operation.finish_save(RequestId::new(9), &draft, Err(failure.clone())),
        ProtocolEditorEffect::Failed(failure)
    );
    assert!(operation.is_idle());

    operation
        .begin_save(RequestId::new(10), draft.clone())
        .unwrap();
    assert_eq!(
        operation.finish_save(RequestId::new(10), &draft, Ok(())),
        ProtocolEditorEffect::Saved
    );
    assert!(operation.is_idle());
}

#[test]
fn import_and_save_are_mutually_exclusive() {
    let mut operation = ProtocolEditorOperation::Idle;
    operation
        .begin_import(RequestId::new(11), URI.to_owned(), 2, 6)
        .unwrap();

    assert_eq!(
        operation
            .begin_save(RequestId::new(12), draft())
            .unwrap_err()
            .code,
        ErrorCode::InvalidState
    );
    assert!(matches!(
        operation,
        ProtocolEditorOperation::ImportAwaitingTerminal { .. }
    ));
}

#[test]
fn cancellation_retires_the_old_request_and_its_late_receipt() {
    let mut operation = ProtocolEditorOperation::Idle;
    operation
        .begin_import(RequestId::new(13), URI.to_owned(), 2, 6)
        .unwrap();
    operation.cancel();

    assert!(operation.is_idle());
    assert_eq!(
        operation.finish_import(RequestId::new(13), URI, Ok(())),
        ProtocolEditorEffect::Ignored
    );
    assert!(operation.is_idle());
}
