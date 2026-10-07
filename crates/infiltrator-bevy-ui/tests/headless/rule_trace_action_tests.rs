//! test-intent: behavior
//! Actual SDK controls, typed command terminals and a shared profile-backed reader.
use crate::command_harness::recording_application;
use crate::native_input::{click_entity, press, type_text};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Button, ButtonPlugin};
use bevy::window::{Ime, PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::rule_trace_fixtures::{
    NAMED_TRACE_DOCUMENT, RuleTraceStore, TRACE_DOCUMENT,
};
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::rules_tabs::RulesTabChip;
use infiltrator_bevy_ui::pages::rules_tracer::{
    ApplyTracerRuleOverrideButton, RulesTraceState, SimulateRuleTraceButton, TracerNativeField,
    TracerPresetChip, TracerText,
};
use infiltrator_bevy_ui::pages::rules_tracer_confirm::TraceConfirmationAction;
use infiltrator_bevy_ui::pages::rules_tracer_sandbox::{TracerSandboxField, TracerSandboxToggle};
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rule_condition::TrafficField;
use infiltrator_contract::rule_tracer::RuleTracerSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::mem::discriminant;
use std::sync::{Arc, atomic::Ordering};
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn setup() -> (App, ApplicationSurfaceReader, Arc<RuleTraceStore>) {
    let store = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(store.clone());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_rule_tracer(owner.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_rule_tracer(owner)
    .with_configuration(ConfigurationApplication::new(store.clone()));
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        CommandPumpPlugin::for_application(application),
    ));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Rules));
    app.update();
    publish(&mut app, &reader);
    let tab = app
        .world_mut()
        .query::<(Entity, &RulesTabChip)>()
        .iter(app.world())
        .find(|(_, chip)| chip.0 == 3)
        .unwrap()
        .0;
    click_entity(&mut app, tab);
    (app, reader, store)
}
fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
fn basic(app: &mut App, order: i32) -> Entity {
    app.world_mut()
        .query_filtered::<(Entity, &NativeTextField), With<TracerNativeField>>()
        .iter(app.world())
        .find(|(_, field)| field.0 == order)
        .unwrap()
        .0
}
fn sandbox(app: &mut App, wanted: TrafficField) -> Entity {
    app.world_mut()
        .query::<(Entity, &TracerSandboxField)>()
        .iter(app.world())
        .find(|(_, field)| field.field == wanted)
        .unwrap()
        .0
}
fn button<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .single(app.world())
        .unwrap()
}
fn confirmation(app: &mut App, wanted: TraceConfirmationAction) -> Entity {
    app.world_mut()
        .query_filtered::<(Entity, &TraceConfirmationAction), With<Button>>()
        .iter(app.world())
        .find(|(_, action)| discriminant(*action) == discriminant(&wanted))
        .unwrap()
        .0
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.world().resource::<RulesTraceState>().model.busy() {
        assert!(Instant::now() < deadline, "actual terminal event");
        app.update();
        yield_now();
    }
    app.update();
}
fn run(app: &mut App, reader: &ApplicationSurfaceReader) {
    let run = button::<SimulateRuleTraceButton>(app);
    click_entity(app, run);
    settle(app);
    publish(app, reader);
}
fn copy(app: &mut App) -> String {
    app.world_mut()
        .query::<(&Text, &TracerText)>()
        .iter(app.world())
        .map(|(text, _)| text.0.clone())
        .collect::<Vec<_>>()
        .join("\n")
}
#[test]
fn native_tracer_missing_context_keyboard_metadata_ime_confirmation_cancel_permission_retry_and_stale_source()
 {
    let (mut app, reader, store) = setup();
    let query = basic(&mut app, 0);
    let source = basic(&mut app, 1);
    press(&mut app, query);
    type_text(&mut app, "google.com");
    run(&mut app, &reader);
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::NotReady
    );
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report
            .is_none()
    );
    assert!(copy(&mut app).contains("来源 IP"));
    press(&mut app, source);
    type_text(&mut app, "192.0.2.1");
    run(&mut app, &reader);
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 2);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .decision_chain
            .as_ref()
            .unwrap()
            .matched_rule_raw,
        "DOMAIN-SUFFIX,google.com,PROXY"
    );
    assert!(!copy(&mut app).contains("自动测速延迟最优"));
    let advanced = button::<TracerSandboxToggle>(&mut app);
    click_entity(&mut app, advanced);
    let process = sandbox(&mut app, TrafficField::ProcessName);
    press(&mut app, process);
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap();
    app.world_mut().write_message(Ime::Enabled { window });
    app.world_mut().write_message(Ime::Preedit {
        window,
        value: "ni hao".into(),
        cursor: Some((0, 6)),
    });
    app.update();
    let run_button = button::<SimulateRuleTraceButton>(&mut app);
    assert!(app.world().get::<ButtonDisabled>(run_button).unwrap().0);
    click_entity(&mut app, run_button);
    assert_eq!(
        store.rule_loads.load(Ordering::SeqCst),
        2,
        "unfinished IME must not run a simulation"
    );
    app.world_mut().write_message(Ime::Commit {
        window,
        value: "你好".into(),
    });
    app.update();
    assert_eq!(
        app.world().get::<TextField>(process).unwrap().0.text(),
        "你好"
    );
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert_eq!(sandbox(&mut app, TrafficField::ProcessName), process);
    assert_eq!(
        app.world().get::<TextField>(process).unwrap().0.text(),
        "你好"
    );
    assert!(app.world().get::<TextFieldFocused>(process).unwrap().0);
    run(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .simulated_context
            .process_name
            .as_deref(),
        Some("你好")
    );
    let apply = button::<ApplyTracerRuleOverrideButton>(&mut app);
    click_entity(&mut app, apply);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_some()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let cancel = confirmation(&mut app, TraceConfirmationAction::Cancel);
    click_entity(&mut app, cancel);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_none()
    );
    assert_eq!(store.content(), TRACE_DOCUMENT);
    click_entity(&mut app, apply);
    let scrim = app
        .world_mut()
        .query_filtered::<Entity, (With<ModalScrim>, With<TraceConfirmationAction>)>()
        .single(app.world())
        .unwrap();
    click_entity(&mut app, scrim);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_none()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    click_entity(&mut app, apply);
    store.deny_write.store(true, Ordering::SeqCst);
    let confirm = confirmation(&mut app, TraceConfirmationAction::Apply);
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_some()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.deny_write.store(false, Ordering::SeqCst);
    click_entity(&mut app, confirm);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert!(store.content().contains("DOMAIN-SUFFIX,google.com,DIRECT"));
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_none()
    );
    assert!(
        app.world().get::<ButtonDisabled>(apply).unwrap().0,
        "old trace cannot rewrite a new source"
    );
    run(&mut app, &reader);
    let target = basic(&mut app, 2);
    press(&mut app, target);
    let length = app
        .world()
        .get::<TextField>(target)
        .unwrap()
        .0
        .text()
        .chars()
        .count();
    for _ in 0..length {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Backspace,
            logical_key: Key::Backspace,
            text: None,
            state: ButtonState::Pressed,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    type_text(&mut app, "REJECT");
    assert_eq!(
        app.world().get::<TextField>(target).unwrap().0.text(),
        "REJECT"
    );
    click_entity(&mut app, apply);
    let changed = format!("{}# changed while confirming\n", store.content());
    store.replace(&changed);
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(store.content(), changed);
}

