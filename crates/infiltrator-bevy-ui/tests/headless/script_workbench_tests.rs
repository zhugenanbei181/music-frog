//! test-intent: behavior
//! Real native input and product command terminals share the reviewed export owner.
use crate::command_harness::recording_application;
use crate::native_input::{click_before_frame, click_entity, press};
use crate::support::headless_plugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input_focus::InputFocus;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{
    CommandPumpPlugin, CommandSinkHandle, DemoCommandSink, UiCommand,
};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::profiles_script_scene::ScriptReviewControl;
use infiltrator_bevy_ui::pages::profiles_script_workbench::{
    ScriptControl, ScriptInput, ScriptWorkbenchState,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::script_export::{
    ScriptExportKind, ScriptExportReceipt, ScriptExportRequest,
};
use infiltrator_contract::script_run::ScriptEditorField;
use infiltrator_ports::error::PortError;
use infiltrator_ports::script_export::ScriptExportPort;
use std::mem::discriminant;
use std::sync::{Arc, Mutex};
use std::thread::yield_now;
use std::time::{Duration, Instant};

#[derive(Default)]
struct ExportHost {
    writes: Mutex<Vec<ScriptExportRequest>>,
    failure: Mutex<bool>,
}
impl ScriptExportPort for ExportHost {
    fn save_export(&self, request: &ScriptExportRequest) -> Result<ScriptExportReceipt, PortError> {
        if *self.failure.lock().unwrap() {
            return Err(PortError::PermissionDenied(
                "read-only export directory".into(),
            ));
        }
        self.writes.lock().unwrap().push(request.clone());
        Ok(ScriptExportReceipt {
            path: format!("/exports/{}", request.file_name),
            bytes_written: request.content.len(),
        })
    }
}
fn setup(host: Arc<ExportHost>) -> (App, ScriptApplication, ScriptExportApplication, Entity) {
    let scripts = ScriptApplication::new();
    let exports = ScriptExportApplication::new(Some(host));
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_scripts(scripts.clone(), exports.clone()),
    ));
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        CommandPumpPlugin::for_application(Arc::new(application)),
    ));
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Profiles));
    app.update();
    (app, scripts, exports, window)
}
fn field(app: &mut App, kind: ScriptEditorField) -> Entity {
    app.world_mut()
        .query::<(Entity, &ScriptInput)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == kind)
        .unwrap()
        .0
}
fn control(app: &mut App, wanted: ScriptControl, review: bool) -> Entity {
    app.world_mut()
        .query::<(
            Entity,
            &ScriptControl,
            Option<&ScriptReviewControl>,
            Option<&ModalScrim>,
        )>()
        .iter(app.world())
        .find(|(_, action, marker, scrim)| {
            marker.is_some() == review
                && scrim.is_none()
                && discriminant(*action) == discriminant(&wanted)
                && match (action, wanted) {
                    (ScriptControl::Export(a), ScriptControl::Export(b)) => *a == b,
                    (ScriptControl::Preset(a), ScriptControl::Preset(b)) => *a == b,
                    _ => true,
                }
        })
        .unwrap()
        .0
}
fn key(app: &mut App, window: Entity, logical_key: Key, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyM,
        logical_key,
        text: text.map(Into::into),
        state: ButtonState::Pressed,
        repeat: false,
        window,
    });
    app.update();
}
fn type_document(app: &mut App, window: Entity, kind: ScriptEditorField, text: &str) -> Entity {
    let entity = field(app, kind);
    press(app, entity);
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(entity));
    key(app, window, Key::Character(text.into()), Some(text));
    assert_eq!(
        app.world().get::<EditableText>(entity).unwrap().value(),
        text
    );
    entity
}
fn click(app: &mut App, action: ScriptControl, review: bool) {
    let entity = control(app, action, review);
    click_entity(app, entity);
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.world().resource::<ScriptWorkbenchState>().model.busy() {
        assert!(Instant::now() < deadline, "actual command terminal");
        app.update();
        yield_now();
    }
    app.update();
}
#[test]
fn native_multiline_run_and_clear_preserve_unicode_crlf_and_locale_entity_identity() {
    let host = Arc::new(ExportHost::default());
    let (mut app, scripts, _, window) = setup(host);
    let code = "function main(config) {\r\n  console.log(\"中文🙂\");\r\n  return config;\r\n}\r\n";
    let yaml = "mode: rule\r\n# 中文👨‍👩‍👧‍👦\r\n";
    let code_entity = type_document(&mut app, window, ScriptEditorField::Code, code);
    let yaml_entity = type_document(&mut app, window, ScriptEditorField::InputYaml, yaml);
    click(&mut app, ScriptControl::Run, false);
    settle(&mut app);
    let result = scripts.observation().result.unwrap();
    assert_eq!(result.snapshot.input_yaml, yaml);
    assert!(result.snapshot.is_success());
    assert!(
        result
            .snapshot
            .console_logs
            .iter()
            .any(|entry| entry.message == "中文🙂")
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(code_entity)
            .unwrap()
            .value(),
        code
    );
    assert_eq!(
        app.world()
            .get::<EditableText>(yaml_entity)
            .unwrap()
            .value(),
        yaml
    );
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(field(&mut app, ScriptEditorField::Code), code_entity);
    assert_eq!(
        app.world()
            .get::<EditableText>(yaml_entity)
            .unwrap()
            .value(),
        yaml
    );
    click(&mut app, ScriptControl::Clear, false);
    settle(&mut app);
    assert!(scripts.observation().result.is_none());
    assert_eq!(
        app.world()
            .get::<EditableText>(yaml_entity)
            .unwrap()
            .value(),
        yaml
    );
}
#[test]
fn native_export_cancel_is_side_effect_free_and_retry_writes_only_reviewed_bytes() {
    let host = Arc::new(ExportHost::default());
    let (mut app, _, exports, window) = setup(host.clone());
    let code = "function main(config) {\n return config;\n}\n";
    let field = type_document(&mut app, window, ScriptEditorField::Code, code);
    click(
        &mut app,
        ScriptControl::Export(ScriptExportKind::DirectiveDslScript),
        false,
    );
    settle(&mut app);
    assert!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .export_visible
    );
    assert!(app.world().get::<InteractionDisabled>(field).is_some());
    assert!(
        app.world()
            .get::<AccessibilityNode>(field)
            .unwrap()
            .is_disabled()
    );
    assert!(host.writes.lock().unwrap().is_empty());
    click(&mut app, ScriptControl::Cancel, true);
    settle(&mut app);
    assert!(
        !app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .export_visible
    );
    assert!(host.writes.lock().unwrap().is_empty());
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    click(
        &mut app,
        ScriptControl::Export(ScriptExportKind::DirectiveDslScript),
        false,
    );
    settle(&mut app);
    let review = app
        .world()
        .resource::<ScriptWorkbenchState>()
        .model
        .review
        .clone()
        .unwrap();
    *host.failure.lock().unwrap() = true;
    click(&mut app, ScriptControl::Confirm, true);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(host.writes.lock().unwrap().is_empty());
    *host.failure.lock().unwrap() = false;
    click(&mut app, ScriptControl::Retry, true);
    settle(&mut app);
    assert_eq!(
        host.writes.lock().unwrap()[0].content,
        review.snapshot.content
    );
    assert_eq!(host.writes.lock().unwrap().len(), 1);
    assert_eq!(exports.snapshot().unwrap().content, review.snapshot.content);
}
#[test]
fn native_ime_preedit_disables_actions_and_commits_once_to_the_actual_buffer() {
    let (mut app, _, _, window) = setup(Arc::new(ExportHost::default()));
    let entity = field(&mut app, ScriptEditorField::Code);
    press(&mut app, entity);
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "zhong".into(),
        cursor: Some((0, 5)),
    });
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    assert!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .script_code
            .is_empty()
    );
    let run = control(&mut app, ScriptControl::Run, false);
    assert!(app.world().get::<ButtonDisabled>(run).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(run).is_some());
    click_entity(&mut app, run);
    assert!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .pending
            .is_none()
    );
    // Clicking outside an editor ends the OS composition. A disabled action
    // must not run, and a fresh focus owns the next composition transaction.
    assert!(
        !app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    assert!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .script_code
            .is_empty()
    );
    press(&mut app, entity);
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "zhong".into(),
        cursor: Some((0, 5)),
    });
    app.update();
    assert!(
        app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "中🙂".into(),
    });
    app.update();
    assert!(
        !app.world()
            .get::<EditableText>(entity)
            .unwrap()
            .is_composing()
    );
    assert_eq!(
        app.world().get::<EditableText>(entity).unwrap().value(),
        "中🙂"
    );
    assert_eq!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .script_code,
        "中🙂"
    );
}

