//! test-intent: behavior
//! Native observers, production command pump, actual coherent reader and persisted results.
use crate::native_input::{click_entity, replace_text};
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ui_widgets::ButtonPlugin;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::filter_capture_store::{
    FILTER_POLICY, FILTER_PROFILE, FilterCaptureStore,
};
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::pages::profiles_editor_filter::{EditorFilterDiscard, EditorFilterText};
use infiltrator_bevy_ui::pages::profiles_editor_panes::{
    EditorFilterSaveButton, ProfileEditorOptionsState, ProfileEditorPane, ProfileEditorPaneButton,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::subscription_filter_form::FilterField;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::future::Future;
use std::sync::{Arc, atomic::Ordering};
use std::thread::yield_now;
use std::time::{Duration, Instant};
use tokio::runtime::Builder;

fn block_on<F: Future>(future: F) -> F::Output {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}
fn publish(app: &mut App, reader: &ApplicationSurfaceReader) {
    let mut snapshot = block_on(reader.read()).unwrap();
    snapshot.revision = app.world().resource::<LatestSurfaceSnapshot>().0.revision + 1;
    app.world_mut().trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
}
fn field(app: &mut App, kind: FilterField) -> Entity {
    app.world_mut()
        .query::<(Entity, &EditorFilterText)>()
        .iter(app.world())
        .find(|(_, field)| field.0 == kind)
        .unwrap()
        .0
}
fn edit(app: &mut App, kind: FilterField, value: &str) {
    let entity = field(app, kind);
    click_entity(app, entity);
    replace_text(app, value);
}
fn wait_terminal(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app
        .world()
        .resource::<ProfileEditorOptionsState>()
        .filter
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "actual application terminal must arrive"
        );
        app.update();
        yield_now();
    }
}
fn setup() -> (App, ApplicationSurfaceReader, Arc<FilterCaptureStore>) {
    let store = Arc::new(FilterCaptureStore::default());
    let core = Arc::new(CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    ));
    let profiles = ProfileApplication::new(store.clone());
    core.install_command_handler(Arc::new(
        CommandApplication::new().with_profile(profiles.clone()),
    ));
    block_on(core.execute(CommandIntent::LoadProfileDocument {
        profile: Some(FILTER_PROFILE.into()),
    }))
    .into_output()
    .and_then(|output| output.into_profile_document())
    .unwrap();
    block_on(core.execute(CommandIntent::LoadProfileOptions {
        profile: Some(FILTER_PROFILE.into()),
    }))
    .into_output()
    .unwrap()
    .into_profile_options()
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
        CommandPumpPlugin::for_application(core),
    ));
    app.update();
    app.world_mut().trigger(RouteChanged(Route::Profiles));
    app.update();
    publish(&mut app, &reader);
    let tab = app
        .world_mut()
        .query::<(Entity, &ProfileEditorPaneButton)>()
        .iter(app.world())
        .find(|(_, tab)| tab.pane == ProfileEditorPane::Filter)
        .unwrap()
        .0;
    click_entity(&mut app, tab);
    (app, reader, store)
}
#[test]
fn native_filter_transaction_reads_actual_policy_cancels_then_denies_retries_and_reports_persisted_nodes()
 {
    let (mut app, reader, store) = setup();
    let original = store.observed();
    assert!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .advanced_policy
            .as_ref()
            .unwrap()
            .contains("drop-private-ip")
    );
    edit(&mut app, FilterField::Include, "unsaved");
    let discard = entity::<EditorFilterDiscard>(&mut app);
    click_entity(&mut app, discard);
    assert_eq!(store.observed(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    edit(&mut app, FilterField::Include, "HK");
    edit(&mut app, FilterField::Advanced, FILTER_POLICY);
    store.deny_write.store(true, Ordering::SeqCst);
    let save = entity::<EditorFilterSaveButton>(&mut app);
    click_entity(&mut app, save);
    wait_terminal(&mut app);
    let editor = &app.world().resource::<ProfileEditorOptionsState>().filter;
    assert_eq!(editor.failure.as_ref().unwrap().code, ErrorCode::Permission);
    assert_eq!(editor.draft.include, "HK");
    assert_eq!(editor.draft.advanced_policy.as_deref(), Some(FILTER_POLICY));
    assert_eq!(store.observed(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.deny_write.store(false, Ordering::SeqCst);
    click_entity(&mut app, save);
    wait_terminal(&mut app);
    let editor = &app.world().resource::<ProfileEditorOptionsState>().filter;
    let report = editor.report.as_ref().unwrap();
    assert_eq!(
        (report.total_input, report.passed, report.excluded_by_server),
        (3, 1, 1)
    );
    assert!(editor.applied);
    assert!(editor.failure.is_none());
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    let saved = store.observed();
    assert!(saved.content.contains("HK-public"));
    assert!(!saved.content.contains("HK-private"));
    publish(&mut app, &reader);
    assert!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .current()
    );
    assert_eq!(
        app.world()
            .resource::<ProfileEditorOptionsState>()
            .filter
            .draft
            .include,
        "HK"
    );
}
