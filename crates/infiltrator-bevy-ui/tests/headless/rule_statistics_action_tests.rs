//! test-intent: behavior
//! Actual SDK controls share source-qualified statistics and transactional cleanup.
use crate::command_harness::recording_application;
use crate::native_input::{click_entity, press, type_text};
use crate::support::headless_plugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::input_focus::InputFocus;
use bevy::ui::widget::Text;
use bevy::ui::{InteractionDisabled, Pressed};
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::{PrimaryWindow, Window};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::rule_statistics_workbench::StatisticsTab;
use infiltrator_application::rule_trace_fixtures::RuleTraceStore;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::rules_builder_input::{RuleBuilderField, RuleBuilderFieldKind};
use infiltrator_bevy_ui::pages::rules_draft::RulesDraftState;
use infiltrator_bevy_ui::pages::rules_edit::RuleToggleButton;
use infiltrator_bevy_ui::pages::rules_statistics::{
    RulesStatisticsState, StatisticsControl, StatisticsLine,
};
use infiltrator_bevy_ui::pages::rules_tabs::{RulesTabChip, RulesTabState};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rule_trace_run::{RuleTraceOperationId, RuleTraceRequest};
use infiltrator_contract::rule_tracer::TrafficContextSnapshot;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::mem::discriminant;
use std::sync::{Arc, atomic::Ordering};
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn setup(trace: bool) -> (App, ApplicationSurfaceReader, Arc<RuleTraceStore>) {
    let store = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(store.clone());
    if trace {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(owner.simulate(
                RuleTraceOperationId(1),
                RuleTraceRequest {
                    query: "special.com".into(),
                    context: TrafficContextSnapshot {
                        src_ip: Some("192.0.2.1".into()),
                        ..Default::default()
                    },
                },
                None,
            ))
            .unwrap();
    }
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
    publish(&mut app, &reader);
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
    app.update();
}
fn control(app: &mut App, wanted: StatisticsControl) -> Entity {
    app.world_mut()
        .query::<(Entity, &StatisticsControl, Option<&ModalScrim>)>()
        .iter(app.world())
        .find(|(_, action, scrim)| {
            scrim.is_none()
                && discriminant(*action) == discriminant(&wanted)
                && match (action, wanted) {
                    (StatisticsControl::Tab(a), StatisticsControl::Tab(b)) => *a == b,
                    _ => true,
                }
        })
        .unwrap()
        .0
}
fn click(app: &mut App, action: StatisticsControl) {
    let entity = control(app, action);
    click_entity(app, entity);
    app.update();
}
fn settle_terminal(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<RulesStatisticsState>()
        .model
        .clear_pending
        .is_some()
    {
        assert!(Instant::now() < deadline, "real command terminal event");
        app.update();
        yield_now();
    }
    app.update();
}

