//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::support::{headless_plugins, subtree_has_text};
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::ui::Node;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_cache_application::DnsCacheApplication;
use infiltrator_application::dns_cache_fixtures::{CachePortMode, IsolatedCaches};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::dns::ClearDnsCacheButton;
use infiltrator_bevy_ui::pages::dns_cache::{
    CacheAction, CacheConfirmation, CacheModalCard, CacheModalRoot,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::dns_cache::{DnsCacheOperationId, DnsFlushOutcome};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;
fn setup(owner: DnsCacheApplication) -> (App, ApplicationSurfaceReader) {
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_cache(owner.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_dns_cache(owner);
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
fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .expect("mounted cache control")
        .0
}
fn button(app: &mut App, wanted: impl Fn(&CacheAction) -> bool) -> Entity {
    app.world_mut()
        .query::<(Entity, &CacheAction)>()
        .iter(app.world())
        .find(|(_, action)| wanted(action))
        .unwrap()
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
fn click(app: &mut App, entity: Entity) {
    app.world_mut().trigger(PointerPress {
        entity,
        pointer: pointer(),
        button: PointerButton::Primary,
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        count: 1,
    });
    app.world_mut().flush();
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
fn open(app: &mut App) {
    let launcher = entity::<ClearDnsCacheButton>(app);
    click(app, launcher);
    app.update();
    assert!(app.world().resource::<CacheConfirmation>().model.open);
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
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<CacheConfirmation>()
        .model
        .pending
        .is_some()
    {
        assert!(Instant::now() < deadline, "actual cache command terminal");
        app.update();
        yield_now();
    }
    app.update();
}
#[test]
fn native_cache_confirmation_cancel_escape_navigation_and_permission_retry_keep_actual_target_facts()
 {
    let ports = Arc::new(IsolatedCaches::default());
    let (mut app, reader) = setup(DnsCacheApplication::new(
        Some(ports.clone()),
        Some(ports.clone()),
    ));
    let before = ports.contents();
    open(&mut app);
    let card = entity::<CacheModalCard>(&mut app);
    let cancel = button(&mut app, |a| matches!(a, CacheAction::Cancel));
    let confirm = button(&mut app, |a| matches!(a, CacheAction::Confirm));
    click(&mut app, cancel);
    app.update();
    assert!(!app.world().resource::<CacheConfirmation>().model.open);
    click(&mut app, confirm);
    app.update();
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 0);
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 0);
    assert_eq!(ports.contents(), before);
    open(&mut app);
    let mut keys = ButtonInput::<KeyCode>::default();
    keys.press(KeyCode::Escape);
    app.insert_resource(keys);
    app.update();
    assert!(!app.world().resource::<CacheConfirmation>().model.open);
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    open(&mut app);
    app.world_mut().trigger(RouteChanged(Route::Settings));
    app.update();
    assert!(!app.world().resource::<CacheConfirmation>().model.open);
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 0);
    app.world_mut().trigger(RouteChanged(Route::Dns));
    app.update();
    open(&mut app);
    ports.set_system_mode(CachePortMode::Denied);
    click(&mut app, confirm);
    let (request, token) = app.world().resource::<CacheConfirmation>().request.unwrap();
    app.world_mut().trigger(CommandExecutedEvent {
        request_id: RequestId(u64::MAX),
        command: UiCommand::ClearDnsCache {
            operation: DnsCacheOperationId(token),
        },
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().trigger(CommandExecutedEvent {
        request_id: request,
        command: UiCommand::ClearDnsCache {
            operation: DnsCacheOperationId(token + 1),
        },
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().flush();
    assert_eq!(
        app.world().resource::<CacheConfirmation>().model.pending,
        Some(token)
    );
    click(&mut app, cancel);
    assert!(app.world().resource::<CacheConfirmation>().model.open);
    settle(&mut app);
    publish(&mut app, &reader);
    let model = &app.world().resource::<CacheConfirmation>().model;
    assert_eq!(model.failure.as_ref().unwrap().code, ErrorCode::Permission);
    assert_eq!(model.snapshot.report.fake_ip, DnsFlushOutcome::Flushed);
    assert!(
        matches!(model.snapshot.report.os_cache, DnsFlushOutcome::Failed { ref failure } if failure.code == ErrorCode::Permission)
    );
    assert!(ports.contents().0.is_empty());
    assert_eq!(ports.contents().1, before.1);
    assert!(subtree_has_text(
        app.world(),
        card,
        "Allow system resolver cache access"
    ));
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert!(subtree_has_text(app.world(), card, "Retry clearing"));
    assert!(subtree_has_text(
        app.world(),
        card,
        "Per-target results of this confirmed request"
    ));
    ports.set_system_mode(CachePortMode::Allowed);
    let retry = button(&mut app, |a| matches!(a, CacheAction::Retry));
    click(&mut app, retry);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(
        app.world()
            .resource::<CacheConfirmation>()
            .model
            .failure
            .is_none()
    );
    assert!(subtree_has_text(app.world(), card, "Clearing finished"));
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 2);
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 2);
    assert!(ports.contents().0.is_empty() && ports.contents().1.is_empty());
    let root = entity::<CacheModalRoot>(&mut app);
    assert!(app.world().get::<Node>(root).is_some());
}
#[test]
fn native_cache_reports_both_unsupported_targets_and_preserves_contents_without_configuration() {
    let ports = Arc::new(IsolatedCaches::default());
    ports.set_fake_mode(CachePortMode::Unsupported);
    ports.set_system_mode(CachePortMode::Unsupported);
    let (mut app, reader) = setup(DnsCacheApplication::new(
        Some(ports.clone()),
        Some(ports.clone()),
    ));
    assert!(
        app.world()
            .resource::<LatestSurfaceSnapshot>()
            .0
            .pages
            .dns
            .data
            .is_none()
    );
    let before = ports.contents();
    open(&mut app);
    let confirm = button(&mut app, |a| matches!(a, CacheAction::Confirm));
    click(&mut app, confirm);
    settle(&mut app);
    publish(&mut app, &reader);
    let model = &app.world().resource::<CacheConfirmation>().model;
    assert_eq!(model.failure.as_ref().unwrap().code, ErrorCode::Unsupported);
    assert!(matches!(
        model.snapshot.report.fake_ip,
        DnsFlushOutcome::Unsupported { .. }
    ));
    assert!(matches!(
        model.snapshot.report.os_cache,
        DnsFlushOutcome::Unsupported { .. }
    ));
    assert!(!model.can_retry());
    assert_eq!(ports.contents(), before);
}
