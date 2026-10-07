//! test-intent: behavior
//! Native clear/filter controls execute the real application commands and publish its actual buffer.
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::command_application::{
    CommandApplication, CommandFuture, CommandHandler,
};
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::log_capture_fixtures::{LogCaptureProcess, populate_logs};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::logs::{ClearLogsButton, LogLevelFilterButton};
use infiltrator_bevy_ui::pages::logs_search::LogsViewState;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::SurfaceSnapshotUpdated;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::Failure;
use infiltrator_contract::logs::LogLevel;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;
use tokio::runtime::Builder;

struct ObservedCommands {
    commands: CommandApplication,
    completed: Sender<(CommandIntent, Result<(), Failure>)>,
}
impl CommandHandler for ObservedCommands {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        let future = self.commands.handle(intent.clone());
        let completed = self.completed.clone();
        Box::pin(async move {
            let result = future.await;
            completed.send((intent, result.clone())).unwrap();
            result
        })
    }
}
#[test]
fn native_logs_filter_and_clear_mutate_the_shared_reader_owner_after_real_completion() {
    let runtime = tokio_application_runtime().unwrap();
    let process = Arc::new(LogCaptureProcess::default());
    let core = Arc::new(CoreApplication::new(
        process.clone(),
        process,
        runtime.clone(),
    ));
    let (completed, observed) = channel();
    core.install_command_handler(Arc::new(ObservedCommands {
        commands: CommandApplication::new().with_logs(core.log_application()),
        completed,
    }));
    let producer = core.clone();
    runtime.block_on(Box::pin(async move {
        populate_logs(&producer).await.unwrap();
    }));
    let reader =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        CommandPumpPlugin::for_application(core.clone()),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Logs));
    app.update();
    let publish = |app: &mut App| {
        let snapshot = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(reader.read())
            .unwrap();
        app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
        app.update();
    };
    publish(&mut app);
    assert_eq!(app.world().resource::<LogsViewState>().0.source_count(), 3);
    let warn = app
        .world_mut()
        .query::<(Entity, &LogLevelFilterButton)>()
        .iter(app.world())
        .find(|(_, marker)| marker.level == Some(LogLevel::Warn))
        .unwrap()
        .0;
    click_entity(&mut app, warn);
    let (intent, result) = observed
        .recv_timeout(Duration::from_secs(5))
        .expect("actual filter completion");
    assert_eq!(
        intent,
        CommandIntent::SetLogLevelFilter {
            level: Some("WARN".into())
        }
    );
    result.unwrap();
    publish(&mut app);
    assert_eq!(app.world().resource::<LogsViewState>().0.matched_count(), 1);
    let clear = app
        .world_mut()
        .query_filtered::<Entity, With<ClearLogsButton>>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, clear);
    let (intent, result) = observed
        .recv_timeout(Duration::from_secs(5))
        .expect("actual clear completion");
    assert_eq!(intent, CommandIntent::ClearLogs);
    result.unwrap();
    publish(&mut app);
    assert_eq!(
        app.world().resource::<LogsViewState>().0.status_key(),
        "logs_no_realtime_records"
    );
    assert_eq!(app.world().resource::<LogsViewState>().0.source_count(), 0);
    let closing = core.clone();
    runtime.block_on(Box::pin(async move {
        closing.close().await.unwrap();
    }));
}
