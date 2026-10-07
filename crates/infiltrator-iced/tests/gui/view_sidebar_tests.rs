use super::*;
use crate::types::message::Message;
use infiltrator_application::runtime_control_projection::{
    RuntimeControlApplication, runtime_control,
};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_toggle::{SystemToggleSnapshot, SystemToggleState};
use infiltrator_domain::runtime::TrafficData;
use infiltrator_domain::runtime::{ConfigSnapshot, TunSnapshot};
use infiltrator_ports::error::PortError;

#[test]
fn shared_observation_replay_keeps_mode_and_tun_unknown_and_preserves_failed_readback_values() {
    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("fixture has no controller read yet"),
    );
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.proxy_mode = None;
    snapshot.revision = 1;
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(state.runtime.proxy_mode, None);
    assert!(!state.runtime.script_block_present);
    assert_eq!(state.runtime.system_toggles.tun, SystemToggleState::Unknown);
    let owner = RuntimeControlApplication::default();
    let config = ConfigSnapshot {
        mode: "global".into(),
        tun: Some(TunSnapshot {
            enable: Some(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    snapshot.runtime_control = owner.observe(0, 2, Some(&Ok(config)));
    snapshot.revision = 2;
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("global"));
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Disabled
    );
    let denied = Failure::new(ErrorCode::Authentication, "controller denied", false);
    snapshot.runtime_control = owner.observe(0, 3, Some(&Err(PortError::Rejected(denied.clone()))));
    snapshot.revision = 3;
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("global"));
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Failed { failure: denied }
    );
    assert!(!state.runtime.system_toggles.tun.can_toggle());
    assert_eq!(runtime_control(None).tun_enabled, None);
}

#[test]
fn test_mode_ids() {
    assert_eq!(mode_ids(), vec!["rule", "global", "direct", "script"]);
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
    state.shell.readout.rules.value = Some(0);
    state.shell.readout.rules.current = true;
    state.shell.readout.upload_bps.value = Some(0.0);
    state.shell.readout.upload_bps.current = true;
    assert_eq!(
        count_copy(&state.shell.readout, PageId::Rules, "en-US"),
        "0"
    );
    assert_eq!(rate_copy(&state.shell.readout.upload_bps, "en-US"), "0 B/s");
}

#[test]
fn test_sidebar_render_smoke() {
    let (state, _) = AppState::new();
    let _elem = sidebar(&state);

    let (mut state2, _) = AppState::new();
    state2.runtime.script_block_present = true;
    state2.runtime.system_proxy_enabled = true;
    state2.runtime.tun_enabled = Some(true);
    state2.shell.current_route = Route::Proxies;
    let _elem_active = sidebar(&state2);
}

#[test]
fn test_sidebar_render_with_traffic_and_samples() {
    let (mut state, _) = AppState::new();
    state.diag.traffic = Some(TrafficData {
        up: 1024 * 50,
        down: 1024 * 1024 * 2,
    });
    state.diag.traffic_history.push_back((100, 200));
    state.diag.traffic_history.push_back((300, 800));
    state.diag.traffic_history.push_back((500, 1200));

    let _elem = sidebar(&state);
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
