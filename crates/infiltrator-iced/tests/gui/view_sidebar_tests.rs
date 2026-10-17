use super::*;
use crate::types::message::Message;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_toggle::{SystemToggleSnapshot, SystemToggleState};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use lurker_core_contract::routing_intent::RoutingIntentStatus;

#[test]
fn shared_observation_replay_keeps_intent_typed_and_preserves_failed_tun_readback() {
    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("fixture has no controller read yet"),
    );
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.routing_intent = RoutingIntentSnapshot::default();
    snapshot.revision = 1;
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(state.runtime.routing_intent, None);
    assert_eq!(state.runtime.system_toggles.tun, SystemToggleState::Unknown);
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Ready,
        tun_enabled: Some(false),
        ..Default::default()
    };
    snapshot.core.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::RouteAllThroughProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    snapshot.revision = 2;
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Disabled
    );
    let denied = Failure::new(ErrorCode::Authentication, "controller denied", false);
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Failed {
            failure: denied.clone(),
        },
        tun_enabled: Some(false),
        ..Default::default()
    };
    snapshot.revision = 3;
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Failed { failure: denied }
    );
    assert!(!state.runtime.system_toggles.tun.can_toggle());
    assert_eq!(state.runtime.runtime_control.tun_enabled, Some(false));
}

#[test]
fn common_routing_intents_are_shared_choices() {
    assert_eq!(
        common_routing_intents(),
        [
            RoutingIntent::FollowRules,
            RoutingIntent::RouteAllThroughProxy,
            RoutingIntent::BypassProxy,
        ]
    );
}

#[test]
fn test_short_label() {
    assert_eq!(short_label("系统代理 (System Proxy)"), "系统代理");
    assert_eq!(short_label("TUN 模式 (TUN Mode)"), "TUN 模式");
    assert_eq!(short_label("Simple"), "Simple");
}

#[test]
fn unobserved_shell_counts_and_rates_are_distinct_from_observed_zero() {
    let (mut state, _) = AppState::new();
    assert_eq!(
        count_copy(&state.shell.readout, PageId::Rules, "en-US"),
        "Not observed"
    );
    assert_eq!(
        rate_copy(&state.shell.readout.upload_bps, "en-US"),
        "Not observed"
    );
    state.shell.readout.rules = ShellObservation::Current(0);
    state.shell.readout.upload_bps = ShellObservation::Current(0.0);
    assert_eq!(
        count_copy(&state.shell.readout, PageId::Rules, "en-US"),
        "0"
    );
    assert_eq!(rate_copy(&state.shell.readout.upload_bps, "en-US"), "0 B/s");
}
#[test]
fn test_sidebar_rail_render_smoke() {
    let (state, _) = AppState::new();
    let _rail = sidebar_rail(&state);
    assert_eq!(RAIL_WIDTH, 64.0);
}

#[test]
fn system_toggle_sidebar_uses_shared_pending_policy() {
    let (mut state, _) = AppState::new();
    state.runtime.system_proxy_enabled = false;
    state.runtime.system_toggles = SystemToggleSnapshot::from_legacy(false, Some(false), 1);

    let _ = state.update(Message::SetSystemProxy(true));
    assert!(matches!(
        state.runtime.system_toggles.system_proxy,
        SystemToggleState::Pending { desired: true }
    ));

    // The Elm update path and the Bevy observer share the same no-reentry
    // rule: a second click cannot enqueue a conflicting request.
    let _ = state.update(Message::SetSystemProxy(false));
    assert!(matches!(
        state.runtime.system_toggles.system_proxy,
        SystemToggleState::Pending { desired: true }
    ));
}
