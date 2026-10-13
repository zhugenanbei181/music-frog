//! test-intent: behavior
use crate::command_harness::{TestCoreLifecycleHostCapabilityProvider, recording_application};
use crate::doctor_port::DiagnosticPort;
use crate::support::{command_pump_for_observed_snapshot, headless_plugins, subtree_has_text};
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::{component::Component, entity::Entity, hierarchy::Children};
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::doctor_actions::DoctorActionPhase;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::doctor::self_heal::ApplyBestNodeButton;
use infiltrator_bevy_ui::pages::doctor::{
    DoctorPageRoot, DoctorProjectionUpdated, RepairDoctorRowButton, RunDoctorDiagnosticsButton,
};
use infiltrator_bevy_ui::pages::doctor_actions::{
    BootstrapDoctorButton, DoctorActions, RetryDoctorButton,
};
use infiltrator_bevy_ui::pages::doctor_rows::{DoctorRow, DoctorRows};
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{
    LatestSurfaceSnapshot, SurfaceSnapshotUpdated, doctor_projection,
};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::outbound::{
    details::OutboundSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
    probe::{OutboundProbeBatchScope, OutboundSelectionRequest},
};
use infiltrator_contract::speedtest::{
    SpeedtestOutboundResult, SpeedtestProgress, SpeedtestResultState, SpeedtestRounds,
    SpeedtestRunId, SpeedtestRunState, SpeedtestSampleOutcome, SpeedtestTarget,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_ports::surface::SurfaceReader;
use std::any::type_name;
use std::sync::{Arc, atomic::Ordering};
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .unwrap_or_else(|| panic!("mounted native control {}", type_name::<T>()))
        .0
}
fn click(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while matches!(
        &app.world().resource::<DoctorActions>().state.phase,
        DoctorActionPhase::Pending(_)
    ) {
        assert!(Instant::now() < deadline, "actual command terminal result");
        app.update();
        yield_now();
    }
    app.update();
}
fn pending_token(app: &App) -> u64 {
    match &app.world().resource::<DoctorActions>().state.phase {
        DoctorActionPhase::Pending(pending) => pending.token,
        DoctorActionPhase::Idle | DoctorActionPhase::Failed { .. } => {
            panic!("doctor action must have a correlated pending token")
        }
    }
}
fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}
#[test]
fn native_doctor_controls_execute_real_failure_retry_repair_bootstrap_and_reject_stale_command_results()
 {
    let port = Arc::new(DiagnosticPort::default());
    let doctor = DoctorApplication::new(port.clone());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_host_capabilities(Arc::new(TestCoreLifecycleHostCapabilityProvider))
            .with_doctor(doctor.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_doctor(doctor);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::default());
    app.add_plugins(command_pump_for_observed_snapshot(application));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Doctor));
    app.update();
    publish(&mut app, &reader);
    let run = entity::<RunDoctorDiagnosticsButton>(&mut app);
    click(&mut app, run);
    settle(&mut app);
    publish(&mut app, &reader);
    let repair = entity::<RepairDoctorRowButton>(&mut app);
    assert_eq!(
        app.world()
            .get::<RepairDoctorRowButton>(repair)
            .unwrap()
            .check_id,
        "config.integrity"
    );
    assert!(!app.world().get::<ButtonDisabled>(repair).unwrap().0);
    port.deny.store(true, Ordering::SeqCst);
    app.world_mut().commands().trigger(Activate { entity: run });
    app.world_mut().flush();
    let token = pending_token(&app);
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::RunDoctorDiagnostics,
        request_id: RequestId(u64::MAX),
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().flush();
    assert_eq!(pending_token(&app), token);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(matches!(
        &app.world().resource::<DoctorActions>().state.phase,
        DoctorActionPhase::Failed { failure, .. }
            if failure.code == ErrorCode::Permission
    ));
    assert!(app.world().get::<ButtonDisabled>(repair).unwrap().0);
    app.update();
    assert!(app.world().get::<InteractionDisabled>(repair).is_some());
    assert!(
        app.world()
            .get::<AccessibilityNode>(repair)
            .unwrap()
            .is_disabled()
    );
    let root = entity::<PageRoot>(&mut app);
    assert!(subtree_has_text(
        app.world(),
        root,
        "allow diagnostic access"
    ));
    click(&mut app, repair);
    assert_eq!(port.fixes.load(Ordering::SeqCst), 0);
    port.deny.store(false, Ordering::SeqCst);
    let retry = entity::<RetryDoctorButton>(&mut app);
    click(&mut app, retry);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(
        app.world().get_entity(repair).is_ok(),
        "identity survives failure and recovery"
    );
    assert!(app.world().get::<InteractionDisabled>(repair).is_none());
    assert!(
        !app.world()
            .get::<AccessibilityNode>(repair)
            .unwrap()
            .is_disabled()
    );
    click(&mut app, repair);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(port.fixes.load(Ordering::SeqCst), 1);
    assert!(app.world().get::<ButtonDisabled>(repair).unwrap().0);
    app.update();
    assert!(app.world().get::<InteractionDisabled>(repair).is_some());
    assert!(
        app.world()
            .get::<AccessibilityNode>(repair)
            .unwrap()
            .is_disabled()
    );
    let bootstrap = entity::<BootstrapDoctorButton>(&mut app);
    click(&mut app, bootstrap);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(port.bootstraps.load(Ordering::SeqCst), 1);
    assert_eq!(port.runs.load(Ordering::SeqCst), 5);
    let mut projection = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .map(|snapshot| doctor_projection(&snapshot))
        .unwrap();
    let original = projection.checks[0].clone();
    let mut added = original.clone();
    added.id = "new.identity".into();
    added.name = "new observed name".into();
    projection.checks = vec![added, original.clone()];
    app.world_mut()
        .commands()
        .trigger(DoctorProjectionUpdated(projection.clone()));
    app.update();
    app.update();
    let container = entity::<DoctorRows>(&mut app);
    let children = app.world().get::<Children>(container).unwrap();
    assert_eq!(
        app.world().get::<DoctorRow>(children[0]).unwrap().0,
        "new.identity"
    );
    assert_eq!(
        app.world().get::<DoctorRow>(children[1]).unwrap().0,
        "config.integrity"
    );
    assert!(app.world().get_entity(repair).is_ok());
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    assert!(subtree_has_text(app.world(), root, "Diagnostic results"));
    assert!(subtree_has_text(app.world(), root, "finding {count}"));
    projection.checks.clear();
    app.world_mut()
        .commands()
        .trigger(DoctorProjectionUpdated(projection.clone()));
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&DoctorRow>()
            .iter(app.world())
            .count(),
        0
    );
    projection.checks = vec![original];
    app.world_mut()
        .commands()
        .trigger(DoctorProjectionUpdated(projection));
    app.update();
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&DoctorRow>()
            .iter(app.world())
            .count(),
        1
    );
    assert!(
        app.world().get_entity(repair).is_err(),
        "removed identity is not revived as a stale control"
    );
}

