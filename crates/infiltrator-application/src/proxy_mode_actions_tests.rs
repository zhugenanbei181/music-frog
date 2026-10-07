//! test-intent: behavior
use super::*;

#[test]
fn stale_receipts_and_old_generations_cannot_publish_or_unlock_a_new_mode_request() {
    let mut state = ProxyModeActions::default();
    state.observe(4, 1, ProxyModeSnapshot::demo_fixture());
    let first = state.begin(ProxyMode::Global).unwrap();
    assert_eq!(state.render_snapshot().current, Some(ProxyMode::Rule));
    assert_eq!(state.render_snapshot().status, ProxyModeStatus::Pending);
    assert!(state.begin(ProxyMode::Direct).is_err());
    state.observe(5, 1, ProxyModeSnapshot::demo_fixture());
    let next = state.begin(ProxyMode::Direct).unwrap();
    assert!(!state.finish(first, Ok(ProxyMode::Global)));
    assert_eq!(state.pending, Some(next));
    assert!(state.finish(next, Ok(ProxyMode::Direct)));
    state.observe(5, 1, ProxyModeSnapshot::demo_fixture());
    assert_eq!(state.observed.current, Some(ProxyMode::Direct));
    let mut current = ProxyModeSnapshot::demo_fixture();
    current.current = Some(ProxyMode::Direct);
    state.observe(5, 2, current.clone());
    let mut external_change = current;
    external_change.current = Some(ProxyMode::Global);
    state.observe(5, 3, external_change);
    assert_eq!(state.observed.current, Some(ProxyMode::Global));
}

#[test]
fn write_failure_remains_distinct_from_read_status_and_retry_requires_a_retryable_failure_and_live_facts()
 {
    let mut state = ProxyModeActions::default();
    state.observe(1, 1, ProxyModeSnapshot::demo_fixture());
    let pending = state.begin(ProxyMode::Global).unwrap();
    let failure = Failure::new(ErrorCode::Network, "mode write unavailable", true);
    assert!(state.finish(pending, Err(failure.clone())));
    assert_eq!(state.observed.status, ProxyModeStatus::Ready);
    assert_eq!(state.failure, Some(failure));
    assert_eq!(state.retry_target(), Some(ProxyMode::Global));
    state.observe(1, 2, ProxyModeSnapshot::demo_fixture());
    assert!(state.failure.is_some());
    let retry = state.begin(state.retry_target().unwrap()).unwrap();
    assert!(!state.finish(pending, Ok(ProxyMode::Global)));
    let denied = Failure::new(ErrorCode::Authentication, "authentication required", false);
    assert!(state.finish(retry, Err(denied.clone())));
    assert_eq!(state.failure, Some(denied));
    assert!(state.needs_controller_settings());
    assert_eq!(state.retry_target(), None);
    state.dismiss_failure();
    assert_eq!(state.failure, None);
    assert!(!state.needs_controller_settings());
    let mismatch = state.begin(ProxyMode::Global).unwrap();
    assert!(state.finish(mismatch, Ok(ProxyMode::Direct)));
    assert_eq!(
        state.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert_eq!(state.observed.current, Some(ProxyMode::Direct));
}

#[test]
fn config_reads_crossing_a_write_or_retirement_cannot_overwrite_verified_mode() {
    let mut state = ProxyModeActions::default();
    state.observe(1, 1, ProxyModeSnapshot::demo_fixture());
    let before_write = state.read_epoch();
    assert!(state.accepts_read(before_write));
    let first = state.begin(ProxyMode::Global).unwrap();
    let during_write = state.read_epoch();
    assert!(!state.accepts_read(before_write));
    assert!(!state.accepts_read(during_write));
    state.finish(first, Ok(ProxyMode::Global));
    assert!(!state.accepts_read(before_write));
    assert!(!state.accepts_read(during_write));
    let fresh = state.read_epoch();
    assert!(state.accepts_read(fresh));
    state.invalidate();
    assert!(!state.accepts_read(fresh));
    state.observe(2, 1, ProxyModeSnapshot::demo_fixture());
    let second = state.begin(ProxyMode::Direct).unwrap();
    assert_ne!(second.token, first.token);
    assert!(!state.finish(first, Ok(ProxyMode::Global)));
    assert_eq!(state.pending, Some(second));
}
