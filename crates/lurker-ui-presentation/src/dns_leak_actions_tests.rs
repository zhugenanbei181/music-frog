use super::*;
#[test]
fn stale_feedback_cannot_complete_new_probe_and_permission_recovery_is_explicit() {
    let mut state = DnsLeakActionState::default();
    assert!(state.begin_retry().is_err());
    let first = state.begin().unwrap();
    assert!(state.begin().is_err());
    assert_eq!(state.finish(first + 1, Ok(())), DnsLeakFinish::Stale);
    assert_eq!(state.phase, DnsLeakActionPhase::Pending { token: first });
    assert_eq!(
        state.finish(
            first,
            Err(Failure::new(
                ErrorCode::Permission,
                "allow echo probe",
                false
            ))
        ),
        DnsLeakFinish::Applied
    );
    assert!(state.can_retry());
    let second = state.begin_retry().unwrap();
    assert_eq!(state.phase, DnsLeakActionPhase::Pending { token: second });
    assert_eq!(state.finish(first, Ok(())), DnsLeakFinish::Stale);
    assert_eq!(state.phase, DnsLeakActionPhase::Pending { token: second });
    assert_eq!(state.finish(second, Ok(())), DnsLeakFinish::Applied);
    assert_eq!(state.phase, DnsLeakActionPhase::Idle);
}
