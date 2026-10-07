//! test-intent: behavior
//! Native controls drive actual Core commands against an isolated source-bound storage adapter.
use crate::command_harness::recording_application;
use crate::native_input::click_entity;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{With, Without};
use bevy::ui::widget::Text;
use bevy::ui::{Display, InteractionDisabled, Node};
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::snapshot_restore::{OpenSnapshotRestore, RestoreState};
use infiltrator_bevy_ui::pages::snapshot_restore_scene::{
    RestoreControl, RestoreLine, RestoreOverlay,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::modal::ModalScrim;
use infiltrator_composition::snapshot_restore_fixture::{AFTER, BEFORE, SnapshotRestoreFixture};
use infiltrator_contract::error::ErrorCode;
use std::mem::discriminant;
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};
fn setup() -> (App, SnapshotRestoreFixture) {
    let fixture = SnapshotRestoreFixture::new();
    let (core, _) = recording_application();
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_snapshots(fixture.application.clone()),
    ));
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ShellPlugin::default(),
        PagesPlugin::new(DemoOverviewSource::running()),
        CommandPumpPlugin::for_application(Arc::new(core)),
    ));
    app.add_plugins(ButtonPlugin);
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Profiles));
    app.update();
    (app, fixture)
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.update();
        if !app.world().resource::<RestoreState>().model.busy() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "actual restoration command did not complete"
        );
        yield_now();
    }
    app.update();
}
fn control(app: &mut App, wanted: RestoreControl) -> Entity {
    app.world_mut()
        .query_filtered::<(Entity, &RestoreControl), Without<ModalScrim>>()
        .iter(app.world())
        .find(|(_, role)| discriminant(*role) == discriminant(&wanted))
        .unwrap()
        .0
}
#[test]
fn native_restore_reviews_complete_bytes_and_cancel_has_zero_writes_with_locale_entity_preservation()
 {
    let (mut app, fixture) = setup();
    app.world_mut()
        .trigger(OpenSnapshotRestore(fixture.target.clone()));
    settle(&mut app);
    assert_eq!(fixture.writes(), 0);
    let overlay = app
        .world_mut()
        .query::<(Entity, &RestoreOverlay)>()
        .single(app.world())
        .unwrap()
        .0;
    assert_eq!(
        app.world().get::<Node>(overlay).unwrap().display,
        Display::Flex
    );
    let content = app
        .world_mut()
        .query::<(Entity, &RestoreLine)>()
        .iter(app.world())
        .find(|(_, role)| matches!(role, RestoreLine::Content))
        .unwrap()
        .0;
    let copy = &app.world().get::<Text>(content).unwrap().0;
    assert!(copy.contains(BEFORE));
    assert!(copy.contains(AFTER));
    let identity = app
        .world()
        .resource::<RestoreState>()
        .model
        .review
        .as_ref()
        .unwrap()
        .identity
        .clone();
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert!(
        app.world()
            .get::<Text>(content)
            .unwrap()
            .0
            .contains("Complete current profile")
    );
    assert_eq!(
        app.world()
            .resource::<RestoreState>()
            .model
            .review
            .as_ref()
            .unwrap()
            .identity,
        identity
    );
    let cancel = control(&mut app, RestoreControl::Cancel);
    click_entity(&mut app, cancel);
    settle(&mut app);
    assert!(!app.world().resource::<RestoreState>().model.visible);
    assert_eq!(fixture.writes(), 0);
    assert_eq!(fixture.content(), BEFORE);
    app.world_mut()
        .trigger(OpenSnapshotRestore(fixture.target.clone()));
    settle(&mut app);
    let scrim = app
        .world_mut()
        .query_filtered::<Entity, (With<ModalScrim>, With<RestoreControl>)>()
        .single(app.world())
        .unwrap();
    assert_ne!(
        scrim, cancel,
        "card cancellation and the backdrop are distinct controls"
    );
    click_entity(&mut app, scrim);
    settle(&mut app);
    assert!(!app.world().resource::<RestoreState>().model.visible);
    assert_eq!(fixture.writes(), 0);
    assert_eq!(fixture.content(), BEFORE);
    let confirm = control(&mut app, RestoreControl::Confirm);
    assert!(app.world().get::<InteractionDisabled>(confirm).is_some());
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(fixture.writes(), 0);
}
#[test]
fn native_restore_permission_retry_commits_once_and_changed_source_never_overwrites() {
    let (mut app, fixture) = setup();
    app.world_mut()
        .trigger(OpenSnapshotRestore(fixture.target.clone()));
    settle(&mut app);
    fixture.deny(true);
    let confirm = control(&mut app, RestoreControl::Confirm);
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RestoreState>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(app.world().resource::<RestoreState>().model.visible);
    assert_eq!(fixture.writes(), 0);
    fixture.deny(false);
    let retry = control(&mut app, RestoreControl::Retry);
    click_entity(&mut app, retry);
    settle(&mut app);
    assert!(
        app.world()
            .resource::<RestoreState>()
            .model
            .restored
            .is_some()
    );
    assert_eq!(fixture.writes(), 1);
    assert_eq!(fixture.content(), AFTER);
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(fixture.writes(), 1);
    let close = control(&mut app, RestoreControl::Cancel);
    click_entity(&mut app, close);
    settle(&mut app);
    app.world_mut()
        .trigger(OpenSnapshotRestore(fixture.target.clone()));
    settle(&mut app);
    fixture.replace_source("# Later edit\nmode: direct\n");
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RestoreState>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(fixture.writes(), 1);
    assert_eq!(fixture.content(), "# Later edit\nmode: direct\n");
    click_entity(&mut app, close);
    settle(&mut app);
    assert!(!app.world().resource::<RestoreState>().model.visible);
    app.world_mut()
        .trigger(OpenSnapshotRestore(fixture.target.clone()));
    settle(&mut app);
    fixture.replace_options(Some("# externally added sidecar\n{}\n"));
    click_entity(&mut app, confirm);
    settle(&mut app);
    assert_eq!(
        app.world()
            .resource::<RestoreState>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(fixture.writes(), 1);
    click_entity(&mut app, close);
    settle(&mut app);
}
