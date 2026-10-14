use super::*;

#[test]
fn busy_and_stale_results_cannot_complete_or_replace_a_request_and_retry_keeps_its_identity() {
    let mut state = DoctorActionState::default();
    assert!(state.begin_retry().is_err());
    let first = state
        .begin(DoctorAction::RepairOne("config.integrity".into()))
        .unwrap();
    assert!(state.begin(DoctorAction::Bootstrap).is_err());
    assert_eq!(
        state.finish(first.token + 1, Ok(())),
        DoctorActionFinish::Stale
    );
    assert_eq!(state.phase, DoctorActionPhase::Pending(first.clone()));
    let failure = Failure::new(ErrorCode::Permission, "privilege required", true);
    assert_eq!(
        state.finish(first.token, Err(failure.clone())),
        DoctorActionFinish::Applied
    );
    assert_eq!(
        state.phase,
        DoctorActionPhase::Failed {
            failure,
            retry_action: first.action.clone(),
        }
    );
    let second = state.begin_retry().unwrap();
    assert!(second.token > first.token);
    assert_eq!(state.finish(first.token, Ok(())), DoctorActionFinish::Stale);
    assert_eq!(state.phase, DoctorActionPhase::Pending(second.clone()));
    assert_eq!(
        state.finish(second.token, Ok(())),
        DoctorActionFinish::Applied
    );
    assert_eq!(state.phase, DoctorActionPhase::Idle);
}

#[test]
fn canceled_actions_keep_their_typed_terminal_reason_and_can_be_retried() {
    let mut state = DoctorActionState::default();
    let pending = state.begin(DoctorAction::Diagnose).unwrap();
    let cancellation = Failure::new(ErrorCode::Canceled, "diagnostic canceled", true);

    assert_eq!(
        state.finish(pending.token, Err(cancellation.clone())),
        DoctorActionFinish::Applied
    );
    assert_eq!(
        state.phase,
        DoctorActionPhase::Failed {
            failure: cancellation,
            retry_action: DoctorAction::Diagnose,
        }
    );
    assert!(state.can_retry());

    let retry = state.begin_retry().unwrap();
    assert_eq!(retry.action, DoctorAction::Diagnose);
    assert!(retry.token > pending.token);
    assert_eq!(
        state.finish(retry.token, Ok(())),
        DoctorActionFinish::Applied
    );
    assert_eq!(state.phase, DoctorActionPhase::Idle);
}

#[test]
fn token_exhaustion_is_a_typed_rejection_without_a_partial_transition() {
    let mut state = DoctorActionState::default();
    state.next_token = u64::MAX;

    assert_eq!(
        state.begin(DoctorAction::Diagnose).unwrap_err().code,
        ErrorCode::InvalidState
    );
    assert_eq!(state.phase, DoctorActionPhase::Idle);
}
