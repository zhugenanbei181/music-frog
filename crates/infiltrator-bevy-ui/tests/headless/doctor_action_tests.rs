//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::doctor_port::DiagnosticPort;
use crate::support::{headless_plugins, subtree_has_text};
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::{component::Component, entity::Entity, hierarchy::Children};
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::doctor_application::DoctorApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::doctor::{
    DoctorProjectionUpdated, RepairDoctorRowButton, RunDoctorDiagnosticsButton,
};
use infiltrator_bevy_ui::pages::doctor_actions::{
    BootstrapDoctorButton, DoctorActions, RetryDoctorButton,
};
use infiltrator_bevy_ui::pages::doctor_rows::{DoctorRow, DoctorRows};
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{SurfaceSnapshotUpdated, doctor_projection};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
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
    while app
        .world()
        .resource::<DoctorActions>()
        .state
        .pending
        .is_some()
    {
        assert!(Instant::now() < deadline, "actual command terminal result");
        app.update();
        yield_now();
    }
    app.update();
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
        CommandApplication::new().with_doctor(doctor.clone()),
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
    app.add_plugins(CommandPumpPlugin::for_application(application));
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
    let token = app
        .world()
        .resource::<DoctorActions>()
        .state
        .pending
        .as_ref()
        .unwrap()
        .token;
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::RunDoctorDiagnostics,
        request_id: RequestId(u64::MAX),
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().flush();
    assert_eq!(
        app.world()
            .resource::<DoctorActions>()
            .state
            .pending
            .as_ref()
            .unwrap()
            .token,
        token
    );
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<DoctorActions>()
            .state
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
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