#[test]
fn native_statistics_cleanup_cancel_restores_focus_and_confirm_changes_only_the_draft() {
    let (mut app, _, store) = setup(true);
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    let field = app
        .world_mut()
        .query::<(Entity, &RuleBuilderField)>()
        .iter(app.world())
        .find(|(_, field)| field.kind == RuleBuilderFieldKind::Payload)
        .unwrap()
        .0;
    press(&mut app, field);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    let before_text = app
        .world()
        .get::<TextField>(field)
        .unwrap()
        .0
        .text()
        .to_owned();
    let before = app
        .world()
        .resource::<RulesDraftState>()
        .model
        .draft
        .clone();
    click(&mut app, StatisticsControl::Inspect);
    click(&mut app, StatisticsControl::PrepareCleanup);
    let confirmation = app
        .world()
        .resource::<RulesStatisticsState>()
        .model
        .confirmation
        .clone()
        .unwrap();
    assert!(!confirmation.targets.is_empty());
    let review = app
        .world_mut()
        .query::<(&StatisticsLine, &Text)>()
        .iter(app.world())
        .find(|(line, _)| matches!(line, StatisticsLine::Confirmation))
        .unwrap()
        .1
        .0
        .clone();
    assert!(review.starts_with("Profile: trace.yaml · 3 rows\n"));
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft,
        before
    );
    assert!(app.world().get::<InteractionDisabled>(field).is_some());
    assert!(!app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert!(!app.world().get::<TextField>(field).unwrap().0.is_disabled());
    type_text(&mut app, "background input must not arrive");
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        before_text
    );
    let toggle = app
        .world_mut()
        .query::<(Entity, &RuleToggleButton)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    assert!(app.world().get::<InteractionDisabled>(toggle).is_some());
    assert!(
        app.world()
            .get::<AccessibilityNode>(toggle)
            .unwrap()
            .is_disabled()
    );
    let tab = app
        .world_mut()
        .query::<(Entity, &RulesTabChip)>()
        .iter(app.world())
        .find(|(_, chip)| chip.0 == 3)
        .unwrap()
        .0;
    let original_tab = app.world().resource::<RulesTabState>().tab;
    assert!(app.world().get::<InteractionDisabled>(tab).is_some());
    click_entity(&mut app, tab);
    assert_eq!(app.world().resource::<RulesTabState>().tab, original_tab);
    click_entity(&mut app, toggle);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft,
        before
    );
    click(&mut app, StatisticsControl::CancelCleanup);
    assert_eq!(
        app.world().resource::<RulesDraftState>().model.draft,
        before
    );
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(field));
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(field).is_none());
    click(&mut app, StatisticsControl::PrepareCleanup);
    click(&mut app, StatisticsControl::ConfirmCleanup);
    let editor = &app.world().resource::<RulesDraftState>().model;
    for target in confirmation.targets {
        assert!(!editor.draft[editor.row_index(target.id).unwrap()].enabled);
    }
    assert!(editor.dirty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_statistics_reset_failure_retry_waits_for_actual_readback_and_keeps_localized_entities() {
    let (mut app, reader, store) = setup(true);
    let before = app
        .world()
        .resource::<RulesStatisticsState>()
        .model
        .audit
        .clone()
        .unwrap();
    click(&mut app, StatisticsControl::Tab(StatisticsTab::TopHits));
    assert_eq!(
        app.world().resource::<RulesStatisticsState>().model.tab,
        StatisticsTab::TopHits
    );
    let status = app
        .world_mut()
        .query::<(Entity, &StatisticsLine, &Text)>()
        .iter(app.world())
        .find(|(_, line, _)| matches!(line, StatisticsLine::Status))
        .unwrap()
        .0;
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    let english = app.world().get::<Text>(status).unwrap().0.clone();
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_ne!(app.world().get::<Text>(status).unwrap().0, english);
    store.deny_read.store(true, Ordering::SeqCst);
    click(&mut app, StatisticsControl::Reset);
    settle_terminal(&mut app);
    let state = &app.world().resource::<RulesStatisticsState>().model;
    assert_eq!(
        state.clear_failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(state.audit.as_ref(), Some(&before));
    assert!(state.can_reset());
    store.deny_read.store(false, Ordering::SeqCst);
    let reset = control(&mut app, StatisticsControl::Reset);
    press(&mut app, reset);
    click_entity(&mut app, reset);
    settle_terminal(&mut app);
    let state = &app.world().resource::<RulesStatisticsState>().model;
    assert!(state.awaiting_revision.is_some());
    assert_eq!(state.audit.as_ref(), Some(&before));
    assert!(app.world().get::<ButtonDisabled>(reset).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(reset).is_some());
    assert!(app.world().get::<Pressed>(reset).is_none());
    publish(&mut app, &reader);
    let state = &app.world().resource::<RulesStatisticsState>().model;
    assert!(!state.busy());
    assert_eq!(state.audit.as_ref().unwrap().total_hits, 0);
    assert!(state.audit.as_ref().unwrap().revision > before.revision);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_statistics_source_change_keeps_review_visible_and_rejects_confirmation() {
    let (mut app, reader, store) = setup(true);
    click(&mut app, StatisticsControl::Inspect);
    click(&mut app, StatisticsControl::PrepareCleanup);
    let confirmation = app
        .world()
        .resource::<RulesStatisticsState>()
        .model
        .confirmation
        .clone()
        .unwrap();
    store.select_profile("other.yaml", "rules:\n  - MATCH,DIRECT\n");
    publish(&mut app, &reader);
    let state = &app.world().resource::<RulesStatisticsState>().model;
    assert_eq!(state.audit, None);
    assert_eq!(state.confirmation, Some(confirmation));
    let confirm = control(&mut app, StatisticsControl::ConfirmCleanup);
    assert!(app.world().get::<ButtonDisabled>(confirm).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(confirm).is_some());
    click_entity(&mut app, confirm);
    assert!(
        app.world()
            .resource::<RulesStatisticsState>()
            .model
            .confirmation
            .is_some()
    );
    assert!(!app.world().resource::<RulesDraftState>().model.dirty());
    click(&mut app, StatisticsControl::CancelCleanup);
    assert!(
        app.world()
            .resource::<RulesStatisticsState>()
            .model
            .confirmation
            .is_none()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}

#[test]
fn native_statistics_unknown_is_readonly_and_language_refresh_keeps_the_status_entity() {
    let (mut app, _, store) = setup(false);
    assert!(
        app.world()
            .resource::<RulesStatisticsState>()
            .model
            .audit
            .is_none()
    );
    let reset = control(&mut app, StatisticsControl::Reset);
    assert!(app.world().get::<ButtonDisabled>(reset).unwrap().0);
    assert!(app.world().get::<InteractionDisabled>(reset).is_some());
    click_entity(&mut app, reset);
    assert!(
        app.world()
            .resource::<RulesStatisticsState>()
            .request
            .is_none()
    );
    let status = app
        .world_mut()
        .query::<(Entity, &StatisticsLine, &Text)>()
        .iter(app.world())
        .find(|(_, line, _)| matches!(line, StatisticsLine::Status))
        .unwrap()
        .0;
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(status).unwrap().0,
        "Local trace statistics · Not observed"
    );
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(status).unwrap().0,
        "本地追踪统计 · 未观测"
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
