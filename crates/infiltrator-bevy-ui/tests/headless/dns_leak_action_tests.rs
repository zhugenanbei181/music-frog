//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::support::{headless_plugins, subtree_has_text};
use bevy::app::App;
use bevy::ecs::{component::Component, entity::Entity};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_application::dns_leak_fixtures::{EchoMode, IsolatedEcho};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::dns::{DnsProjectionUpdated, TestDnsLeakButton};
use infiltrator_bevy_ui::pages::dns_leak::LeakCard;
use infiltrator_bevy_ui::pages::dns_leak_actions::{LeakActions, RetryLeakButton};
use infiltrator_bevy_ui::pages::dns_leak_rows::LeakRowIdentity;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{SurfaceSnapshotUpdated, dns_projection};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;
fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .expect("mounted native DNS control")
        .0
}
fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<LeakActions>()
        .state
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "actual echo probe terminal feedback"
        );
        app.update();
        yield_now();
    }
    app.update();
}
fn click(app: &mut App, button: Entity) {
    app.world_mut()
        .commands()
        .trigger(Activate { entity: button });
    app.world_mut().flush();
}
#[test]
fn native_probe_failure_retry_and_unsupported_stay_on_the_surface_with_real_facts_and_locale_replay()
 {
    let echo = Arc::new(IsolatedEcho::default());
    let leak = DnsLeakApplication::new(Some(echo.clone()), default_echo_sources());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_leak(leak.clone()),
    ));
    let application = Arc::new(application);
    let reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_dns_leak(leak.clone());
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::default());
    app.add_plugins(CommandPumpPlugin::for_application(application.clone()));
    app.update();
    app.world_mut().commands().trigger(RouteChanged(Route::Dns));
    app.update();
    publish(&mut app, &reader);
    let run = entity::<TestDnsLeakButton>(&mut app);
    let card = entity::<LeakCard>(&mut app);
    click(&mut app, run);
    let token = app.world().resource::<LeakActions>().state.pending.unwrap();
    app.world_mut().commands().trigger(CommandExecutedEvent {
        command: UiCommand::TestDnsLeak,
        request_id: RequestId(u64::MAX),
        result: Ok(CommandOutput::Unit),
    });
    app.world_mut().flush();
    assert_eq!(
        app.world().resource::<LeakActions>().state.pending,
        Some(token)
    );
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(subtree_has_text(app.world(), card, "203.0.113.9"));
    assert!(subtree_has_text(app.world(), card, "198.51.100.7"));
    let rows: Vec<_> = app
        .world_mut()
        .query::<(Entity, &LeakRowIdentity)>()
        .iter(app.world())
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(rows.len(), 2);
    echo.set_mode(EchoMode::Denied);
    click(&mut app, run);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<LeakActions>()
            .state
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(subtree_has_text(app.world(), card, "allow DNS echo access"));
    assert!(
        rows.iter()
            .all(|entity| app.world().get_entity(*entity).is_ok()),
        "failed read keeps the actual rows"
    );
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    assert!(subtree_has_text(
        app.world(),
        card,
        "2 distinct resolver identities"
    ));
    assert!(
        rows.iter()
            .all(|entity| app.world().get_entity(*entity).is_ok())
    );
    echo.set_mode(EchoMode::Consistent);
    let retry = entity::<RetryLeakButton>(&mut app);
    click(&mut app, retry);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(subtree_has_text(
        app.world(),
        card,
        "2 sources observed the same resolver identity"
    ));
    echo.set_mode(EchoMode::AllSourcesFailed);
    click(&mut app, run);
    settle(&mut app);
    publish(&mut app, &reader);
    assert!(subtree_has_text(
        app.world(),
        card,
        "Every probe source failed"
    ));
    assert!(!subtree_has_text(app.world(), card, "observed identity:"));
    let unsupported = DnsLeakApplication::unconfigured();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_leak(unsupported.clone()),
    ));
    let unsupported_reader = ApplicationSurfaceReader::new(
        application.clone(),
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
    )
    .with_dns_leak(unsupported);
    click(&mut app, run);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<LeakActions>()
            .state
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Unsupported
    );
    let snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(unsupported_reader.read())
        .unwrap();
    app.world_mut()
        .commands()
        .trigger(DnsProjectionUpdated(dns_projection(&snapshot)));
    app.update();
    app.update();
    assert!(subtree_has_text(
        app.world(),
        card,
        "no DNS leak fact source"
    ));
    assert_eq!(
        app.world_mut()
            .query::<&LeakRowIdentity>()
            .iter(app.world())
            .count(),
        0
    );
    assert!(matches!(
        snapshot.dns_leak.operation,
        DnsLeakOperation::Failed { .. }
    ));
    echo.set_mode(EchoMode::Consistent);
    application.install_command_handler(Arc::new(CommandApplication::new().with_dns_leak(leak)));
    click(&mut app, run);
    settle(&mut app);
    publish(&mut app, &reader);
    assert_eq!(
        app.world_mut()
            .query::<&LeakRowIdentity>()
            .iter(app.world())
            .count(),
        2
    );
    assert!(subtree_has_text(
        app.world(),
        card,
        "2 sources observed the same resolver identity"
    ));
}
