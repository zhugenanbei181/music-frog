//! test-intent: behavior
use super::*;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::routing_intent::EngineDefinedRoutingIntent;
use infiltrator_contract::routing_intent::RoutingIntentExtensionId;

fn common_target() -> RoutingIntent {
    RoutingIntent::RouteAllThroughProxy
}

#[test]
fn stale_receipts_and_old_generations_cannot_publish_or_unlock_a_new_request() {
    let mut state = RoutingIntentActions::default();
    state.observe(4, 1, RoutingIntentSnapshot::demo_fixture());
    let first = state.begin(common_target()).unwrap();
    assert_eq!(
        state.render_snapshot().current,
        Some(RoutingIntent::FollowRules)
    );
    assert_eq!(state.render_snapshot().status, RoutingIntentStatus::Pending);
    assert!(state.begin(RoutingIntent::BypassProxy).is_err());
    state.observe(5, 1, RoutingIntentSnapshot::demo_fixture());
    let next = state.begin(RoutingIntent::BypassProxy).unwrap();
    assert!(!state.finish(first.clone(), Ok(common_target())));
    assert_eq!(state.pending, Some(next.clone()));
    assert!(state.finish(next, Ok(RoutingIntent::BypassProxy)));
    state.observe(
        5,
        2,
        RoutingIntentSnapshot {
            current: Some(RoutingIntent::BypassProxy),
            status: RoutingIntentStatus::Ready,
            failure: None,
        },
    );
    state.observe(
        5,
        3,
        RoutingIntentSnapshot {
            current: Some(common_target()),
            status: RoutingIntentStatus::Ready,
            failure: None,
        },
    );
    assert_eq!(state.observed.current, Some(common_target()));
}

#[test]
fn write_failure_keeps_observation_and_retry_needs_live_facts() {
    let mut state = RoutingIntentActions::default();
    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let pending = state.begin(common_target()).unwrap();
    let failure = Failure::new(ErrorCode::Network, "routing write unavailable", true);
    assert!(state.finish(pending.clone(), Err(failure.clone())));
    assert_eq!(state.observed.status, RoutingIntentStatus::Ready);
    assert_eq!(state.failure, Some(failure));
    assert_eq!(state.retry_target(), Some(common_target()));
    state.observe(1, 2, RoutingIntentSnapshot::demo_fixture());
    assert!(state.failure.is_some());
    let retry = state.begin(state.retry_target().unwrap()).unwrap();
    assert!(!state.finish(pending, Ok(common_target())));
    let denied = Failure::new(ErrorCode::Authentication, "authentication required", false);
    assert!(state.finish(retry, Err(denied.clone())));
    assert_eq!(state.failure, Some(denied));
    assert!(state.needs_controller_settings());
    assert_eq!(state.retry_target(), None);
    state.dismiss_failure();
    assert_eq!(state.failure, None);
    assert!(!state.needs_controller_settings());
    let mismatch = state.begin(common_target()).unwrap();
    assert!(state.finish(mismatch, Ok(RoutingIntent::BypassProxy)));
    assert_eq!(
        state.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert_eq!(state.observed.current, Some(RoutingIntent::BypassProxy));
}

#[test]
fn engine_defined_intents_are_not_shared_selectable_choices() {
    let mut state = RoutingIntentActions::default();
    state.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let extension = EngineDefinedRoutingIntent::new(
        EngineId::new("mihomo").unwrap(),
        RoutingIntentExtensionId::new("script").unwrap(),
    );
    assert!(
        state
            .begin(RoutingIntent::EngineDefined(extension))
            .is_err()
    );
}
