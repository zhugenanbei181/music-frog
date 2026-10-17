//! test-intent: behavior
//! Native controls execute actual lifecycle ports through the production command sink.
use crate::command_harness::install_test_core_capability_application;
use crate::support::{command_pump_for_observed_snapshot, headless_plugins, subtree_has_text};
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ui::{Display, Node};
use bevy::ui_widgets::Activate;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::command_execution::ApplicationCommandSink;
use infiltrator_bevy_ui::pages::overview_lifecycle::{
    CoreControlButton, CoreControlIssue, CoreControlPhase, CoreControlState,
};
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::route::PagesPlugin;
use infiltrator_bevy_ui::surface::{
    DemoSurfaceSource, SurfaceSnapshotUpdated, SurfaceSource, overview_projection,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::capability::{Availability, CapabilityStatus};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::core_capabilities::{CoreLifecycleOperation, CoreOperation};
use infiltrator_contract::surface_snapshot::{SurfaceOrigin, SurfaceSnapshot};
use infiltrator_ports::core_scope::CoreScopeObservation;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, CoreVersion, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::thread::yield_now;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Process {
    running: AtomicBool,
    deny_start: AtomicBool,
    starts: AtomicUsize,
    stops: AtomicUsize,
}
#[async_trait::async_trait]
impl CoreProcess for Process {
    fn core_identity(&self) -> Option<CoreIdentity> {
        Some(CoreIdentity::new(
            EngineId::new("mihomo").expect("test engine id"),
            CoreInstanceId::new("lifecycle-test").expect("test instance id"),
            Some(CoreVersion::new("1.19.12").expect("test engine version")),
        ))
    }

    async fn start(&self, _context: CoreProcessStartContext) -> Result<(), PortError> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        if self.deny_start.load(Ordering::SeqCst) {
            return Err(PortError::PermissionDenied(
                "grant process permission".into(),
            ));
        }
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }
    async fn stop(&self) -> Result<(), PortError> {
        self.stops.fetch_add(1, Ordering::SeqCst);
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }
    async fn status(&self) -> Result<CoreLifecycle, PortError> {
        Ok(if self.running.load(Ordering::SeqCst) {
            CoreLifecycle::Running
        } else {
            CoreLifecycle::Stopped
        })
    }
    fn controller_endpoint(&self) -> Option<String> {
        None
    }
}
#[async_trait::async_trait]
impl CoreReadiness for Process {
    async fn probe(&self) -> Result<String, PortError> {
        Ok("test-core".into())
    }

    async fn active_core_version(&self) -> Result<Option<CoreVersion>, PortError> {
        Ok(Some(
            CoreVersion::new("1.19.12").expect("test engine version"),
        ))
    }
}
#[derive(Clone)]
struct HostSource(Arc<CoreApplication>);
impl OverviewSource for HostSource {
    fn current(&self) -> OverviewProjection {
        overview_projection(&self.surface_snapshot())
    }
    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}
impl SurfaceSource for HostSource {
    fn surface_snapshot(&self) -> SurfaceSnapshot {
        let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
        snapshot.origin = SurfaceOrigin::Live;
        snapshot.core = self.0.snapshot();
        snapshot.generation = snapshot.core.generation;
        snapshot.revision = snapshot.core.revision;
        snapshot.capabilities.entries = vec![CapabilityStatus {
            capability: Capability::CoreLifecycle,
            availability: Availability::Supported,
        }];
        snapshot
    }
}
fn setup() -> (App, Arc<Process>, HostSource) {
    let process = Arc::new(Process::default());
    let application = Arc::new(CoreApplication::new(
        process.clone(),
        process.clone(),
        tokio_application_runtime().unwrap(),
    ));
    install_test_core_capability_application(
        &application,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        ],
    );
    let source = HostSource(application.clone());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new_surface(source.clone()));
    app.add_plugins(command_pump_for_observed_snapshot(application));
    app.update();
    (app, process, source)
}
fn button(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &CoreControlButton)>()
        .single(app.world())
        .unwrap()
        .0
}
fn activate(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.world_mut().flush();
}
fn until(app: &mut App, predicate: impl Fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !predicate(app) {
        assert!(
            Instant::now() < deadline,
            "production command must publish terminal feedback"
        );
        app.update();
        yield_now();
    }
}
fn publish(app: &mut App, source: &HostSource) {
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(source.surface_snapshot()));
    app.update();
}

fn wait_for_sink_result(
    sink: &ApplicationCommandSink,
    request_id: RequestId,
) -> CommandExecutedEvent {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(result) = sink
            .drain_results()
            .into_iter()
            .find(|result| result.request_id == request_id)
        {
            return result;
        }
        assert!(Instant::now() < deadline, "scoped sink must acknowledge");
        yield_now();
    }
}

#[test]
fn surface_observed_sink_rejects_stale_scope_and_accepts_rebound_scope() {
    let process = Arc::new(Process::default());
    let application = Arc::new(CoreApplication::new(
        process.clone(),
        process.clone(),
        tokio_application_runtime().unwrap(),
    ));
    install_test_core_capability_application(
        &application,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Stop),
        ],
    );
    let observed_scope = Arc::new(RwLock::new(CoreScopeObservation::Observed(
        application.snapshot().scope().expect("initial core scope"),
    )));
    let sink = ApplicationCommandSink::new_for_surface(application.clone(), observed_scope.clone());

    let start = sink
        .submit_tracked(UiCommand::StartCore)
        .expect("observed scope permits a tracked request");
    assert!(wait_for_sink_result(&sink, start).result.is_ok());
    assert_eq!(process.starts.load(Ordering::SeqCst), 1);

    let stale = sink
        .submit_tracked(UiCommand::StartCore)
        .expect("stale observed scope returns a terminal rejection");
    assert!(wait_for_sink_result(&sink, stale).result.is_err());
    assert_eq!(process.starts.load(Ordering::SeqCst), 1);

    *observed_scope.write().expect("scope observation lock") =
        CoreScopeObservation::Observed(application.snapshot().scope().expect("new core scope"));
    let stop = sink
        .submit_tracked(UiCommand::StopCore)
        .expect("rebound scope permits a tracked request");
    assert!(wait_for_sink_result(&sink, stop).result.is_ok());
    assert_eq!(process.stops.load(Ordering::SeqCst), 1);
}

