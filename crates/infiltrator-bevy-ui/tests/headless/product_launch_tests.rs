//! test-intent: behavior
//! Product commands and read models share the composed application; failed hosts stay failed.
use bevy::app::App;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use infiltrator_bevy_ui::command::{UiCommand, UiCommandSink};
use infiltrator_bevy_ui::launch::{LaunchMode, UnavailableCommandSink, missing_host_failure};
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::route::SurfaceSourceHandle;
use infiltrator_bevy_ui::surface::{
    LatestSurfaceSnapshot, SurfaceSnapshotUpdated, SurfaceSource, SurfaceSourceRefreshPlugin,
    UnavailableSurfaceSource, overview_projection,
};
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{PageId, PageStatus, SurfaceOrigin, SurfaceSnapshot};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct MutableSurfaceSource {
    snapshot: Mutex<SurfaceSnapshot>,
}

impl MutableSurfaceSource {
    fn set_snapshot(&self, snapshot: SurfaceSnapshot) {
        *self.snapshot.lock().expect("surface snapshot lock") = snapshot;
    }

    fn set_revision(&self, revision: u64) {
        let mut snapshot = self.snapshot.lock().expect("surface snapshot lock");
        snapshot.revision = revision;
        snapshot.core.revision = revision;
    }
}

fn scoped_owner_snapshot(instance: &str, revision: u64) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyAndroid,
        HostKind::Android,
        Failure::new(ErrorCode::NotReady, "owner is starting", true),
    );
    let identity = CoreIdentity::new(
        EngineId::new("mihomo").expect("engine ID"),
        CoreInstanceId::new(instance).expect("core instance ID"),
        None,
    );
    snapshot.revision = revision;
    snapshot.generation = 1;
    snapshot.core.identity = Some(identity);
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.generation = 1;
    snapshot.core.revision = revision;
    snapshot
}

impl SurfaceSource for MutableSurfaceSource {
    fn surface_snapshot(&self) -> SurfaceSnapshot {
        self.snapshot.lock().expect("surface snapshot lock").clone()
    }
}

impl OverviewSource for MutableSurfaceSource {
    fn current(&self) -> OverviewProjection {
        overview_projection(&self.surface_snapshot())
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

#[derive(Resource, Default)]
struct ObservedSurfaceRevisions(Vec<u64>);

fn observe_surface_update(
    update: On<SurfaceSnapshotUpdated>,
    mut observed: ResMut<ObservedSurfaceRevisions>,
) {
    observed.0.push(update.0.revision);
}

#[test]
fn demo_mode_requires_explicit_opt_in() {
    for flag in [None, Some(""), Some("0"), Some("true"), Some("invalid")] {
        assert_eq!(LaunchMode::from_demo_flag(flag), LaunchMode::Product);
    }
    assert_eq!(LaunchMode::from_demo_flag(Some("1")), LaunchMode::Demo);
}

#[test]
fn periodic_host_surface_refresh_emits_only_newer_owner_snapshots() {
    let initial = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyAndroid,
        HostKind::Android,
        Failure::new(ErrorCode::NotReady, "owner is starting", true),
    );
    let source = Arc::new(MutableSurfaceSource {
        snapshot: Mutex::new(initial.clone()),
    });
    let mut app = App::new();
    app.insert_resource(SurfaceSourceHandle(source.clone()));
    app.insert_resource(LatestSurfaceSnapshot(initial));
    app.init_resource::<ObservedSurfaceRevisions>();
    app.add_observer(observe_surface_update);
    app.add_plugins(SurfaceSourceRefreshPlugin::new(Duration::ZERO));

    app.update();
    source.set_revision(1);
    app.update();
    app.update();
    source.set_revision(2);
    app.update();

    assert_eq!(
        app.world().resource::<ObservedSurfaceRevisions>().0,
        vec![1, 2]
    );
}

#[test]
fn periodic_host_surface_refresh_does_not_let_a_foreign_scope_block_the_current_owner() {
    let owner = scoped_owner_snapshot("owner-current", 1);
    let source = Arc::new(MutableSurfaceSource {
        snapshot: Mutex::new(owner.clone()),
    });
    let mut app = App::new();
    app.insert_resource(SurfaceSourceHandle(source.clone()));
    app.insert_resource(LatestSurfaceSnapshot(owner));
    app.init_resource::<ObservedSurfaceRevisions>();
    app.add_observer(observe_surface_update);
    app.add_plugins(SurfaceSourceRefreshPlugin::new(Duration::ZERO));

    app.update();
    source.set_snapshot(scoped_owner_snapshot("owner-foreign", 2));
    app.update();
    source.set_snapshot(scoped_owner_snapshot("owner-current", 2));
    app.update();

    assert_eq!(
        app.world().resource::<ObservedSurfaceRevisions>().0,
        vec![2]
    );
}

#[test]
fn mobile_product_without_a_host_is_not_ready_not_a_silent_demo() {
    // BANDROID-004: an unattached host must surface a typed terminal state,
    // never an implicitly interactive demo over no VPN/kernel.
    let failure = missing_host_failure();
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(failure.retryable);
}

#[test]
fn unattached_product_routing_intent_is_unavailable_not_demo_success() {
    use infiltrator_contract::routing_intent::RoutingIntent;

    let failure = missing_host_failure();
    let sink = UnavailableCommandSink::new(failure.clone());
    let request = sink
        .submit_tracked(UiCommand::SetRoutingIntent(RoutingIntent::BypassProxy))
        .expect("unavailable product sink returns terminal feedback");
    let result = sink
        .drain_results()
        .pop()
        .expect("terminal result is queued");

    assert_eq!(result.request_id, request);
    assert_eq!(
        result.result,
        Err(failure),
        "a hostless product never receives demo routing-intent readback"
    );
}

#[test]
fn failed_host_projects_failure_on_every_page_and_rejects_each_operation_once() {
    let failure = Failure::new(ErrorCode::Storage, "profile directory is read only", true);
    let source =
        UnavailableSurfaceSource::new(SurfaceKind::BevyDesktop, HostKind::Desktop, failure.clone());
    let snapshot = source.surface_snapshot();
    assert_eq!(snapshot.origin, SurfaceOrigin::Live);
    for page in PageId::ALL {
        assert_eq!(
            snapshot.page_status(page),
            PageStatus::Unavailable {
                failure: failure.clone()
            }
        );
    }
    let sink = UnavailableCommandSink::new(failure.clone());
    let first = sink.submit_tracked(UiCommand::StartCore).unwrap();
    let second = sink.submit_tracked(UiCommand::CloseAllConnections).unwrap();
    assert_ne!(first, second);
    let results = sink.drain_results();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].request_id, first);
    assert_eq!(results[1].request_id, second);
    for result in results {
        assert!(result.result.is_err());
        assert_eq!(result.result.as_ref().err(), Some(&failure));
    }
    assert!(sink.drain_results().is_empty());
}
