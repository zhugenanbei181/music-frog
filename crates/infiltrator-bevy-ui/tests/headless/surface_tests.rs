//! Contract-level proof that one shared surface snapshot fans out to every
//! Bevy page projection without a production demo fallback.

use bevy::app::App;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::settings::SettingsProjectionUpdated;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, LatestCoreLifecycle, LatestSurfaceSnapshot, SurfaceSnapshotUpdated,
    SurfaceSource, SurfaceStatusBanner, core_lifecycle_projection, overview_projection,
    settings_projection,
};
use infiltrator_bevy_widgets::theme::LightDark;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::port_conflict::{PortBinding, PortConflict, PortConflictSnapshot};
use infiltrator_contract::session::SessionToken;
use infiltrator_contract::snapshot::{CoreLifecycle, CoreWatchdogSnapshot, CoreWatchdogState};
use infiltrator_contract::surface_snapshot::SurfaceOrigin;

use crate::support::{headless_plugins, page_root, subtree_has_text};

struct StaticSurface {
    snapshot: infiltrator_contract::surface_snapshot::SurfaceSnapshot,
}

fn has_status_banner(app: &mut App, page: infiltrator_contract::surface_snapshot::PageId) -> bool {
    let mut query = app.world_mut().query::<&SurfaceStatusBanner>();
    query.iter(app.world()).any(|banner| banner.page == page)
}

impl infiltrator_bevy_ui::projection::OverviewSource for StaticSurface {
    fn current(&self) -> infiltrator_bevy_ui::projection::OverviewProjection {
        overview_projection(&self.snapshot)
    }

    fn kind(&self) -> infiltrator_bevy_ui::projection::SourceKind {
        infiltrator_bevy_ui::projection::SourceKind::LiveCore
    }
}

impl SurfaceSource for StaticSurface {
    fn surface_snapshot(&self) -> infiltrator_contract::surface_snapshot::SurfaceSnapshot {
        self.snapshot.clone()
    }
}

fn app_with_shared_source() -> App {
    let source = DemoSurfaceSource::running();
    let mut snapshot = source.surface_snapshot();
    snapshot.origin = SurfaceOrigin::Live;
    snapshot.revision = 42;
    snapshot.core.revision = 42;
    snapshot.core.session_token = Some(SessionToken::new(42));
    snapshot.core.core_version = Some("live-contract-test".to_owned());
    snapshot.pages.proxies.data.as_mut().unwrap().active_exit = "live-proxy".to_owned();
    snapshot.pages.profiles.data.as_mut().unwrap().profiles[0].name = "live-profile".to_owned();
    snapshot.pages.rules.data.as_mut().unwrap().default_action = "live-rule".to_owned();
    snapshot
        .pages
        .connections
        .data
        .as_mut()
        .unwrap()
        .connections[0]
        .host = "live-host".to_owned();
    snapshot.pages.logs.data.as_mut().unwrap().entries[0].message = "live-log".to_owned();
    snapshot.pages.dns.data.as_mut().unwrap().fake_ip_range = "live-dns".to_owned();
    snapshot.pages.doctor.data.as_mut().unwrap().last_run = "live-doctor".to_owned();
    snapshot.pages.app_routing.data.as_mut().unwrap().apps[0].name = "live-app".to_owned();
    snapshot.pages.sync.data.as_mut().unwrap().server_url = "live-sync".to_owned();
    snapshot.pages.settings.data.as_mut().unwrap().tun_stack = "live-settings".to_owned();

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(LightDark::Dark, 1180.0));
    app.add_plugins(PagesPlugin::new_surface(StaticSurface { snapshot }));
    app.update();
    app
}

