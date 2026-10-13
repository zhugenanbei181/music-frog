use super::*;
use infiltrator_contract::command::CommandIntent;

fn state(lifecycle: CoreLifecycle) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        lifecycle,
        ..Default::default()
    }
}

#[test]
fn lifecycle_controls_follow_real_transitions_and_never_admit_commands_while_busy() {
    for (lifecycle, expected_label, expected_action) in [
        (
            CoreLifecycle::Stopped,
            CoreControlLabel::Start,
            Some(CoreControlAction::Start),
        ),
        (
            CoreLifecycle::Running,
            CoreControlLabel::Stop,
            Some(CoreControlAction::Stop),
        ),
        (
            CoreLifecycle::Ready,
            CoreControlLabel::Stop,
            Some(CoreControlAction::Stop),
        ),
        (
            CoreLifecycle::Failed,
            CoreControlLabel::Retry,
            Some(CoreControlAction::Start),
        ),
        (CoreLifecycle::Starting, CoreControlLabel::Starting, None),
        (CoreLifecycle::Stopping, CoreControlLabel::Stopping, None),
    ] {
        let core = state(lifecycle);
        let projection = project_core_control(&core, Some(&Availability::Supported), false, None);
        assert_eq!(
            (projection.label, projection.action),
            (expected_label, expected_action)
        );
        let pending = project_core_control(&core, Some(&Availability::Supported), true, None);
        assert_eq!(pending.label, CoreControlLabel::Pending);
        assert_eq!(pending.action, None);
    }
}

#[test]
fn absent_composition_and_platform_unsupported_are_distinct_visible_failures() {
    let core = state(CoreLifecycle::Stopped);
    let missing = project_core_control(&core, None, false, None);
    assert_eq!(missing.action, None);
    assert_eq!(missing.label, CoreControlLabel::Unavailable);
    assert_eq!(missing.issue.unwrap().code, ErrorCode::NotReady);
    let unsupported = project_core_control(
        &core,
        Some(&Availability::Unsupported {
            reason: "platform has no process host".into(),
        }),
        false,
        None,
    );
    assert_eq!(unsupported.action, None);
    assert_eq!(unsupported.label, CoreControlLabel::Unsupported);
    let failure = unsupported.issue.unwrap();
    assert_eq!(failure.code, ErrorCode::Unsupported);
    assert_eq!(failure.message, "platform has no process host");
}

#[test]
fn a_recoverable_failure_keeps_the_original_failure_identity_and_retry_action() {
    let failure = Failure::new(ErrorCode::Permission, "permission must be granted", true);
    let mut core = state(CoreLifecycle::Failed);
    core.failure = Some(failure.clone());
    let projected = project_core_control(&core, Some(&Availability::Supported), false, None);
    assert_eq!(projected.action.unwrap().intent(), CommandIntent::StartCore);
    assert_eq!(projected.issue, Some(failure));
}