#[test]
fn native_start_and_stop_execute_once_and_wait_for_the_real_snapshot_without_remounting() {
    let (mut app, process, source) = setup();
    let entity = button(&mut app);
    assert!(!app.world().get::<ButtonDisabled>(entity).unwrap().0);
    activate(&mut app, entity);
    activate(&mut app, entity);
    until(&mut app, |app| {
        matches!(
            &app.world().resource::<CoreControlState>().phase,
            CoreControlPhase::AwaitingSnapshot(_)
        )
    });
    assert_eq!(process.starts.load(Ordering::SeqCst), 1);
    assert!(app.world().get::<ButtonDisabled>(entity).unwrap().0);
    assert!(matches!(
        &app.world().resource::<CoreControlState>().phase,
        CoreControlPhase::AwaitingSnapshot(_)
    ));
    publish(&mut app, &source);
    assert_eq!(
        app.world().resource::<CoreControlState>().phase,
        CoreControlPhase::Idle
    );
    assert!(!app.world().get::<ButtonDisabled>(entity).unwrap().0);
    assert_eq!(button(&mut app), entity);
    activate(&mut app, entity);
    activate(&mut app, entity);
    until(&mut app, |app| {
        matches!(
            &app.world().resource::<CoreControlState>().phase,
            CoreControlPhase::AwaitingSnapshot(_)
        )
    });
    assert_eq!(process.stops.load(Ordering::SeqCst), 1);
    publish(&mut app, &source);
    assert_eq!(source.0.snapshot().lifecycle, CoreLifecycle::Stopped);
    assert_eq!(button(&mut app), entity);
    assert_eq!(
        app.world().resource::<CoreControlState>().phase,
        CoreControlPhase::Idle
    );
}

#[test]
fn a_permission_failure_retains_its_identity_and_retry_executes_the_same_native_control() {
    let (mut app, process, source) = setup();
    let entity = button(&mut app);
    process.deny_start.store(true, Ordering::SeqCst);
    activate(&mut app, entity);
    until(&mut app, |app| {
        matches!(
            &app.world().resource::<CoreControlState>().phase,
            CoreControlPhase::Failed(_)
        )
    });
    publish(&mut app, &source);
    let failure = app.world().resource::<CoreControlState>().phase.clone();
    let CoreControlPhase::Failed(failure) = failure else {
        panic!("permission failure remains visible until retry")
    };
    let expected = Failure::from(PortError::PermissionDenied(
        "grant process permission".into(),
    ));
    assert_eq!(failure, expected);
    let (issue, _, node) = app
        .world_mut()
        .query::<(Entity, &CoreControlIssue, &Node)>()
        .single(app.world())
        .unwrap();
    assert_eq!(node.display, Display::Flex);
    assert!(subtree_has_text(app.world(), issue, &failure.message));
    assert!(!app.world().get::<ButtonDisabled>(entity).unwrap().0);
    process.deny_start.store(false, Ordering::SeqCst);
    activate(&mut app, entity);
    until(&mut app, |app| {
        matches!(
            &app.world().resource::<CoreControlState>().phase,
            CoreControlPhase::Failed(_) | CoreControlPhase::AwaitingSnapshot(_)
        )
    });
    assert!(
        matches!(
            &app.world().resource::<CoreControlState>().phase,
            CoreControlPhase::AwaitingSnapshot(_)
        ),
        "retry should reach the current observed owner scope"
    );
    publish(&mut app, &source);
    assert_eq!(process.starts.load(Ordering::SeqCst), 2);
    assert_eq!(source.0.snapshot().lifecycle, CoreLifecycle::Running);
    assert_eq!(button(&mut app), entity);
    assert_eq!(
        app.world().resource::<CoreControlState>().phase,
        CoreControlPhase::Idle
    );
}

#[test]
fn stale_or_unrelated_terminal_feedback_cannot_unlock_a_pending_lifecycle_control() {
    let (mut app, _, _) = setup();
    let entity = button(&mut app);
    activate(&mut app, entity);
    let id = app.world().resource::<CoreControlState>().phase.clone();
    let CoreControlPhase::AwaitingResult(pending) = id else {
        panic!("the new request awaits its terminal result")
    };
    let id = pending.request_id;
    for (request_id, command) in [
        (RequestId(id.0 + 10), UiCommand::StartCore),
        (id, UiCommand::StopCore),
    ] {
        app.world_mut().commands().trigger(CommandExecutedEvent {
            request_id,
            command,
            result: Err(Failure::new(ErrorCode::Permission, "stale refusal", true)),
        });
        app.world_mut().flush();
        let state = app.world().resource::<CoreControlState>();
        assert!(matches!(
            &state.phase,
            CoreControlPhase::AwaitingResult(pending) if pending.request_id == id
        ));
    }
}
