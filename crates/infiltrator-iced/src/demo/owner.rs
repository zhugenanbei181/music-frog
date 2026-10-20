//! Bind each isolated demo capture owner through its accepted shared surface.
use crate::state::AppState;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::SurfaceOrigin;

pub(super) fn attach_capture_owner(state: &mut AppState, application: CoreApplication) {
    let snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(
            ErrorCode::NotReady,
            "capture owner has not published its page snapshot yet",
            true,
        ),
    );
    attach_capture_owner_with_snapshot(state, application, snapshot);
}

pub(super) fn attach_capture_owner_with_snapshot(
    state: &mut AppState,
    application: CoreApplication,
    mut snapshot: SurfaceSnapshot,
) {
    assert!(
        state.shell.demo,
        "capture owners are only valid in demo mode"
    );

    // A capture owns an isolated Core instance. Start a new observation epoch
    // instead of carrying a demo surface from another Core identity forward.
    state.surface = Default::default();
    let core = application.snapshot();
    snapshot.origin = SurfaceOrigin::Demo;
    snapshot.revision = 1;
    snapshot.generation = core.generation;
    // A snapshot that already carries the owner's own core projection keeps
    // it: that projection is the owner's real readback (the routing intent it
    // really observed, for instance) and must not be replaced by a bare
    // lifecycle snapshot. A hand-made snapshot without one is completed from
    // the owner.
    if snapshot.core.identity != core.identity {
        snapshot.core = core;
    }

    state.attach_product_application(application);
    assert!(
        state.apply_shared_surface_snapshot(snapshot),
        "capture owner must be bound by an accepted surface snapshot from that owner"
    );
}
