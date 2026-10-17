//! test-intent: behavior
//! Native SDK gestures and ordinary keyboard input execute real Core exports in isolated host files.
use crate::command_harness::install_test_core_capability_application;
use crate::native_input::{click_entity, press, type_text};
use crate::support::{
    command_pump_for_observed_snapshot, execute_at_observed_scope, headless_plugins,
};
use async_trait::async_trait;
use bevy::app::{App, PreUpdate};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{With, Without};
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input_focus::{InputFocus, dispatch_focused_input};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_application::LogIngestOutcome;
use infiltrator_application::log_export_application::LogExportApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::pages::logs::{ClearLogsButton, ExportLogsButton};
use infiltrator_bevy_ui::pages::logs_export::{LogsExportAction, LogsExportLine, LogsExportState};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::SurfaceSnapshotUpdated;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_composition::capture_fixtures::{append_follow_records, populate_logs};
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_capabilities::{CoreLifecycleOperation, CoreOperation};
use infiltrator_contract::log_export::{LogExportArtifact, LogExportReceipt};
use infiltrator_contract::logs::LogSession;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::log_export::LogExportPort;
use infiltrator_ports::runtime_stream::RuntimeStreamEvent;
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::log_capture_fixtures::LogCaptureProcess;
use lurker_core_contract::error::ErrorCode;
use lurker_core_ports::error::PortError;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::sleep;
use std::time::{Duration, Instant};
use tempfile::tempdir;
use tokio::runtime::Builder;

#[derive(Clone)]
struct TestLogExportPort {
    directory: PathBuf,
}

impl TestLogExportPort {
    fn new(directory: &Path) -> Self {
        Self {
            directory: directory.join("exports"),
        }
    }
}

#[async_trait]
impl LogExportPort for TestLogExportPort {
    async fn save(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        fs::create_dir_all(&self.directory).map_err(|error| PortError::Io(error.to_string()))?;
        let path = self.directory.join("logs-test.log");
        fs::write(&path, &artifact.content).map_err(|error| PortError::Io(error.to_string()))?;
        Ok(LogExportReceipt {
            summary: artifact.summary,
            path: path.to_string_lossy().into_owned(),
            bytes_written: artifact.content.len(),
        })
    }
}

