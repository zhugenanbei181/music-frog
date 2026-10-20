//! test-intent: behavior
use crate::operation_phase::{OperationPhase, OperationState};
use lurker_core_contract::error::{ErrorCode, Failure};

fn failure(message: &str) -> Failure {
    Failure::new(ErrorCode::Internal, message, false)
}

#[test]
fn an_operation_is_idle_until_it_is_submitted() {
    let phase = OperationPhase::default();
    assert_eq!(phase.state(), &OperationState::Idle);
    assert!(!phase.is_running());
    assert!(phase.failure().is_none());
    assert!(phase.token().is_none());
}

#[test]
fn a_submitted_operation_reports_running_until_the_owner_answers() {
    let mut phase = OperationPhase::default();
    let token = phase.begin();

    assert!(phase.is_running());
    assert_eq!(phase.token(), Some(token));
    assert!(phase.failure().is_none());

    assert!(phase.succeed(token));
    assert!(!phase.is_running());
    assert_eq!(phase.state(), &OperationState::Succeeded { token });
}

#[test]
fn a_refused_operation_keeps_the_typed_failure() {
    let mut phase = OperationPhase::default();
    let token = phase.begin();

    assert!(phase.fail(token, failure("host refused")));
    assert!(!phase.is_running());
    assert_eq!(
        phase.failure().map(|failure| failure.message.as_str()),
        Some("host refused")
    );
}

#[test]
fn a_superseded_answer_never_moves_the_phase() {
    let mut phase = OperationPhase::default();
    let stale = phase.begin();
    let current = phase.begin();

    assert_ne!(stale, current);
    assert!(!phase.succeed(stale));
    assert!(!phase.fail(stale, failure("stale")));
    assert!(phase.is_running());
    assert_eq!(phase.token(), Some(current));

    assert!(phase.succeed(current));
    assert_eq!(phase.state(), &OperationState::Succeeded { token: current });
}

#[test]
fn resetting_returns_the_operation_to_idle() {
    let mut phase = OperationPhase::default();
    let token = phase.begin();
    assert!(phase.succeed(token));

    phase.reset();
    assert_eq!(phase.state(), &OperationState::Idle);
    assert!(phase.token().is_none());
    assert!(phase.failure().is_none());
}
