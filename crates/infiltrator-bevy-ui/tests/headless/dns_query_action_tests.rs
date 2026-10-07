//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::native_input::{click_entity, type_text};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_query_actions::QuerySection;
use infiltrator_application::dns_query_application::DnsQueryApplication;
use infiltrator_application::dns_query_fixtures::{IsolatedQueries, QueryFixtureMode};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::dns_query::{QueryAction, QueryLine, QueryNameField, QueryPanel};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, UiLocale};
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::dns_query::{DnsQueryOperationId, DnsQueryRequest, DnsRecordType};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn setup(port: Arc<IsolatedQueries>) -> (App, ApplicationSurfaceReader) {
    let owner = DnsQueryApplication::new(Some(port));
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_query(owner.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_dns_query(owner);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        CommandPumpPlugin::for_application(application),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Dns));
    app.update();
    publish(&mut app, &reader);
    (app, reader)
}
fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    assert!(
        snapshot.pages.dns.data.is_none(),
        "no configuration is installed in this isolated query product"
    );
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
fn action(app: &mut App, wanted: QueryAction) -> Entity {
    app.world_mut()
        .query::<(Entity, &QueryAction, &LocalizedLabel)>()
        .iter(app.world())
        .find(|(_, action, _)| **action == wanted)
        .unwrap()
        .0
}
fn click(app: &mut App, wanted: QueryAction) {
    let entity = action(app, wanted);
    click_entity(app, entity);
}
fn input(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &QueryNameField)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.world().resource::<QueryPanel>().model.pending.is_some() {
        assert!(Instant::now() < deadline, "correlated query terminal event");
        app.update();
        yield_now();
    }
    app.update();
}
fn line(app: &mut App, wanted: QueryLine) -> String {
    app.world_mut()
        .query::<(&QueryLine, &Text)>()
        .iter(app.world())
        .find(|(line, _)| **line == wanted)
        .unwrap()
        .1
        .0
        .clone()
}