fn mount_doctor_with_sink() -> (App, Arc<DemoCommandSink>) {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((ShellPlugin::default(), PagesPlugin::demo()));
    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(
        sink.clone() as Arc<dyn UiCommandSink>
    ));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Doctor));
    app.update();
    (app, sink)
}

fn publish_outbounds(app: &mut App, outbounds: &[(&str, Option<Vec<SpeedtestSampleOutcome>>)]) {
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.core.revision = snapshot.revision;
    let scope = snapshot
        .core
        .scope()
        .expect("demo surface exposes its accepted core scope");
    let engine = scope.identity.engine.clone();
    let id = |name: &str| OutboundId::new(engine.clone(), name).expect("test outbound ID is valid");
    let entries = outbounds
        .iter()
        .map(|(name, _)| OutboundSnapshot {
            id: id(name),
            display_name: Some((*name).to_owned()),
        })
        .collect();
    let member_ids = outbounds.iter().map(|(name, _)| id(name)).collect();
    snapshot.outbound_inventory = PageData::ready(
        OutboundInventorySnapshot::new(
            scope.clone(),
            SnapshotRevision::new(13),
            OutboundInventoryState::Ready {
                entries,
                collections: vec![OutboundSelectionSet {
                    id: id("GLOBAL"),
                    member_ids,
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Unknown,
                }],
            },
        )
        .expect("test outbound inventory is valid"),
    );
    snapshot.speedtest.state = SpeedtestRunState::Completed {
        run_id: SpeedtestRunId::new(1).expect("test run ID is valid"),
        target: SpeedtestTarget::new(
            scope,
            SnapshotRevision::new(13),
            OutboundProbeBatchScope::All,
            SpeedtestRounds::DEFAULT,
        )
        .expect("test speedtest target is valid"),
        progress: SpeedtestProgress {
            completed_targets: outbounds
                .iter()
                .filter(|(_, samples)| samples.is_some())
                .count(),
            total_targets: outbounds.len(),
        },
    };
    for (name, samples) in outbounds {
        let Some(samples) = samples else {
            continue;
        };
        let state = if samples
            .iter()
            .any(|sample| matches!(sample, SpeedtestSampleOutcome::Measured { .. }))
        {
            SpeedtestResultState::Completed
        } else {
            SpeedtestResultState::TimedOut
        };
        snapshot.speedtest.results.push(SpeedtestOutboundResult {
            outbound_id: id(name),
            display_name: Some((*name).to_owned()),
            protocol_display_name: Some("test".to_owned()),
            samples: samples.clone(),
            state,
            tested_at_epoch_ms: 1,
        });
    }
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}