#[test]
fn shared_snapshot_reaches_all_eleven_page_lanes() {
    let mut app = app_with_shared_source();
    let snapshot = app
        .world()
        .resource::<infiltrator_bevy_ui::surface::LatestSurfaceSnapshot>()
        .0
        .clone();
    assert_eq!(snapshot.revision, 42);
    assert_eq!(snapshot.origin, SurfaceOrigin::Live);

    for (route, marker) in [
        (Route::Overview, "live-contract-test"),
        (Route::Proxies, "live-proxy"),
        (Route::Profiles, "live-profile"),
        (Route::Rules, "live-rule"),
        (Route::Connections, "live-host"),
        (Route::Logs, "live-log"),
        (Route::Dns, "live-dns"),
        (Route::Doctor, "live-doctor"),
        (Route::AppRouting, "live-app"),
        (Route::Sync, "live-sync"),
        (Route::Settings, "live-settings"),
    ] {
        app.world_mut().commands().trigger(RouteChanged(route));
        app.update();
        let (root, mounted) = page_root(app.world_mut());
        assert_eq!(mounted, route);
        assert!(
            subtree_has_text(app.world(), root, marker),
            "shared live marker {marker:?} did not reach {route:?}"
        );
        app.world_mut()
            .commands()
            .trigger(SurfaceSnapshotUpdated(snapshot.clone()));
        app.update();
    }
}

#[test]
fn shared_core_lifecycle_projection_tracks_session_generation_and_revision() {
    let mut app = app_with_shared_source();
    let initial = app.world().resource::<LatestCoreLifecycle>().0.clone();
    assert_eq!(initial.lifecycle, CoreLifecycle::Running);
    assert_eq!(initial.generation, 1);
    assert_eq!(initial.session_token.map(|token| token.value()), Some(42));
    assert_eq!(initial.revision, 42);

    let mut next = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    next.revision = 43;
    next.core.revision = 43;
    next.core.lifecycle = CoreLifecycle::Stopped;
    next.core.session_token = None;
    let expected = core_lifecycle_projection(&next);
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(next));
    app.update();

    assert_eq!(app.world().resource::<LatestCoreLifecycle>().0, expected);
    assert_eq!(expected.lifecycle, CoreLifecycle::Stopped);
    assert_eq!(expected.revision, 43);
}

#[test]
fn dual_surface_headless_lifecycle_matrix_covers_failure_conflict_and_stop() {
    let mut app = app_with_shared_source();
    let mut failed = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    failed.revision = 43;
    failed.core.revision = 43;
    failed.core.lifecycle = CoreLifecycle::Failed;
    failed.core.failure = Some(Failure::new(
        ErrorCode::Internal,
        "simulated core start failure",
        true,
    ));
    failed.port_conflicts = PortConflictSnapshot {
        revision: 2,
        conflicts: vec![PortConflict {
            binding: PortBinding::Controller,
            port: 9090,
            available: false,
            owner_pid: Some(4242),
            owner_name: Some("unrelated-app".to_owned()),
            can_release: false,
        }],
    };
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(failed.clone()));
    app.update();
    assert_eq!(
        app.world().resource::<LatestCoreLifecycle>().0.lifecycle,
        CoreLifecycle::Failed
    );

    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Settings));
    app.update();
    app.world_mut()
        .commands()
        .trigger(SettingsProjectionUpdated(settings_projection(&failed)));
    app.update();
    let (root, route) = page_root(app.world_mut());
    assert_eq!(route, Route::Settings);
    assert!(subtree_has_text(app.world(), root, "unrelated-app"));
    assert!(subtree_has_text(app.world(), root, "占用"));

    let mut stopped = failed;
    stopped.revision = 44;
    stopped.core.revision = 44;
    stopped.core.lifecycle = CoreLifecycle::Stopped;
    stopped.core.session_token = None;
    stopped.core.failure = None;
    stopped.port_conflicts = PortConflictSnapshot::default();
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(stopped));
    app.update();
    let lifecycle = &app.world().resource::<LatestCoreLifecycle>().0;
    assert_eq!(lifecycle.lifecycle, CoreLifecycle::Stopped);
    assert!(lifecycle.session_token.is_none());
}

