//! Source-bound native export confirmation and exact Core terminal correlation.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::logs::ExportLogsButton;
use crate::pages::logs_export_focus::LogExportFocus;
use crate::pages::logs_export_render::render;
use crate::pages::logs_export_scene::spawn;
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, Plugin, PostUpdate, Startup, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::InputFocus;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::log_export_actions::{LogExportActions, LogExportPending};
use infiltrator_bevy_widgets::button::{ButtonDisabled, sync_button_disabled};
use infiltrator_contract::command::{CommandIntent, RequestId};
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Resource, Default)]
pub struct LogsExportState {
    pub model: LogExportActions,
    pub request: Option<(RequestId, u64)>,
    pub held_fields: Vec<(Entity, bool, bool)>,
    pub holding_focus: bool,
    pub previous_sdk_focus: Option<Entity>,
}
#[derive(Component, Clone, Copy, Default)]
pub struct LogsExportRoot;
#[derive(Component, Clone, Copy, Default)]
pub struct LogsExportCard;
#[derive(Component, Clone, Copy, Default)]
#[require(Button, ButtonDisabled)]
pub enum LogsExportAction {
    #[default]
    Confirm,
    Cancel,
    Retry,
}
#[derive(Component, Clone, Copy, Default)]
pub enum LogsExportLine {
    #[default]
    Status,
    Details,
    Path,
}
pub struct LogsExportPlugin;
impl Plugin for LogsExportPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LogsExportState>()
            .init_resource::<InputFocus>()
            .add_systems(Startup, spawn)
            .add_observer(activate)
            .add_observer(finish)
            .add_systems(Update, observe)
            .add_systems(PostUpdate, render.before(sync_button_disabled));
    }
}
fn submit(
    state: &mut LogsExportState,
    request: LogExportPending,
    sink: Option<&CommandSinkHandle>,
) {
    let operation = request.operation;
    let command = match request.intent {
        CommandIntent::PrepareLogExport => UiCommand::PrepareLogExport { operation },
        CommandIntent::SaveLogExport { identity } => UiCommand::SaveLogExport {
            operation,
            identity,
        },
        CommandIntent::CancelLogExport { identity } => UiCommand::CancelLogExport {
            operation,
            identity,
        },
        _ => unreachable!("shared log export request"),
    };
    if let Some(request_id) = sink.and_then(|sink| sink.submit_tracked(command)) {
        state.request = Some((request_id, operation));
    } else {
        state.model.finish(
            operation,
            Err(Failure::new(
                ErrorCode::NotReady,
                "Log export command service has no terminal response",
                true,
            )),
        );
    }
}
fn activate(
    event: On<Activate>,
    launchers: Query<(), With<ExportLogsButton>>,
    actions: Query<&LogsExportAction>,
    route: Res<ActiveRoute>,
    mut state: ResMut<LogsExportState>,
    sink: Option<Res<CommandSinkHandle>>,
    mut focus: LogExportFocus,
) {
    let request = if launchers.contains(event.entity) {
        if route.0 != Some(Route::Logs) {
            return;
        }
        state.model.show()
    } else {
        match actions.get(event.entity) {
            Ok(LogsExportAction::Confirm) => state.model.confirm(),
            Ok(LogsExportAction::Cancel) => state.model.cancel(),
            Ok(LogsExportAction::Retry) => state.model.retry(),
            Err(_) => return,
        }
    };
    focus.sync(&mut state);
    if let Some(request) = request {
        submit(&mut state, request, sink.as_deref());
    }
}
fn finish(
    event: On<CommandExecutedEvent>,
    mut state: ResMut<LogsExportState>,
    mut focus: LogExportFocus,
) {
    let Some((request, operation)) = state.request else {
        return;
    };
    let event_operation = match event.command {
        UiCommand::PrepareLogExport { operation }
        | UiCommand::SaveLogExport { operation, .. }
        | UiCommand::CancelLogExport { operation, .. } => operation,
        _ => return,
    };
    if event.request_id == request
        && event_operation == operation
        && event.command.to_intent()
            == state
                .model
                .pending
                .as_ref()
                .map(|pending| pending.intent.clone())
        && state.model.finish(operation, event.result.clone())
    {
        state.request = None;
        focus.sync(&mut state);
    }
}
fn observe(
    route: Res<ActiveRoute>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    mut state: ResMut<LogsExportState>,
    sink: Option<Res<CommandSinkHandle>>,
    mut focus: LogExportFocus,
) {
    if state.model.open
        && (route.0 != Some(Route::Logs)
            || keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)))
        && let Some(request) = state.model.cancel()
    {
        submit(&mut state, request, sink.as_deref());
    }
    focus.sync(&mut state);
}