#[test]
fn native_tracer_confirmation_isolates_background_and_permission_guide_releases_focus() {
    let (mut app, reader, store) = setup();
    let query = basic(&mut app, 0);
    let source = basic(&mut app, 1);
    press(&mut app, query);
    type_text(&mut app, "google.com");
    press(&mut app, source);
    type_text(&mut app, "192.0.2.1");
    run(&mut app, &reader);
    let apply = button::<ApplyTracerRuleOverrideButton>(&mut app);
    click_entity(&mut app, apply);
    press(&mut app, query);
    type_text(&mut app, "blocked.test");
    assert_eq!(
        app.world().get::<TextField>(query).unwrap().0.text(),
        "google.com"
    );
    let run_button = button::<SimulateRuleTraceButton>(&mut app);
    assert!(app.world().get::<ButtonDisabled>(run_button).unwrap().0);
    click_entity(&mut app, run_button);
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    store.deny_write.store(true, Ordering::SeqCst);
    let confirm = confirmation(&mut app, TraceConfirmationAction::Apply);
    click_entity(&mut app, confirm);
    settle(&mut app);
    let guide = confirmation(&mut app, TraceConfirmationAction::Settings);
    assert!(!app.world().get::<ButtonDisabled>(guide).unwrap().0);
    click_entity(&mut app, guide);
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Settings)
    );
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_none()
    );
    assert!(
        app.world_mut()
            .query::<&TextFieldFocused>()
            .iter(app.world())
            .all(|focus| !focus.0)
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.content(), TRACE_DOCUMENT);
}

#[test]
fn native_tracer_tab_close_releases_hidden_focus_and_reopens_the_same_draft() {
    let (mut app, reader, store) = setup();
    let query = basic(&mut app, 0);
    let source = basic(&mut app, 1);
    press(&mut app, query);
    type_text(&mut app, "google.com");
    press(&mut app, source);
    type_text(&mut app, "192.0.2.1");
    run(&mut app, &reader);
    let list = app
        .world_mut()
        .query::<(Entity, &RulesTabChip)>()
        .iter(app.world())
        .find(|(_, tab)| tab.0 == 0)
        .unwrap()
        .0;
    click_entity(&mut app, list);
    assert!(!app.world().get::<TextFieldFocused>(source).unwrap().0);
    type_text(&mut app, "hidden");
    assert_eq!(
        app.world().get::<TextField>(source).unwrap().0.text(),
        "192.0.2.1"
    );
    let tracer = app
        .world_mut()
        .query::<(Entity, &RulesTabChip)>()
        .iter(app.world())
        .find(|(_, tab)| tab.0 == 3)
        .unwrap()
        .0;
    click_entity(&mut app, tracer);
    assert_eq!(basic(&mut app, 0), query);
    assert_eq!(
        app.world().get::<TextField>(query).unwrap().0.text(),
        "google.com"
    );
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report
            .is_some()
    );
}

