//! Actual product read failures, source verification and native retained editor surface.
//! test-intent: behavior
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ui::widget::Text;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::filter_capture_store::{FILTER_PROFILE, FilterCaptureStore};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::profiles_editor::{
    ProfileEditorSaveButton, ProfileEditorStatusText,
};
use infiltrator_bevy_ui::pages::profiles_editor_state::ProfileEditorState;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_ports::surface::SurfaceReader;
use std::future::Future;
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;
fn run<F: Future>(future: F) -> F::Output {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let mut snapshot = run(reader.read()).unwrap();
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
#[test]
fn native_editor_keeps_its_observed_document_on_permission_failure_disables_save_and_replays_locale_until_actual_recovery()
 {
    let store = Arc::new(FilterCaptureStore::default());
    let profiles = ProfileApplication::new(store.clone());
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    ));
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_profile(profiles.clone()),
    ));
    run(core.execute(CommandIntent::LoadProfileDocument {
        profile: Some(FILTER_PROFILE.into()),
    }))
    .into_output()
    .unwrap()
    .into_profile_document()
    .unwrap();
    let reader =
        ApplicationSurfaceReader::new(core.clone(), SurfaceKind::BevyDesktop, HostKind::Desktop)
            .with_profiles(profiles);
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((
        ButtonPlugin,
        ShellPlugin::default(),
        PagesPlugin::default(),
        CommandPumpPlugin::for_application(core.clone()),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Profiles));
    app.update();
    publish(&mut app, &reader);
    let initial = app
        .world()
        .resource::<ProfileEditorState>()
        .buffer
        .full_text();
    let status = app
        .world_mut()
        .query::<(Entity, &ProfileEditorStatusText)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    let save = app
        .world_mut()
        .query::<(Entity, &ProfileEditorSaveButton)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    store.deny_read.store(true, Ordering::SeqCst);
    let failure = run(core.execute(CommandIntent::LoadProfileDocument {
        profile: Some(FILTER_PROFILE.into()),
    }))
    .into_output()
    .unwrap_err();
    assert_eq!(failure.code, ErrorCode::Permission);
    publish(&mut app, &reader);
    assert_eq!(
        app.world()
            .resource::<ProfileEditorState>()
            .buffer
            .full_text(),
        initial
    );
    assert!(app.world().get::<ButtonDisabled>(save).unwrap().0);
    assert!(
        app.world()
            .get::<Text>(status)
            .unwrap()
            .0
            .contains("编辑器读取失败")
    );
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    assert!(
        app.world()
            .get::<Text>(status)
            .unwrap()
            .0
            .contains("Editor read failed:")
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.deny_read.store(false, Ordering::SeqCst);
    run(core.execute(CommandIntent::LoadProfileDocument {
        profile: Some(FILTER_PROFILE.into()),
    }))
    .into_output()
    .unwrap()
    .into_profile_document()
    .unwrap();
    publish(&mut app, &reader);
    assert!(!app.world().get::<ButtonDisabled>(save).unwrap().0);
    assert_eq!(
        app.world()
            .resource::<ProfileEditorState>()
            .buffer
            .full_text(),
        initial
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let mut failed_page = run(reader.read()).unwrap();
    failed_page.pages.profiles.status = PageStatus::Failed {
        failure: Failure::new(ErrorCode::Storage, "profile list failed", true),
    };
    failed_page.pages.profiles.data = None;
    failed_page.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(failed_page));
    app.update();
    assert_eq!(
        app.world()
            .resource::<ProfileEditorState>()
            .buffer
            .full_text(),
        initial
    );
    assert!(!app.world().get::<ButtonDisabled>(save).unwrap().0);
}
