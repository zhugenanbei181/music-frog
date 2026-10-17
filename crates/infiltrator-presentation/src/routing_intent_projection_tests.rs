//! test-intent: behavior
use super::*;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::EngineId;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::EngineDefinedRoutingIntent;
use lurker_core_contract::routing_intent::RoutingIntentExtensionId;
use lurker_ui_presentation::routing_intent_actions::RoutingIntentFinishOutcome;

#[test]
fn captions_use_the_shared_typed_intent_and_status() {
    let mut snapshot = RoutingIntentSnapshot::default();
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("en")),
        "Not observed"
    );
    snapshot.current = Some(RoutingIntent::FollowRules);
    snapshot.status = RoutingIntentStatus::Ready;
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("en")),
        "Follow rules"
    );
    snapshot.current = Some(RoutingIntent::RouteAllThroughProxy);
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("en")),
        "Route all traffic through proxy"
    );
    snapshot.status = RoutingIntentStatus::Pending;
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("en")),
        "Route all traffic through proxy (updating)"
    );
    snapshot.status = RoutingIntentStatus::Failed;
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("en")),
        "Route all traffic through proxy (stale)"
    );
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("zh-CN")),
        "全部流量经代理（已失效）"
    );
}

#[test]
fn engine_defined_readbacks_are_described_without_becoming_shared_choices() {
    let intent = RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
        EngineId::new("mihomo").unwrap(),
        RoutingIntentExtensionId::new("script").unwrap(),
    ));
    assert_eq!(
        routing_intent_copy(&intent, &Lang("en")),
        "Engine-defined: script"
    );
    let snapshot = RoutingIntentSnapshot {
        current: Some(intent),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    assert_eq!(
        routing_intent_status_copy(&snapshot, &Lang("en")),
        "Engine-defined: script"
    );
    let mut actions = RoutingIntentActions::default();
    actions.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    assert!(actions.begin(snapshot.current.unwrap()).is_err());
}

#[test]
fn failure_copy_uses_the_typed_requested_intent_and_redacted_reason() {
    let mut actions = RoutingIntentActions::default();
    actions.observe(1, 1, RoutingIntentSnapshot::demo_fixture());
    let pending = actions.begin(RoutingIntent::BypassProxy).unwrap();
    let failure = Failure::new(ErrorCode::Network, "connection refused", true);
    assert_eq!(
        actions.finish(pending, Err(failure)),
        RoutingIntentFinishOutcome::Applied
    );
    assert_eq!(
        routing_intent_failure_copy(&actions, &Lang("en")),
        "Switch to Bypass proxy failed: connection refused"
    );
}

#[test]
fn lifecycle_projection_keeps_pending_stopped_and_failed_distinct() {
    let mut surface = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("surface unavailable"),
    );
    surface.core.routing_intent.current = Some(RoutingIntent::FollowRules);
    surface.core.routing_intent.status = RoutingIntentStatus::Ready;
    surface.core.lifecycle = CoreLifecycle::Starting;
    assert_eq!(from_surface(&surface).status, RoutingIntentStatus::Pending);

    surface.core.lifecycle = CoreLifecycle::Stopped;
    assert_eq!(
        from_surface(&surface).status,
        RoutingIntentStatus::Unobserved
    );

    let failure = Failure::new(ErrorCode::Network, "owner read failed", true);
    surface.core.lifecycle = CoreLifecycle::Failed;
    surface.core.failure = Some(failure.clone());
    let projected = from_surface(&surface);
    assert_eq!(projected.status, RoutingIntentStatus::Failed);
    assert_eq!(projected.failure, Some(failure));
}

#[test]
fn verified_intent_updates_the_snapshot_with_a_new_revision() {
    let surface = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("surface unavailable"),
    );
    let next = with_verified_intent(&surface, RoutingIntent::BypassProxy);
    assert_eq!(next.revision, surface.revision + 1);
    assert_eq!(
        next.core.routing_intent.current,
        Some(RoutingIntent::BypassProxy)
    );
    assert_eq!(next.core.routing_intent.status, RoutingIntentStatus::Ready);
}
