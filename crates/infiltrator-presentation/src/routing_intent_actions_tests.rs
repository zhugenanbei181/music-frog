//! test-intent: behavior
use super::*;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::routing_intent::{EngineDefinedRoutingIntent, RoutingIntentExtensionId};

fn common_target() -> RoutingIntent {
    RoutingIntent::RouteAllThroughProxy
}

#[test]
fn newer_generation_and_same_generation_readback_revoke_late_receipts() {
    let mut state = RoutingIntentActions::default();
    assert_eq!(
        state.observe(4, 1, RoutingIntentSnapshot::demo_fixture()),
        RoutingIntentObserveOutcome::Initialized
    );
    let old = state.begin(common_target()).unwrap();
    assert_eq!(state.render_snapshot().status, RoutingIntentStatus::Pending);

    assert_eq!(
        state.observe(5, 1, RoutingIntentSnapshot::demo_fixture()),
        RoutingIntentObserveOutcome::GenerationChanged
    );
    let change = state.begin(RoutingIntent::BypassProxy).unwrap();
    assert_eq!(
        state.finish(old, Ok(common_target())),
        RoutingIntentFinishOutcome::IgnoredStale
    );
    assert_eq!(state.pending_change(), Some(&change));

    let readback = RoutingIntentSnapshot {
        current: Some(RoutingIntent::BypassProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    assert_eq!(
        state.observe(5, 2, readback),
        RoutingIntentObserveOutcome::PendingSuperseded
    );
    assert_eq!(
        state.finish(change, Ok(RoutingIntent::BypassProxy)),
        RoutingIntentFinishOutcome::IgnoredStale
    );
    assert_eq!(
        state.phase(),
        &RoutingIntentActionPhase::Observed(RoutingIntentObservation {
            generation: 5,
            revision: 2,
            snapshot: RoutingIntentSnapshot {
                current: Some(RoutingIntent::BypassProxy),
                status: RoutingIntentStatus::Ready,
                failure: None,
            },
        })
    );
}

#[test]
fn successful_terminal_readback_is_published_as_observed_facts() {
    let mut state = RoutingIntentActions::default();
    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let change = state.begin(common_target()).unwrap();
    assert_eq!(
        state.finish(change, Ok(common_target())),
        RoutingIntentFinishOutcome::Applied
    );
    assert_eq!(
        state.render_snapshot(),
        RoutingIntentSnapshot {
            current: Some(common_target()),
            status: RoutingIntentStatus::Ready,
            failure: None,
        }
    );
    assert_eq!(state.next_observation_revision(), 3);
}

#[test]
fn failure_retains_observed_facts_and_retry_dismissal_use_typed_phases() {
    let mut state = RoutingIntentActions::default();
    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let change = state.begin(common_target()).unwrap();
    let failure = Failure::new(ErrorCode::Network, "routing write unavailable", true);
    assert_eq!(
        state.finish(change, Err(failure.clone())),
        RoutingIntentFinishOutcome::Applied
    );
    assert_eq!(state.failure(), Some(&failure));
    assert_eq!(state.retry_target(), Some(common_target()));
    assert_eq!(
        state.render_snapshot().current,
        Some(RoutingIntent::FollowRules)
    );

    assert_eq!(
        state.observe(1, 3, RoutingIntentSnapshot::demo_fixture()),
        RoutingIntentObserveOutcome::Updated
    );
    let retry = state.begin(state.retry_target().unwrap()).unwrap();
    let denied = Failure::new(ErrorCode::Authentication, "authentication required", false);
    assert_eq!(
        state.finish(retry, Err(denied.clone())),
        RoutingIntentFinishOutcome::Applied
    );
    assert_eq!(state.failure(), Some(&denied));
    assert_eq!(state.retry_target(), None);
    assert!(state.needs_controller_settings());
    assert_eq!(
        state.dismiss_failure(),
        RoutingIntentDismissOutcome::Dismissed
    );
    assert_eq!(state.failure(), None);
    assert_eq!(state.requested_target(), None);
    assert_eq!(
        state.dismiss_failure(),
        RoutingIntentDismissOutcome::NoFailure
    );
}

#[test]
fn mismatched_success_keeps_actual_readback_and_records_requested_failure() {
    let mut state = RoutingIntentActions::default();
    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let change = state.begin(common_target()).unwrap();
    assert_eq!(
        state.finish(change, Ok(RoutingIntent::BypassProxy)),
        RoutingIntentFinishOutcome::Applied
    );
    assert_eq!(
        state.render_snapshot().current,
        Some(RoutingIntent::BypassProxy)
    );
    assert_eq!(state.requested_target(), Some(&common_target()));
    assert_eq!(
        state.failure().map(|failure| failure.code.clone()),
        Some(ErrorCode::InvalidState)
    );
}

#[test]
fn engine_defined_intents_are_not_shared_selectable_choices() {
    let mut state = RoutingIntentActions::default();
    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let extension = EngineDefinedRoutingIntent::new(
        EngineId::new("mihomo").unwrap(),
        RoutingIntentExtensionId::new("script").unwrap(),
    );
    let failure = state
        .begin(RoutingIntent::EngineDefined(extension))
        .unwrap_err();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert_eq!(
        state.phase(),
        &RoutingIntentActionPhase::Observed(RoutingIntentObservation {
            generation: 1,
            revision: 1,
            snapshot: RoutingIntentSnapshot::demo_fixture(),
        })
    );
}

#[test]
fn rejected_unavailable_action_is_a_failed_phase_and_cannot_replace_pending_work() {
    let mut state = RoutingIntentActions::default();
    let failure = Failure::new(ErrorCode::NotReady, "application unavailable", true);
    assert_eq!(
        state.record_failure(common_target(), failure.clone()),
        RoutingIntentRecordFailureOutcome::Recorded
    );
    assert_eq!(state.failure(), Some(&failure));
    assert_eq!(state.requested_target(), Some(&common_target()));

    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let change = state.begin(RoutingIntent::BypassProxy).unwrap();
    assert_eq!(
        state.record_failure(common_target(), failure),
        RoutingIntentRecordFailureOutcome::PendingPreserved
    );
    assert_eq!(state.pending_change(), Some(&change));
}

#[test]
fn invalidating_pending_change_cancels_late_receipt_and_keeps_request_identity_monotonic() {
    let mut state = RoutingIntentActions::default();
    state.observe(7, 1, RoutingIntentSnapshot::demo_fixture());
    let cancelled = state.begin(common_target()).unwrap();

    state.invalidate();
    assert_eq!(state.phase(), &RoutingIntentActionPhase::Unobserved);
    assert_eq!(
        state.finish(cancelled.clone(), Ok(common_target())),
        RoutingIntentFinishOutcome::IgnoredStale
    );

    state.observe(7, 2, RoutingIntentSnapshot::demo_fixture());
    let next = state.begin(common_target()).unwrap();
    assert!(next.token > cancelled.token);
    assert_eq!(next.generation, 7);
    assert_eq!(next.revision, 2);
}
