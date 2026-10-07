//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::probe_settings_store::ProbeSettingsStore;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui::Node;
use bevy::ui::prelude::Display;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::proxy_probe_options_projection::project_settings;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{
    CommandPumpPlugin, CommandSinkHandle, UiCommand, UiCommandSink,
};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::proxy_probe_settings::{
    ApplyProbeSettings, CancelProbeSettings, OpenProbeSettings, ProbeSettingsRoot,
    ProbeSettingsState, ProbeTimeoutField, ProbeUrlField,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::LatestSurfaceSnapshot;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread::yield_now;
use std::time::{Duration, Instant};

fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
}
fn field<T: Component>(app: &mut App) -> Entity {
    let wrapper = entity::<T>(app);
    app.world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .unwrap()
}
fn activate(app: &mut App, target: Entity) {
    app.world_mut()
        .commands()
        .trigger(Activate { entity: target });
    app.update();
}
fn edit(app: &mut App, target: Entity, value: &str) {
    app.world_mut()
        .get_mut::<TextField>(target)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(value.into()));
    app.update();
}
fn replay(app: &mut App, store: &ProbeSettingsStore) {
    app.world_mut()
        .resource_mut::<LatestSurfaceSnapshot>()
        .0
        .probe_settings = project_settings(Some(&Ok(store.settings.lock().unwrap().clone())));
    app.update();
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app
        .world()
        .resource::<ProbeSettingsState>()
        .editor
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "durable command must publish a terminal result"
        );
        app.update();
        yield_now();
    }
    app.update();
}

#[test]
fn native_parameter_modal_preserves_input_entities_cancel_has_no_write_and_real_failure_retries() {
    let store = Arc::new(ProbeSettingsStore::default());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_settings(SettingsApplication::new(store.clone())),
    ));
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::for_application(Arc::new(application)));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    replay(&mut app, &store);
    let open = entity::<OpenProbeSettings>(&mut app);
    let root = entity::<ProbeSettingsRoot>(&mut app);
    let url = field::<ProbeUrlField>(&mut app);
    let timeout = field::<ProbeTimeoutField>(&mut app);
    let apply = entity::<ApplyProbeSettings>(&mut app);
    let cancel = entity::<CancelProbeSettings>(&mut app);
    activate(&mut app, open);
    app.update();
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::Flex
    );
    assert!(
        !app.world()
            .get::<TextField>(timeout)
            .unwrap()
            .0
            .is_disabled()
    );
    edit(&mut app, timeout, "60000");
    assert_eq!(
        app.world()
            .resource::<ProbeSettingsState>()
            .editor
            .validation
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidInput
    );
    assert!(app.world().get::<ButtonDisabled>(apply).unwrap().0);
    activate(&mut app, apply);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    activate(&mut app, cancel);
    assert!(!app.world().resource::<ProbeSettingsState>().open);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    activate(&mut app, open);
    app.update();
    edit(&mut app, url, "https://probe.example.test/204");
    edit(&mut app, timeout, "32767");
    app.world_mut().get_mut::<TextFieldFocused>(url).unwrap().0 = true;
    replay(&mut app, &store);
    assert_eq!(
        app.world().get::<TextField>(url).unwrap().0.text(),
        "https://probe.example.test/204"
    );
    assert!(app.world().get::<TextFieldFocused>(url).unwrap().0);
    store.reject.store(true, Ordering::SeqCst);
    activate(&mut app, apply);
    settle(&mut app);
    let state = app.world().resource::<ProbeSettingsState>();
    assert!(state.open);
    assert_eq!(
        state.editor.failure.as_ref().unwrap().code,
        ErrorCode::Storage
    );
    assert_eq!(state.editor.draft.timeout_ms, "32767");
    assert_eq!(state.editor.applied.as_ref().unwrap().timeout_ms, 5000);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.reject.store(false, Ordering::SeqCst);
    activate(&mut app, apply);
    settle(&mut app);
    assert!(!app.world().resource::<ProbeSettingsState>().open);
    assert_eq!(
        app.world()
            .resource::<ProbeSettingsState>()
            .editor
            .applied
            .as_ref()
            .unwrap()
            .timeout_ms,
        32767
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    replay(&mut app, &store);
    assert_eq!(field::<ProbeUrlField>(&mut app), url);
    assert_eq!(field::<ProbeTimeoutField>(&mut app), timeout);
    assert!(app.world().get::<TextField>(url).unwrap().0.is_disabled());
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
}

