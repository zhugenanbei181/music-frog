//! Bind each isolated demo capture owner through its accepted shared surface.
use crate::state::AppState;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::version::CoreVersionSnapshot;
use infiltrator_test_support::language_choice_fixtures::LanguageCaptureProcess;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::SurfaceOrigin;
use std::sync::Arc;

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

/// Bind an isolated capture owner that publishes a core version journal.
///
/// The kernel card renders the owner's rollback target, so the capture needs
/// an owner that published one; the desktop version port is never contacted
/// because a demo session must not rewrite the installed engine.
pub(super) fn attach_capture_owner_with_versions(
    state: &mut AppState,
    versions: CoreVersionSnapshot,
) {
    let runtime = tokio_application_runtime().expect("isolated capture runtime");
    let application = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        runtime,
    );
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(
            ErrorCode::NotReady,
            "capture owner has not published its page snapshot yet",
            true,
        ),
    );
    snapshot.versions = versions;
    attach_capture_owner_with_snapshot(state, application, snapshot);
}
