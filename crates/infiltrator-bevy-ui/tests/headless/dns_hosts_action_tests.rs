//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::support::{headless_plugins, subtree_has_text};
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::ui::{Display, Node};
use bevy::ui_widgets::ButtonPlugin;
use bevy::window::Ime;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::dns_hosts_fixtures::{HostsCaptureStore, ORIGINAL_HOSTS};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::dns_hosts::{
    DnsHostsDomainField, DnsHostsEditorField, DnsHostsEditorState, HostAction, HostsModalCard,
    HostsModalRoot,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn setup(store: Arc<HostsCaptureStore>) -> (App, ApplicationSurfaceReader) {
    let configuration = ConfigurationApplication::new(store);
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_configuration(configuration.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_configuration(configuration);
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
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .expect("native Hosts element")
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
fn action(app: &mut App, wanted: impl Fn(&HostAction) -> bool) -> Entity {
    app.world_mut()
        .query::<(Entity, &HostAction)>()
        .iter(app.world())
        .find(|(_, action)| wanted(action))
        .expect("native Hosts action")
        .0
}
fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::None {
                width: 1180,
                height: 780,
            },
            position: Vec2::new(20.0, 20.0),
        },
    )
}
fn press(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    app.world_mut().flush();
}
fn pointer_click(app: &mut App, entity: Entity) {
    press(app, entity);
    app.world_mut().trigger(PointerClick {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        duration: Duration::from_millis(100),
        count: 1,
    });
    app.world_mut().flush();
}
fn click(app: &mut App, entity: Entity) {
    pointer_click(app, entity);
    app.update();
}
fn do_action(app: &mut App, wanted: impl Fn(&HostAction) -> bool) {
    let button = action(app, wanted);
    click(app, button);
}
fn type_value(app: &mut App, field: Entity, value: &str) {
    press(app, field);
    let mut modifiers = ButtonInput::<KeyCode>::default();
    modifiers.press(KeyCode::ControlLeft);
    app.insert_resource(modifiers);
    key(app, Key::Character("a".into()), None);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ControlLeft);
    key(app, Key::Character(value.into()), Some(value));
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), value);
}
fn key(app: &mut App, logical_key: Key, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key,
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "actual Hosts command terminal result"
        );
        app.update();
        yield_now();
    }
    app.update();
}
#[test]
fn native_hosts_keyboard_pointer_migration_failure_retry_cancel_and_empty_restore_preserve_real_profile_bytes()
 {
    let store = Arc::new(HostsCaptureStore::default());
    let (mut app, reader) = setup(store.clone());
    do_action(&mut app, |a| matches!(a, HostAction::Open));
    assert!(app.world().resource::<DnsHostsEditorState>().editor.open);
    let card = entity::<HostsModalCard>(&mut app);
    let first = app.world().resource::<DnsHostsEditorState>().editor.rows[0].id;
    do_action(
        &mut app,
        |a| matches!(a, HostAction::Edit(id) if *id == first),
    );
    let address = field::<DnsHostsEditorField>(&mut app);
    let domain = field::<DnsHostsDomainField>(&mut app);
    type_value(&mut app, address, "4.4.4.4");
    key(&mut app, Key::Tab, None);
    assert!(app.world().get::<TextFieldFocused>(domain).unwrap().0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::ShiftLeft);
    key(&mut app, Key::Tab, None);
    assert!(app.world().get::<TextFieldFocused>(address).unwrap().0);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::ShiftLeft);
    do_action(&mut app, |a| matches!(a, HostAction::CommitRow));
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.rows[0].id,
        first
    );
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.rows[0]
            .entry
            .address,
        "4.4.4.4"
    );
    assert_eq!(store.content(), ORIGINAL_HOSTS);
    do_action(&mut app, |a| matches!(a, HostAction::Cancel));
    assert!(!app.world().resource::<DnsHostsEditorState>().editor.open);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    do_action(&mut app, |a| matches!(a, HostAction::Open));
    do_action(&mut app, |a| matches!(a, HostAction::ImportLegacy));
    assert_eq!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .rows
            .len(),
        3
    );
    store.deny_save.store(true, Ordering::SeqCst);
    let apply = action(&mut app, |a| matches!(a, HostAction::Apply));
    pointer_click(&mut app, apply);
    let pending = app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .pending
        .clone()
        .unwrap();
    let (request, _) = app
        .world()
        .resource::<DnsHostsEditorState>()
        .request
        .unwrap();
    app.world_mut().trigger(CommandExecutedEvent {
        command: UiCommand::ApplyDnsSettings {
            patch: pending.patch.clone(),
        },
        request_id: RequestId(u64::MAX),
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().trigger(CommandExecutedEvent {
        command: UiCommand::ApplyDnsSettings {
            patch: Default::default(),
        },
        request_id: request,
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().flush();
    assert_eq!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .pending
            .as_ref()
            .unwrap()
            .token,
        pending.token
    );
    let cancel = action(&mut app, |a| matches!(a, HostAction::Cancel));
    pointer_click(&mut app, cancel);
    assert!(app.world().resource::<DnsHostsEditorState>().editor.open);
    settle(&mut app);
    publish(&mut app, &reader);
    let editor = &app.world().resource::<DnsHostsEditorState>().editor;
    assert_eq!(editor.failure.as_ref().unwrap().code, ErrorCode::Permission);
    assert_eq!(editor.rows.len(), 3);
    assert_eq!(editor.applied.as_ref().unwrap().entries.len(), 2);
    assert_eq!(store.content(), ORIGINAL_HOSTS);
    assert!(subtree_has_text(app.world(), card, "Allow profile writes"));
    store.deny_save.store(false, Ordering::SeqCst);
    do_action(&mut app, |a| matches!(a, HostAction::Apply));
    settle(&mut app);
    publish(&mut app, &reader);
    let editor = &app.world().resource::<DnsHostsEditorState>().editor;
    assert!(editor.failure.is_none());
    assert_eq!(editor.applied.as_ref().unwrap().entries.len(), 3);
    assert!(editor.applied.as_ref().unwrap().legacy_entries.is_empty());
    assert_eq!(store.writes.load(Ordering::SeqCst), 2);
    assert!(store.content().contains("future-setting: preserve-me"));
    let ids: Vec<_> = editor.rows.iter().map(|row| row.id).collect();
    for id in ids {
        do_action(
            &mut app,
            |a| matches!(a, HostAction::Remove(row) if *row == id),
        );
    }
    do_action(&mut app, |a| matches!(a, HostAction::Apply));
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .applied
            .as_ref()
            .unwrap()
            .entries
            .is_empty()
    );
    type_value(&mut app, address, "6.6.6.6");
    type_value(&mut app, domain, "restored.test");
    do_action(&mut app, |a| matches!(a, HostAction::CommitRow));
    do_action(&mut app, |a| matches!(a, HostAction::Apply));
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .applied
            .as_ref()
            .unwrap()
            .entries[0]
            .domain,
        "restored.test"
    );
    assert!(app.world().get_entity(card).is_ok());
}
#[test]
fn native_hosts_ime_locale_reader_fences_invalid_rows_and_navigation_keep_the_draft_owned_and_visible()
 {
    let store = Arc::new(HostsCaptureStore::default());
    let (mut app, reader) = setup(store.clone());
    do_action(&mut app, |a| matches!(a, HostAction::Open));
    let root = entity::<HostsModalRoot>(&mut app);
    let first = app.world().resource::<DnsHostsEditorState>().editor.rows[0].id;
    let address = field::<DnsHostsEditorField>(&mut app);
    let domain = field::<DnsHostsDomainField>(&mut app);
    type_value(&mut app, address, "1.1.1.1");
    type_value(&mut app, domain, "new.test");
    do_action(
        &mut app,
        |a| matches!(a, HostAction::Edit(id) if *id == first),
    );
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.address,
        "1.1.1.1"
    );
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.domain,
        "new.test"
    );
    do_action(&mut app, |a| matches!(a, HostAction::CancelRow));
    type_value(&mut app, address, "invalid");
    type_value(&mut app, domain, "bad domain");
    do_action(&mut app, |a| matches!(a, HostAction::CommitRow));
    assert!(
        !app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .issues
            .is_empty()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    type_value(&mut app, address, "1.1.1.1");
    type_value(&mut app, domain, "new.test");
    app.world_mut().write_message(Ime::Enabled {
        window: Entity::PLACEHOLDER,
    });
    app.update();
    key(&mut app, Key::Character("x".into()), Some("x"));
    assert_eq!(
        app.world().get::<TextField>(domain).unwrap().0.text(),
        "new.test",
        "an open IME session owns keys even before the first preedit"
    );
    app.world_mut().write_message(Ime::Preedit {
        window: Entity::PLACEHOLDER,
        value: "zhong".into(),
        cursor: Some((5, 5)),
    });
    app.update();
    let mut expected = app.world().get::<TextField>(domain).unwrap().0.clone();
    app.insert_resource(UiLocale::new("en-US"));
    publish(&mut app, &reader);
    expected.set_placeholder("Domain");
    assert_eq!(app.world().get::<TextField>(domain).unwrap().0, expected);
    assert!(app.world().get::<TextFieldFocused>(domain).unwrap().0);
    key(&mut app, Key::Character("x".into()), Some("x"));
    assert_eq!(
        app.world().get::<TextField>(domain).unwrap().0,
        expected,
        "preedit owns text insertion"
    );
    app.world_mut().write_message(Ime::Disabled {
        window: Entity::PLACEHOLDER,
    });
    app.update();
    assert_eq!(
        app.world().get::<TextField>(domain).unwrap().0.text(),
        "new.test"
    );
    app.world_mut().write_message(Ime::Enabled {
        window: Entity::PLACEHOLDER,
    });
    app.world_mut().write_message(Ime::Preedit {
        window: Entity::PLACEHOLDER,
        value: "uncommitted".into(),
        cursor: None,
    });
    app.update();
    press(&mut app, address);
    app.update();
    assert!(
        app.world()
            .get::<TextField>(domain)
            .unwrap()
            .0
            .preedit()
            .is_empty()
    );
    assert!(
        !app.world()
            .get::<TextField>(domain)
            .unwrap()
            .0
            .is_in_ime_transaction()
    );
    assert_eq!(
        app.world().get::<TextField>(domain).unwrap().0.text(),
        "new.test"
    );
    do_action(&mut app, |a| matches!(a, HostAction::CommitRow));
    let draft = app
        .world()
        .resource::<DnsHostsEditorState>()
        .editor
        .rows
        .clone();
    store.deny_read.store(true, Ordering::SeqCst);
    publish(&mut app, &reader);
    let editor = &app.world().resource::<DnsHostsEditorState>().editor;
    assert_eq!(editor.rows, draft);
    assert_eq!(
        editor.read_failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert!(!editor.can_apply());
    store.deny_read.store(false, Ordering::SeqCst);
    publish(&mut app, &reader);
    store.replace("hosts:\n  changed.test: 2.2.2.2\n");
    publish(&mut app, &reader);
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.rows,
        draft
    );
    assert!(
        !app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .can_apply()
    );
    app.world_mut().trigger(CommandExecutedEvent {
        command: UiCommand::ApplyDnsSettings {
            patch: Default::default(),
        },
        request_id: RequestId(u64::MAX),
        result: Ok(CommandOutput::Unit),
    });
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.rows,
        draft
    );
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(store.set_current("other"))
        .unwrap();
    publish(&mut app, &reader);
    assert_eq!(
        app.world().resource::<DnsHostsEditorState>().editor.rows,
        draft
    );
    assert!(
        !app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .can_apply()
    );
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    assert!(!app.world().resource::<DnsHostsEditorState>().editor.open);
    assert_eq!(
        app.world().get::<Node>(root).unwrap().display,
        Display::None
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert!(!app.world().get::<TextFieldFocused>(domain).unwrap().0);
}

#[test]
fn hosts_unavailable_reader_is_visible_and_disabled_native_apply_performs_no_write() {
    let store = Arc::new(HostsCaptureStore::default());
    let (mut app, _) = setup(store.clone());
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.dns_hosts =
        PageData::unavailable(Failure::unsupported("configuration reader unavailable"));
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    do_action(&mut app, |a| matches!(a, HostAction::Open));
    let card = entity::<HostsModalCard>(&mut app);
    let editor = &app.world().resource::<DnsHostsEditorState>().editor;
    assert_eq!(
        editor.read_failure.as_ref().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(!editor.can_apply());
    assert!(subtree_has_text(
        app.world(),
        card,
        "configuration reader unavailable"
    ));
    do_action(&mut app, |a| matches!(a, HostAction::Apply));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.content(), ORIGINAL_HOSTS);
    assert!(
        app.world()
            .resource::<DnsHostsEditorState>()
            .editor
            .pending
            .is_none()
    );
}
