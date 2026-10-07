//! test-intent: behavior
//! Injected clock instants distinguish a baseline, an observed zero, a reset and an old session.
use super::*;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use std::time::Duration;

fn core(session: u128) -> CoreSnapshot {
    let mut core = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("fixture"),
    )
    .core;
    core.generation = 1;
    core.session_token = Some(SessionToken::new(session));
    core.lifecycle = CoreLifecycle::Running;
    core.failure = None;
    core
}
fn sample(epoch: i64, up: u64, down: u64) -> OverviewSample {
    OverviewSample {
        lifecycle: CoreLifecycle::Running,
        mode: None,
        upload_total: up,
        download_total: down,
        active_connections: 3,
        memory_bytes: Some(2048),
        core_version: Some("actual-version".into()),
        sampled_at_epoch_ms: Some(epoch),
    }
}

#[test]
fn baseline_duplicate_out_of_order_counter_reset_and_session_switch_never_invent_a_current_zero() {
    let mut state = TelemetryState::default();
    let now = Instant::now();
    let baseline = state.apply(core(1), sample(10, 100, 200), now);
    assert!(baseline.sampled_at_epoch_ms.is_none());
    let current = state.apply(core(1), sample(11, 120, 240), now + Duration::from_secs(2));
    assert_eq!((current.upload_bps, current.download_bps), (10.0, 20.0));
    assert_eq!(current.sampled_at_epoch_ms, Some(11));
    assert_eq!(current.memory_bytes, Some(2048));
    let duplicate = state.apply(core(1), sample(11, 999, 999), now + Duration::from_secs(3));
    assert!(duplicate.sampled_at_epoch_ms.is_none());
    let old = state.apply(core(1), sample(9, 1000, 1000), now + Duration::from_secs(4));
    assert!(old.sampled_at_epoch_ms.is_none());
    let zero = state.apply(core(1), sample(12, 120, 240), now + Duration::from_secs(5));
    assert_eq!((zero.upload_bps, zero.download_bps), (0.0, 0.0));
    assert_eq!(zero.sampled_at_epoch_ms, Some(12));
    let reset = state.apply(core(1), sample(13, 0, 0), now + Duration::from_secs(6));
    assert!(reset.sampled_at_epoch_ms.is_none());
    let recovered = state.apply(core(1), sample(14, 5, 8), now + Duration::from_secs(7));
    assert_eq!((recovered.upload_bps, recovered.download_bps), (5.0, 8.0));
    let switched = state.apply(core(2), sample(15, 10, 10), now + Duration::from_secs(8));
    assert!(switched.sampled_at_epoch_ms.is_none());
}