#[test]
fn native_query_keyboard_type_menu_cancel_pagination_permission_retry_and_locale_preserve_real_results()
 {
    let port = Arc::new(IsolatedQueries::default());
    let (mut app, reader) = setup(port.clone());
    click(&mut app, QueryAction::Open);
    let field = input(&mut app);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    type_text(&mut app, "music.test");
    assert_eq!(
        app.world().resource::<QueryPanel>().model.name,
        "music.test"
    );
    click(&mut app, QueryAction::Cancel);
    assert!(!app.world().resource::<QueryPanel>().model.open);
    assert!(port.requests().is_empty());
    click(&mut app, QueryAction::Open);
    click(&mut app, QueryAction::Type(DnsRecordType::Txt));
    assert_eq!(
        app.world().resource::<QueryPanel>().model.record_type,
        DnsRecordType::A,
        "a hidden native type button must not select anything"
    );
    click(&mut app, QueryAction::TypePicker);
    assert!(app.world().resource::<QueryPanel>().kind_open);
    click(&mut app, QueryAction::Type(DnsRecordType::Txt));
    assert!(!app.world().resource::<QueryPanel>().kind_open);
    port.set_mode(QueryFixtureMode::Pending);
    click(&mut app, QueryAction::Run);
    assert!(app.world().resource::<QueryPanel>().model.pending.is_some());
    let run = action(&mut app, QueryAction::Run);
    assert!(app.world().get::<ButtonDisabled>(run).unwrap().0);
    click(&mut app, QueryAction::Run);
    let deadline = Instant::now() + Duration::from_secs(5);
    while port.requests().is_empty() {
        assert!(Instant::now() < deadline);
        app.update();
        yield_now();
    }
    assert_eq!(port.requests().len(), 1);
    assert_eq!(port.requests()[0].record_type, DnsRecordType::Txt);
    let (request_id, token) = app.world().resource::<QueryPanel>().request.unwrap();
    app.world_mut().trigger(CommandExecutedEvent {
        request_id,
        command: UiCommand::QueryDns {
            operation: DnsQueryOperationId(token + 1),
            request: DnsQueryRequest {
                name: "music.test".into(),
                record_type: DnsRecordType::Txt,
            },
        },
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert_eq!(
        app.world().resource::<QueryPanel>().model.pending,
        Some(token)
    );

    click(&mut app, QueryAction::Cancel);
    assert!(app.world().resource::<QueryPanel>().model.open);
    port.set_mode(QueryFixtureMode::Answer);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(line(&mut app, QueryLine::Records).contains("observed-0 {ttl}"));
    click(&mut app, QueryAction::Next);
    click(&mut app, QueryAction::Next);
    assert!(line(&mut app, QueryLine::Records).contains("observed-17 {ttl}"));
    click(&mut app, QueryAction::Section(QuerySection::Authority));
    assert!(line(&mut app, QueryLine::Records).contains("ns.test."));
    click(&mut app, QueryAction::Section(QuerySection::Additional));
    assert!(line(&mut app, QueryLine::Records).contains("TTL 0s"));
    let previous = app
        .world()
        .resource::<QueryPanel>()
        .model
        .snapshot
        .report
        .clone();
    port.set_mode(QueryFixtureMode::Permission);
    click(&mut app, QueryAction::Run);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<QueryPanel>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(
        app.world().resource::<QueryPanel>().model.snapshot.report,
        previous
    );
    assert!(app.world().resource::<QueryPanel>().model.can_guide());
    for code in ["en-US", "zh-CN"] {
        app.insert_resource(UiLocale::new(code));
        app.update();
        assert_eq!(input(&mut app), field);
        assert_eq!(
            app.world().get::<TextField>(field).unwrap().0.text(),
            "music.test"
        );
        assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
        assert!(
            line(&mut app, QueryLine::Status)
                .ends_with("Allow controller DNS query access {reason}")
        );
    }
    port.set_mode(QueryFixtureMode::Answer);
    click(&mut app, QueryAction::Retry);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(app.world().resource::<QueryPanel>().model.failure.is_none());
    assert_eq!(port.requests().len(), 3);
    assert!(line(&mut app, QueryLine::Records).contains("observed-0 {ttl}"));
    port.set_mode(QueryFixtureMode::Authentication);
    click(&mut app, QueryAction::Run);
    settle(&mut app);
    publish(&mut app, &reader);
    click(&mut app, QueryAction::Settings);
    assert!(!app.world().resource::<QueryPanel>().model.open);
    assert!(
        !app.world().get::<TextFieldFocused>(field).unwrap().0,
        "settings guidance releases the query input focus"
    );
    type_text(&mut app, "must-not-enter-closed-query");
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "music.test"
    );
    assert_eq!(
        port.requests().len(),
        4,
        "settings guidance does not retry or write anything"
    );
}

#[test]
fn native_invalid_and_unsupported_query_cannot_invent_a_response_and_negative_answers_keep_other_sections()
 {
    let port = Arc::new(IsolatedQueries::default());
    let (mut app, reader) = setup(port.clone());
    click(&mut app, QueryAction::Open);
    type_text(&mut app, "bad..name");
    click(&mut app, QueryAction::Run);
    assert_eq!(
        app.world()
            .resource::<QueryPanel>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidInput
    );
    assert!(port.requests().is_empty());
    let field = input(&mut app);
    for _ in 0..9 {
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
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "");
    type_text(&mut app, "music.test");
    port.set_mode(QueryFixtureMode::Unsupported);
    click(&mut app, QueryAction::Run);
    settle(&mut app);
    publish(&mut app, &reader);
    let state = &app.world().resource::<QueryPanel>().model;
    assert_eq!(state.failure.as_ref().unwrap().code, ErrorCode::Unsupported);
    assert!(state.snapshot.report.is_none());
    assert!(!state.can_retry());
    port.set_mode(QueryFixtureMode::NegativeAnswer);
    click(&mut app, QueryAction::Run);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<QueryPanel>()
            .model
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .response
            .status,
        3
    );
    assert!(app.world().resource::<QueryPanel>().model.rows().is_empty());
    click(&mut app, QueryAction::Section(QuerySection::Authority));
    assert!(line(&mut app, QueryLine::Records).contains("ns.test."));
    assert_eq!(port.requests().len(), 2);
    click(&mut app, QueryAction::Cancel);
    click(&mut app, QueryAction::Open);
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    assert!(!app.world().resource::<QueryPanel>().model.open);
    assert_eq!(port.requests().len(), 2);
}