#[test]
fn native_doctor_self_heal_surfaces_best_node_typed_unknown_and_applies_through_the_sink() {
    let (mut app, sink) = mount_doctor_with_sink();
    publish_outbounds(
        &mut app,
        &[
            (
                "HK-Fast",
                Some(vec![SpeedtestSampleOutcome::Measured { delay_ms: 40 }]),
            ),
            (
                "US-Lossy",
                Some(vec![
                    SpeedtestSampleOutcome::Measured { delay_ms: 120 },
                    SpeedtestSampleOutcome::TimedOut,
                    SpeedtestSampleOutcome::TimedOut,
                ]),
            ),
            ("JP-Unknown", None),
        ],
    );

    let root = entity::<DoctorPageRoot>(&mut app);
    // The scored recommendation and both observed anomaly signals surface.
    assert!(subtree_has_text(app.world(), root, "HK-Fast"));
    assert!(subtree_has_text(app.world(), root, "US-Lossy"));
    // An unobserved node is a typed unknown, never a fabricated score.
    assert!(subtree_has_text(app.world(), root, "JP-Unknown"));
    assert!(subtree_has_text(app.world(), root, "未观测"));

    let apply = entity::<ApplyBestNodeButton>(&mut app);
    assert!(!app.world().get::<ButtonDisabled>(apply).unwrap().0);
    click(&mut app, apply);
    assert_eq!(
        sink.submitted(),
        vec![UiCommand::SelectOutbound {
            request: OutboundSelectionRequest::new(
                OutboundId::new(EngineId::new("mihomo").unwrap(), "GLOBAL").unwrap(),
                OutboundId::new(EngineId::new("mihomo").unwrap(), "HK-Fast").unwrap(),
                SnapshotRevision::new(13),
            )
            .unwrap(),
        }],
        "apply routes the recommendation through the shared command sink"
    );
}

#[test]
fn native_doctor_self_heal_without_scored_facts_is_unknown_and_not_actionable() {
    let (mut app, sink) = mount_doctor_with_sink();
    publish_outbounds(&mut app, &[("JP-Unknown", None)]);

    let root = entity::<DoctorPageRoot>(&mut app);
    assert!(subtree_has_text(app.world(), root, "JP-Unknown"));
    assert!(subtree_has_text(app.world(), root, "未观测"));

    let apply = entity::<ApplyBestNodeButton>(&mut app);
    assert!(app.world().get::<ButtonDisabled>(apply).unwrap().0);
    click(&mut app, apply);
    assert!(sink.submitted().is_empty());
}
