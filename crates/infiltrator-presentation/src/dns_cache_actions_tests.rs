//! test-intent: behavior
use crate::dns_cache_actions::DnsCacheActions;
use crate::dns_cache_projection::project_cache;
use infiltrator_contract::dns_cache::{
    DnsCacheFlushReport, DnsCacheOperation, DnsCacheOperationId, DnsCacheSnapshot, DnsFlushOutcome,
};
use lurker_core_contract::error::{ErrorCode, Failure};

#[test]
fn cancellation_duplicate_submit_reader_replay_and_wrong_terminal_are_inert() {
    let mut state = DnsCacheActions::default();
    assert!(state.confirm().is_err());
    state.show();
    assert!(state.cancel());
    assert!(state.confirm().is_err());

    state.show();
    let operation = state.confirm().unwrap();
    assert!(state.confirm().is_err());
    assert!(!state.cancel());
    state.observe(&DnsCacheSnapshot {
        revision: 2,
        report_id: Some(operation),
        ..Default::default()
    });
    assert!(state.is_pending());
    assert!(!state.finish(DnsCacheOperationId(operation.0 + 1), Ok(())));
    let denied = Failure::new(ErrorCode::Permission, "grant cache access", false);
    assert!(state.finish(operation, Err(denied.clone())));
    assert!(state.can_retry());

    state.observe(&DnsCacheSnapshot::default());
    assert_eq!(state.snapshot.revision, 2);
    assert_eq!(state.failure(), Some(&denied));
    let retry = state.retry().unwrap();
    assert!(!state.finish(operation, Ok(())));
    assert!(state.finish(retry, Ok(())));
    assert!(state.failure().is_none());
    assert!(state.cancel());
    assert!(!state.is_open());
}

#[test]
fn matching_readback_may_precede_the_receipt_but_foreign_readback_cannot_finish_it() {
    let mut model = DnsCacheActions::default();
    model.show();
    let operation = model.confirm().unwrap();
    let before = model.snapshot.clone();
    model.observe(&DnsCacheSnapshot {
        revision: 20,
        operation_id: Some(DnsCacheOperationId(operation.0 + 1)),
        report_id: Some(DnsCacheOperationId(operation.0 + 1)),
        operation: DnsCacheOperation::Completed,
        ..Default::default()
    });
    assert_eq!(model.snapshot, before);
    model.observe(&DnsCacheSnapshot {
        revision: 21,
        operation_id: Some(operation),
        report_id: Some(operation),
        operation: DnsCacheOperation::Completed,
        report: DnsCacheFlushReport {
            fake_ip: DnsFlushOutcome::Flushed,
            os_cache: DnsFlushOutcome::Flushed,
        },
        ..Default::default()
    });
    assert!(model.is_pending());
    assert!(model.finish(operation, Ok(())));
    assert!(
        project_cache(&model, "en-US")
            .status
            .contains("Clearing finished")
    );

    model.show();
    let retry = model.confirm().unwrap();
    let permission = Failure::new(ErrorCode::Permission, "allow {reason} verbatim", false);
    assert!(model.finish(retry, Err(permission.clone())));
    let view = project_cache(&model, "zh-CN");
    assert!(view.retry);
    assert!(view.status.contains("allow {reason} verbatim"));
    assert_eq!(model.failure(), Some(&permission));
}