#[derive(Default)]
struct DeferredProbeSink(Mutex<Vec<UiCommand>>);
impl UiCommandSink for DeferredProbeSink {
    fn submit(&self, command: UiCommand) {
        let _ = self.submit_tracked(command);
    }
    fn submit_tracked(&self, command: UiCommand) -> Option<RequestId> {
        self.0.lock().unwrap().push(command);
        Some(RequestId(45))
    }
}

#[test]
fn pending_native_fields_are_disabled_and_wrong_command_or_request_cannot_finish_the_write() {
    let sink = Arc::new(DeferredProbeSink::default());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.insert_resource(CommandSinkHandle(sink.clone()));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    app.world_mut()
        .resource_mut::<LatestSurfaceSnapshot>()
        .0
        .probe_settings
        .can_persist = true;
    let open = entity::<OpenProbeSettings>(&mut app);
    activate(&mut app, open);
    app.update();
    let timeout = field::<ProbeTimeoutField>(&mut app);
    edit(&mut app, timeout, "32767");
    let apply = entity::<ApplyProbeSettings>(&mut app);
    activate(&mut app, apply);
    app.update();
    let submitted = sink.0.lock().unwrap()[0].clone();
    let original = app
        .world()
        .resource::<ProbeSettingsState>()
        .editor
        .applied
        .clone();
    assert!(
        app.world()
            .get::<TextField>(timeout)
            .unwrap()
            .0
            .is_disabled()
    );
    assert!(app.world().get::<ButtonDisabled>(apply).unwrap().0);
    activate(&mut app, apply);
    let cancel = entity::<CancelProbeSettings>(&mut app);
    activate(&mut app, cancel);
    assert_eq!(sink.0.lock().unwrap().len(), 1);
    assert!(app.world().resource::<ProbeSettingsState>().open);
    for (request_id, command) in [
        (RequestId(44), submitted.clone()),
        (
            RequestId(45),
            UiCommand::SetProxyProbeOptions {
                options: Default::default(),
            },
        ),
        (
            RequestId(45),
            UiCommand::SetProxySearchQuery {
                query: "other operation".into(),
            },
        ),
    ] {
        app.world_mut().commands().trigger(CommandExecutedEvent {
            request_id,
            command,
            result: Ok(CommandOutput::Unit),
        });
        app.update();
        assert!(
            app.world()
                .resource::<ProbeSettingsState>()
                .editor
                .pending
                .is_some()
        );
        assert_eq!(
            app.world().resource::<ProbeSettingsState>().editor.applied,
            original
        );
    }
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Settings));
    app.update();
    assert!(!app.world().resource::<ProbeSettingsState>().open);
    assert!(
        app.world()
            .resource::<ProbeSettingsState>()
            .editor
            .pending
            .is_some(),
        "navigation cannot cancel an accepted durable write"
    );
    app.world_mut().commands().trigger(CommandExecutedEvent {
        request_id: RequestId(45),
        command: submitted,
        result: Err(Failure::new(ErrorCode::Storage, "write rejected", true)),
    });
    app.update();
    assert!(
        app.world()
            .resource::<ProbeSettingsState>()
            .editor
            .pending
            .is_none()
    );
    assert_eq!(
        app.world().resource::<ProbeSettingsState>().editor.applied,
        original
    );
    assert_eq!(
        app.world()
            .resource::<ProbeSettingsState>()
            .editor
            .draft
            .timeout_ms,
        "32767"
    );
    assert!(
        !app.world().resource::<ProbeSettingsState>().open,
        "late failure cannot reopen a departed page"
    );
}