#[test]
fn native_pending_input_and_unrelated_or_unit_terminals_cannot_complete_execution() {
    let (mut app, _, _, window) = setup(Arc::new(ExportHost::default()));
    let code = "function main(config) { return config; }";
    let entity = type_document(&mut app, window, ScriptEditorField::Code, code);
    type_document(
        &mut app,
        window,
        ScriptEditorField::InputYaml,
        "mode: rule\n",
    );
    let recording = Arc::new(DemoCommandSink::accepting());
    app.insert_resource(CommandSinkHandle(recording.clone()));
    let run = control(&mut app, ScriptControl::Run, false);
    click_before_frame(&mut app, run);
    app.update();
    assert!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .is_running()
    );
    assert!(app.world().get::<ButtonDisabled>(run).unwrap().0);
    press(&mut app, entity);
    key(
        &mut app,
        window,
        Key::Character("changed".into()),
        Some("changed"),
    );
    assert_eq!(
        app.world().get::<EditableText>(entity).unwrap().value(),
        code
    );
    assert_eq!(recording.submitted().len(), 1);
    let (id, _) = app
        .world()
        .resource::<ScriptWorkbenchState>()
        .request
        .unwrap();
    let command = recording.submitted().pop().unwrap();
    assert!(matches!(command, UiCommand::RunScriptSandbox { .. }));
    app.world_mut().trigger(CommandExecutedEvent {
        command: command.clone(),
        request_id: RequestId(id.0 + 1),
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert!(
        app.world()
            .resource::<ScriptWorkbenchState>()
            .model
            .is_running()
    );
    app.world_mut().trigger(CommandExecutedEvent {
        command,
        request_id: id,
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    let state = app.world().resource::<ScriptWorkbenchState>();
    assert!(!state.model.busy());
    assert!(state.model.snapshot.is_none());
    assert_eq!(
        state.model.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
}