#[test]
fn live_snapshot_reconciles_an_initial_unavailable_banner() {
    let source = DemoSurfaceSource::running();
    let mut initial = source.surface_snapshot();
    initial.pages.proxies = infiltrator_contract::surface_snapshot::PageData::unavailable(
        infiltrator_contract::error::Failure::new(
            infiltrator_contract::error::ErrorCode::NotReady,
            "waiting for proxy reader",
            true,
        ),
    );
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(LightDark::Dark, 1180.0));
    app.add_plugins(PagesPlugin::new_surface(StaticSurface {
        snapshot: initial.clone(),
    }));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    assert!(has_status_banner(
        &mut app,
        infiltrator_contract::surface_snapshot::PageId::Proxies
    ));

    initial.revision = initial.revision.saturating_add(1);
    initial.core.revision = initial.revision;
    initial.pages.proxies = source.surface_snapshot().pages.proxies;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(initial));
    app.update();
    app.update();
    assert!(!has_status_banner(
        &mut app,
        infiltrator_contract::surface_snapshot::PageId::Proxies
    ));
}

#[test]
fn stale_session_snapshot_cannot_replace_a_newer_bevy_projection() {
    let source = DemoSurfaceSource::running();
    let mut snapshot = source.surface_snapshot();
    snapshot.origin = SurfaceOrigin::Live;
    snapshot.generation = 4;
    snapshot.core.generation = 4;
    snapshot.core.session_token = Some(SessionToken::new(40));
    snapshot.revision = 10;
    snapshot.core.revision = 10;

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(LightDark::Dark, 1180.0));
    app.add_plugins(PagesPlugin::new_surface(StaticSurface {
        snapshot: snapshot.clone(),
    }));
    app.update();

    let mut stale = snapshot;
    stale.core.session_token = Some(SessionToken::new(39));
    stale.revision = 11;
    stale.core.revision = 11;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(stale));
    app.update();
    assert_eq!(
        app.world()
            .resource::<infiltrator_bevy_ui::surface::LatestSurfaceSnapshot>()
            .0
            .core
            .session_token,
        Some(SessionToken::new(40))
    );
}

#[test]
fn hot_reload_snapshot_keeps_bevy_generation_and_session_identity() {
    let source = DemoSurfaceSource::running();
    let mut snapshot = source.surface_snapshot();
    snapshot.origin = SurfaceOrigin::Live;
    snapshot.generation = 4;
    snapshot.core.generation = 4;
    snapshot.core.session_token = Some(SessionToken::new(40));
    snapshot.revision = 10;
    snapshot.core.revision = 10;

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(LightDark::Dark, 1180.0));
    app.add_plugins(PagesPlugin::new_surface(StaticSurface {
        snapshot: snapshot.clone(),
    }));
    app.update();

    snapshot.revision = 11;
    snapshot.core.revision = 11;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    let latest = &app
        .world()
        .resource::<infiltrator_bevy_ui::surface::LatestSurfaceSnapshot>()
        .0;
    assert_eq!(latest.revision, 11);
    assert_eq!(latest.generation, 4);
    assert_eq!(latest.core.session_token, Some(SessionToken::new(40)));
}

#[test]
fn shared_watchdog_snapshot_reaches_the_bevy_doctor_projection() {
    let source = DemoSurfaceSource::running();
    let mut snapshot = source.surface_snapshot();
    snapshot.origin = SurfaceOrigin::Live;
    snapshot.core.watchdog = CoreWatchdogSnapshot {
        state: CoreWatchdogState::Tripped { attempts: 3 },
        session_token: Some(SessionToken::new(41)),
        consecutive_failures: 3,
        last_error: None,
    };

    let projection = infiltrator_bevy_ui::surface::doctor_projection(&snapshot);
    assert_eq!(
        projection.watchdog.state,
        CoreWatchdogState::Tripped { attempts: 3 }
    );
    assert_eq!(
        projection.watchdog.session_token,
        Some(SessionToken::new(41))
    );
}
