//! Native script actions consume SDK multiline edits before freezing a command.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::profiles::ProfilesProjectionUpdated;
use crate::pages::profiles_script_scene::spawn_review;
use crate::pages::profiles_script_view;
use crate::route::{ActiveRoute, Route};
use bevy::app::{App, Plugin, PostUpdate, Startup};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EntityEvent;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::text::{EditableText, EditableTextSystems, TextEditChange};
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::script_workbench::{ScriptPending, ScriptWorkbench};
use infiltrator_bevy_widgets::button::{
    ButtonDisabled, ButtonInteractionState, sync_button_disabled, sync_control_labels,
    sync_control_visuals,
};
use infiltrator_bevy_widgets::interaction_block::sync_inputs;
use infiltrator_bevy_widgets::multiline_editor::{MultilineEditorPlugin, MultilineModeSet};
use infiltrator_contract::command::{CommandIntent, RequestId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::script_export::ScriptExportKind;
use infiltrator_contract::script_export_review::ScriptExportDraft;
use infiltrator_contract::script_run::ScriptEditorField;
use std::mem::take;

#[derive(Resource, Default)]
pub struct ScriptWorkbenchState {
    pub model: ScriptWorkbench,
    pub request: Option<(RequestId, u64)>,
    pub queue: Vec<(Entity, ScriptControl)>,
    pub previous_focus: Option<Entity>,
    pub holding_focus: bool,
    pub blocked: Vec<Entity>,
    pub last_editor_focus: Option<Entity>,
    pub rows: Vec<String>,
}
#[derive(Component, Clone, Copy)]
pub struct ScriptInput(pub ScriptEditorField);
#[derive(Component, Clone, Copy)]
#[require(Button, ButtonDisabled)]
pub enum ScriptControl {
    Run,
    Clear,
    Preset(&'static str),
    Export(ScriptExportKind),
    Confirm,
    Cancel,
    Retry,
}
pub struct ScriptWorkbenchPlugin;
impl Plugin for ScriptWorkbenchPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MultilineEditorPlugin>() {
            app.add_plugins(MultilineEditorPlugin);
        }
        app.init_resource::<ScriptWorkbenchState>()
            .add_observer(initialize)
            .add_observer(enqueue)
            .add_observer(completed)
            .add_observer(observe)
            .add_observer(edited)
            .add_systems(Startup, spawn_review)
            .add_systems(
                PostUpdate,
                (
                    process,
                    ApplyDeferred,
                    profiles_script_view::render,
                    ApplyDeferred,
                    (|commands: Commands, buttons: Query<ButtonInteractionState>| {
                        sync_button_disabled(commands, buttons);
                    }),
                    sync_inputs,
                    ApplyDeferred,
                    sync_control_visuals,
                    sync_control_labels,
                )
                    .chain()
                    .after(EditableTextSystems)
                    .before(MultilineModeSet),
            );
    }
}
fn initialize(
    event: On<Insert<ScriptInput>>,
    state: Res<ScriptWorkbenchState>,
    mut fields: Query<(&ScriptInput, &mut EditableText)>,
) {
    let Ok((field, mut editable)) = fields.get_mut(event.entity) else {
        return;
    };
    let value = match field.0 {
        ScriptEditorField::Code => &state.model.script_code,
        ScriptEditorField::InputYaml => &state.model.input_yaml,
    };
    editable.editor_mut().set_text(value);
}
fn enqueue(
    event: On<Activate>,
    controls: Query<&ScriptControl>,
    mut state: ResMut<ScriptWorkbenchState>,
) {
    if let Ok(control) = controls.get(event.entity) {
        state.queue.push((event.entity, *control));
    }
}
fn edited(
    event: On<TextEditChange>,
    fields: Query<(&ScriptInput, &EditableText)>,
    mut state: ResMut<ScriptWorkbenchState>,
) {
    let Ok((field, value)) = fields.get(event.event_target()) else {
        return;
    };
    if value.is_composing() {
        return;
    }
    let text = value.value().to_string();
    match field.0 {
        ScriptEditorField::Code if state.model.script_code != text => state.model.edit_script(text),
        ScriptEditorField::InputYaml if state.model.input_yaml != text => {
            state.model.edit_yaml(text)
        }
        _ => {}
    }
}
fn observe(event: On<ProfilesProjectionUpdated>, mut state: ResMut<ScriptWorkbenchState>) {
    if state.model.has_local_actions() {
        return;
    }
    state.model.snapshot = event.0.script_sandbox.clone();
    state.model.export = event.0.script_export.clone();
}
#[derive(SystemParam)]
pub struct ScriptInteraction<'w, 's> {
    state: ResMut<'w, ScriptWorkbenchState>,
    route: Res<'w, ActiveRoute>,
    handle: Option<Res<'w, CommandSinkHandle>>,
    controls: Query<'w, 's, &'static ScriptControl>,
    fields: Query<'w, 's, (&'static ScriptInput, &'static mut EditableText)>,
    keyboard: Option<Res<'w, ButtonInput<KeyCode>>>,
}
fn process(mut surface: ScriptInteraction) {
    let queued = take(&mut surface.state.queue);
    let composing = surface.fields.iter().any(|(_, field)| field.is_composing());
    let leaving = surface.route.0 != Some(Route::Profiles);
    let escape = surface
        .keyboard
        .as_deref()
        .is_some_and(|keys| keys.just_pressed(KeyCode::Escape));
    if surface.state.model.export_visible
        && !surface.state.model.busy()
        && (leaving || (!composing && escape))
        && let Some(request) = surface.state.model.cancel_export()
    {
        submit(&mut surface.state, surface.handle.as_deref(), request);
    }
    if leaving {
        return;
    }
    for (entity, control) in queued {
        if !surface.controls.contains(entity) {
            continue;
        }
        if surface.state.model.busy() {
            continue;
        }
        if surface.state.model.export_visible
            && !matches!(
                control,
                ScriptControl::Confirm | ScriptControl::Cancel | ScriptControl::Retry
            )
        {
            continue;
        }
        if surface.fields.iter().any(|(_, field)| field.is_composing()) {
            continue;
        }
        for (field, value) in &surface.fields {
            match field.0 {
                ScriptEditorField::Code => {
                    surface.state.model.edit_script(value.value().to_string())
                }
                ScriptEditorField::InputYaml => {
                    surface.state.model.edit_yaml(value.value().to_string())
                }
            }
        }
        let request = match control {
            ScriptControl::Run => surface.state.model.run(),
            ScriptControl::Clear => surface.state.model.clear(),
            ScriptControl::Preset(id) => {
                surface.state.model.select_preset(id);
                for (field, mut value) in &mut surface.fields {
                    if field.0 == ScriptEditorField::Code {
                        value
                            .editor_mut()
                            .set_text(&surface.state.model.script_code);
                    }
                }
                None
            }
            ScriptControl::Export(kind) => {
                let draft = ScriptExportDraft {
                    kind,
                    profile: None,
                    base_yaml: surface.state.model.input_yaml.clone(),
                    mixin_yaml: String::new(),
                    script_code: surface.state.model.script_code.clone(),
                    preset: surface.state.model.selected_preset.clone(),
                };
                surface.state.model.prepare_export(draft)
            }
            ScriptControl::Confirm => surface.state.model.confirm_export(),
            ScriptControl::Cancel => surface.state.model.cancel_export(),
            ScriptControl::Retry => surface.state.model.retry(),
        };
        if let Some(request) = request {
            submit(&mut surface.state, surface.handle.as_deref(), request);
        }
    }
}
fn submit(
    state: &mut ScriptWorkbenchState,
    handle: Option<&CommandSinkHandle>,
    request: ScriptPending,
) {
    let Some(handle) = handle else {
        state.model.finish(
            request.operation,
            Err(Failure::new(
                ErrorCode::NotReady,
                "Script command service is unavailable",
                true,
            )),
        );
        return;
    };
    let command = match request.intent {
        CommandIntent::RunScriptSandbox { request } => UiCommand::RunScriptSandbox { request },
        CommandIntent::ClearScriptSandbox { operation } => {
            UiCommand::ClearScriptSandbox { operation }
        }
        CommandIntent::PrepareScriptExport { draft } => UiCommand::PrepareScriptExport {
            operation: request.operation,
            draft,
        },
        CommandIntent::SaveScriptExport { identity } => UiCommand::SaveScriptExport {
            operation: request.operation,
            identity,
        },
        CommandIntent::CancelScriptExport { identity } => UiCommand::CancelScriptExport {
            operation: request.operation,
            identity,
        },
        _ => unreachable!("script workbench command vocabulary"),
    };
    if let Some(id) = handle.submit_tracked(command) {
        state.request = Some((id, request.operation));
    } else {
        state.model.finish(
            request.operation,
            Err(Failure::new(
                ErrorCode::NotReady,
                "Script command service has no correlated terminal feedback",
                true,
            )),
        );
    }
}
fn completed(event: On<CommandExecutedEvent>, mut state: ResMut<ScriptWorkbenchState>) {
    let Some((id, operation)) = state.request else {
        return;
    };
    if id != event.request_id
        || state
            .model
            .pending
            .as_ref()
            .is_none_or(|pending| Some(pending.intent.clone()) != event.command.to_intent())
    {
        return;
    }
    state.request = None;
    state.model.finish(operation, event.result.clone());
}