#[test]
fn native_tracer_route_remount_restores_unsubmitted_drafts_without_rerunning() {
    let (mut app, reader, store) = setup();
    let query = basic(&mut app, 0);
    let source = basic(&mut app, 1);
    press(&mut app, query);
    type_text(&mut app, "google.com");
    press(&mut app, source);
    type_text(&mut app, "192.0.2.1");
    run(&mut app, &reader);
    press(&mut app, query);
    type_text(&mut app, ".draft");
    let advanced = button::<TracerSandboxToggle>(&mut app);
    click_entity(&mut app, advanced);
    let process = sandbox(&mut app, TrafficField::ProcessName);
    press(&mut app, process);
    type_text(&mut app, "unsubmitted.exe");
    let report = app
        .world()
        .resource::<RulesTraceState>()
        .model
        .snapshot
        .report
        .clone();
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    assert!(app.world().get_entity(query).is_err());
    app.world_mut().trigger(RouteChanged(Route::Rules));
    app.update();
    app.update();
    let restored = basic(&mut app, 0);
    assert_eq!(
        app.world().get::<TextField>(restored).unwrap().0.text(),
        "google.com.draft"
    );
    let source = basic(&mut app, 1);
    assert_eq!(
        app.world().get::<TextField>(source).unwrap().0.text(),
        "192.0.2.1"
    );
    let process = sandbox(&mut app, TrafficField::ProcessName);
    assert_eq!(
        app.world().get::<TextField>(process).unwrap().0.text(),
        "unsubmitted.exe"
    );
    assert!(
        app.world_mut()
            .query::<&TextFieldFocused>()
            .iter(app.world())
            .all(|focused| !focused.0)
    );
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report,
        report
    );
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_tracer_every_shared_preset_updates_real_input_without_sending_packets_or_commands() {
    let (mut app, _, store) = setup();
    for preset in RuleTracerSnapshot::default_presets() {
        let entity = app
            .world_mut()
            .query::<(Entity, &TracerPresetChip)>()
            .iter(app.world())
            .find(|(_, chip)| chip.0 == preset.query)
            .unwrap()
            .0;
        click_entity(&mut app, entity);
        let query = basic(&mut app, 0);
        assert_eq!(
            app.world().get::<TextField>(query).unwrap().0.text(),
            preset.query
        );
        assert_eq!(
            app.world().resource::<RulesTraceState>().model.query,
            preset.query
        );
    }
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 0);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report
            .is_none()
    );
}

#[test]
fn native_named_rule_descent_confirmation_cancel_permission_retry_and_leaf_commit() {
    let (mut app, reader, store) = setup();
    store.replace(NAMED_TRACE_DOCUMENT);
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    publish(&mut app, &reader);
    let query = basic(&mut app, 0);
    press(&mut app, query);
    type_text(&mut app, "google.com:443");
    run(&mut app, &reader);
    let report = app
        .world()
        .resource::<RulesTraceState>()
        .model
        .snapshot
        .report
        .clone()
        .unwrap();
    let chain = report.decision_chain.as_ref().unwrap();
    assert_eq!(chain.hit_rule_table.as_deref(), Some("secure"));
    assert_eq!(chain.rule_path.len(), 3);
    assert_eq!(chain.target_proxy, "PROXY");
    assert!(copy(&mut app).contains("Table secure rule #1"));
    let apply = button::<ApplyTracerRuleOverrideButton>(&mut app);
    click_entity(&mut app, apply);
    let request = app
        .world()
        .resource::<RulesTraceState>()
        .model
        .confirmation
        .clone()
        .unwrap();
    assert_eq!(request.rule_table.as_deref(), Some("secure"));
    assert_eq!(request.rule_index, 0);
    assert_eq!(request.new_target, "DIRECT");
    let cancel = confirmation(&mut app, TraceConfirmationAction::Cancel);
    click_entity(&mut app, cancel);
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    click_entity(&mut app, apply);
    store.deny_write.store(true, Ordering::SeqCst);
    let confirm = confirmation(&mut app, TraceConfirmationAction::Apply);
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_some()
    );
    store.deny_write.store(false, Ordering::SeqCst);
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        store.content(),
        NAMED_TRACE_DOCUMENT.replace(
            "'DOMAIN-SUFFIX,google.com,PROXY'",
            "'DOMAIN-SUFFIX,google.com,DIRECT'"
        )
    );
    publish(&mut app, &reader);
    assert!(
        app.world().get::<ButtonDisabled>(apply).unwrap().0,
        "the source-bound projection must disable the actual apply control"
    );
    click_entity(&mut app, apply);
    assert!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .confirmation
            .is_none()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    run(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<RulesTraceState>()
            .model
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .decision_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "DIRECT"
    );
}