#[test]
fn native_missing_host_is_explicit_unsupported_and_cancel_remains_effect_free() {
    let directory = tempdir().unwrap();
    let (mut app, core) = setup(Arc::new(TestLogExportPort::new(directory.path())));
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_logs(core.log_application()),
    ));
    open(&mut app);
    click(&mut app, LogsExportAction::Confirm);
    terminal(&mut app);
    let model = &app.world().resource::<LogsExportState>().model;
    assert_eq!(model.failure().unwrap().code, ErrorCode::Unsupported);
    assert!(!model.can_retry() && model.receipt().is_none());
    assert!(!directory.path().join("exports").exists());
    click(&mut app, LogsExportAction::Cancel);
    terminal(&mut app);
    assert!(!app.world().resource::<LogsExportState>().model.is_open());
    assert!(!directory.path().join("exports").exists());
    run(core.close()).unwrap();
}
fn run<T>(future: impl Future<Output = T>) -> T {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
fn setup(port: Arc<dyn LogExportPort>) -> (App, Arc<CoreApplication>) {
    let process = Arc::new(LogCaptureProcess::default());
    let core = Arc::new(CoreApplication::new(
        process.clone(),
        process,
        tokio_application_runtime().unwrap(),
    ));
    install_test_core_capability_application(
        &core,
        [
            CoreOperation::Lifecycle(CoreLifecycleOperation::Start),
            CoreOperation::Lifecycle(CoreLifecycleOperation::Restart),
        ],
    );
    core.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_logs(core.log_application())
            .with_log_export(LogExportApplication::new(
                core.log_application(),
                Some(port),
                vec![],
            )),
    ));
    run(populate_logs(&core)).unwrap();
    let scope = core.snapshot();
    assert_eq!(
        core.log_application().ingest(
            LogSession::from_core_scope(scope.scope().expect("core scope")).expect("log session"),
            RuntimeStreamEvent::Item(
                "INFO[4] https://sub.example/?token=private-subscription-token".into()
            )
        ),
        LogIngestOutcome::Applied
    );
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        command_pump_for_observed_snapshot(core.clone()),
    ));
    app.add_systems(PreUpdate, dispatch_focused_input::<KeyboardInput>);
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Logs));
    app.update();
    let reader =
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop);
    let snapshot = run(reader.read()).unwrap();
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    (app, core)
}
fn launcher(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<ExportLogsButton>>()
        .single(app.world())
        .unwrap()
}
fn action(app: &mut App, target: LogsExportAction) -> Entity {
    app.world_mut()
        .query_filtered::<(Entity, &LogsExportAction), Without<ModalScrim>>()
        .iter(app.world())
        .find(|(_, value)| {
            matches!(
                (value, target),
                (LogsExportAction::Confirm, LogsExportAction::Confirm)
                    | (LogsExportAction::Cancel, LogsExportAction::Cancel)
                    | (LogsExportAction::Retry, LogsExportAction::Retry)
            )
        })
        .unwrap()
        .0
}
fn click(app: &mut App, target: LogsExportAction) {
    let entity = action(app, target);
    click_entity(app, entity);
}
fn terminal(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<LogsExportState>()
        .model
        .pending()
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "actual Core terminal did not arrive"
        );
        app.update();
        sleep(Duration::from_millis(1));
    }
    app.update();
}
fn open(app: &mut App) {
    let entity = launcher(app);
    click_entity(app, entity);
    terminal(app);
}
#[test]
fn native_modal_cancel_and_confirm_freeze_all_records_and_show_only_an_actual_redacted_file_receipt()
 {
    let directory = tempdir().unwrap();
    let (mut app, core) = setup(Arc::new(TestLogExportPort::new(directory.path())));
    let field = app
        .world_mut()
        .query::<(Entity, &NativeTextField)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == 11)
        .unwrap()
        .0;
    press(&mut app, field);
    type_text(&mut app, "api");
    let draft = app
        .world()
        .get::<TextField>(field)
        .unwrap()
        .0
        .text()
        .to_owned();
    assert_eq!(draft, "api");
    open(&mut app);
    assert_eq!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .summary()
            .as_ref()
            .unwrap()
            .records,
        4
    );
    assert!(app.world().get::<TextField>(field).unwrap().0.is_disabled());
    assert!(!app.world().get::<TextFieldFocused>(field).unwrap().0);
    type_text(&mut app, "blocked");
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), draft);
    assert!(!directory.path().join("exports").exists());
    // Ordinary keyboard input is dispatched by the same SDK focus system as the product.
    let cancel = action(&mut app, LogsExportAction::Cancel);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(cancel));
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: Key::Character("Enter".into()),
        text: None,
        state: ButtonState::Pressed,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
    terminal(&mut app);
    assert!(!app.world().resource::<LogsExportState>().model.is_open());
    assert!(!directory.path().join("exports").exists());
    assert!(!app.world().get::<TextField>(field).unwrap().0.is_disabled());
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    open(&mut app);
    append_follow_records(&core).unwrap();
    click(&mut app, LogsExportAction::Confirm);
    terminal(&mut app);
    let receipt = app
        .world()
        .resource::<LogsExportState>()
        .model
        .receipt()
        .cloned()
        .unwrap();
    assert!(Path::new(&receipt.path).starts_with(directory.path().join("exports")));
    let bytes = fs::read_to_string(&receipt.path).unwrap();
    assert_eq!(bytes.lines().count(), 4);
    assert_eq!(receipt.bytes_written, bytes.len());
    assert!(!bytes.contains("private-subscription-token") && bytes.contains("token=***"));
    assert!(!bytes.contains("continued controller record"));
    let visible_path = {
        let world = app.world_mut();
        let mut query = world.query::<(&LogsExportLine, &Text)>();
        query
            .iter(world)
            .find(|(marker, _)| matches!(marker, LogsExportLine::Path))
            .unwrap()
            .1
            .0
            .clone()
    };
    assert_eq!(visible_path, receipt.path);
    click(&mut app, LogsExportAction::Cancel);
    terminal(&mut app);
    assert!(!app.world().resource::<LogsExportState>().model.is_open());
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        1
    );
    run(core.close()).unwrap();
}
struct ControlledWriter {
    inner: TestLogExportPort,
    denied: AtomicBool,
    pause: AtomicBool,
    entered: Sender<()>,
    release: Mutex<Receiver<()>>,
}
#[async_trait]
impl LogExportPort for ControlledWriter {
    async fn save(&self, artifact: LogExportArtifact) -> Result<LogExportReceipt, PortError> {
        if self.denied.load(Ordering::Acquire) {
            return Err(PortError::PermissionDenied("host export denied".into()));
        }
        if self.pause.load(Ordering::Acquire) {
            self.entered.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
        self.inner.save(artifact).await
    }
}
#[test]
fn native_failure_retry_pending_cancel_and_test_port_effects_remain_correlated() {
    let directory = tempdir().unwrap();
    let (entered, arrival) = channel();
    let (release, permit) = channel();
    let writer = Arc::new(ControlledWriter {
        inner: TestLogExportPort::new(directory.path()),
        denied: AtomicBool::new(true),
        pause: AtomicBool::new(false),
        entered,
        release: Mutex::new(permit),
    });
    let (mut app, core) = setup(writer.clone());
    open(&mut app);
    let summary = app
        .world()
        .resource::<LogsExportState>()
        .model
        .summary()
        .cloned()
        .unwrap();
    click(&mut app, LogsExportAction::Confirm);
    terminal(&mut app);
    assert_eq!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .failure()
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .receipt()
            .is_none()
    );
    assert!(!directory.path().join("exports").exists());
    writer.denied.store(false, Ordering::Release);
    writer.pause.store(true, Ordering::Release);
    click(&mut app, LogsExportAction::Retry);
    arrival.recv_timeout(Duration::from_secs(5)).unwrap();
    let cancel = action(&mut app, LogsExportAction::Cancel);
    assert!(app.world().get::<ButtonDisabled>(cancel).unwrap().0);
    click_entity(&mut app, cancel);
    assert!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .pending()
            .is_some()
    );
    let clear = app
        .world_mut()
        .query_filtered::<Entity, With<ClearLogsButton>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, clear);
    assert_eq!(core.log_application().export_records().unwrap().1.len(), 4);
    release.send(()).unwrap();
    terminal(&mut app);
    assert_eq!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .receipt()
            .as_ref()
            .unwrap()
            .summary,
        summary.clone()
    );
    writer.pause.store(false, Ordering::Release);
    click(&mut app, LogsExportAction::Cancel);
    terminal(&mut app);
    open(&mut app);
    run(execute_at_observed_scope(&core, CommandIntent::RestartCore))
        .into_unit()
        .unwrap();
    let reader =
        ProductSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop);
    let snapshot = run(reader.read()).unwrap();
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    click(&mut app, LogsExportAction::Confirm);
    terminal(&mut app);
    assert_eq!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .failure()
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidState
    );
    assert!(!app.world().resource::<LogsExportState>().model.can_retry());
    assert!(
        app.world()
            .resource::<LogsExportState>()
            .model
            .receipt()
            .is_none()
    );
    assert_eq!(
        fs::read_dir(directory.path().join("exports"))
            .unwrap()
            .count(),
        1
    );
    click(&mut app, LogsExportAction::Cancel);
    terminal(&mut app);
    assert!(!app.world().resource::<LogsExportState>().model.is_open());
    run(core.close()).unwrap();
}
