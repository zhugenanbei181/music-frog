//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::probe_settings_store::ProbeSettingsStore;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui::widget::Text;
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::language_choice::project_language;
use infiltrator_application::settings_application::SettingsApplication;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::command_palette::{CommandPaletteState, OpenCommandPalette};
use infiltrator_bevy_ui::pages::settings::settings_core::{SettingsLine, SettingsLineKind};
use infiltrator_bevy_ui::pages::settings::settings_lan::LanBindAddressField;
use infiltrator_bevy_ui::pages::settings_language::{
    LanguageChoiceButton, LanguageChoiceResource, RetryLanguageChoice,
};
use infiltrator_bevy_ui::route::{PageRoot, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::language::LanguagePreference;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::yield_now;
use std::time::{Duration, Instant};

fn activate(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}
fn settle(app: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while app
        .world()
        .resource::<LanguageChoiceResource>()
        .model
        .pending
        .is_some()
    {
        assert!(
            Instant::now() < deadline,
            "actual language command must finish"
        );
        app.update();
        yield_now();
    }
    app.update();
}
fn replay(app: &mut App, store: &ProbeSettingsStore) {
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.language_settings =
        project_language(Some(&Ok(store.settings.lock().unwrap().clone())));
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
}
#[test]
fn real_language_buttons_retry_failed_save_and_refresh_keeps_entities_draft_selection_preedit_and_palette_locale()
 {
    let store = Arc::new(ProbeSettingsStore::default());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_settings(SettingsApplication::new(store.clone())),
    ));
    let mut app = App::new();
    headless_plugins(&mut app);
    app.insert_resource(UiLocale::new("zh-CN"));
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::for_application(Arc::new(application)));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Settings));
    app.update();
    replay(&mut app, &store);
    let root = app
        .world_mut()
        .query::<(Entity, &PageRoot)>()
        .iter(app.world())
        .find(|(_, root)| root.0 == Route::Settings)
        .unwrap()
        .0;
    let wrapper = app
        .world_mut()
        .query::<(Entity, &LanBindAddressField)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    let field = app
        .world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .unwrap();
    {
        let mut input = app.world_mut().get_mut::<TextField>(field).unwrap();
        input
            .0
            .apply(TextFieldInput::SetText("unsaved address {name}".into()));
        input.0.apply(TextFieldInput::SelectAll);
        input.0.set_preedit("zhong");
    }
    app.world_mut()
        .get_mut::<TextFieldFocused>(field)
        .unwrap()
        .0 = true;
    let expected = app.world().get::<TextField>(field).unwrap().0.clone();
    let english = app
        .world_mut()
        .query::<(Entity, &LanguageChoiceButton)>()
        .iter(app.world())
        .find(|(_, button)| button.0 == LanguagePreference::English)
        .unwrap()
        .0;
    store.reject.store(true, Ordering::SeqCst);
    activate(&mut app, english);
    settle(&mut app);
    assert_eq!(app.world().resource::<UiLocale>().code(), "zh-CN");
    assert_eq!(
        app.world()
            .resource::<LanguageChoiceResource>()
            .model
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Storage
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    store.reject.store(false, Ordering::SeqCst);
    let retry = app
        .world_mut()
        .query::<(Entity, &RetryLanguageChoice)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    assert!(!app.world().get::<ButtonDisabled>(retry).unwrap().0);
    activate(&mut app, retry);
    settle(&mut app);
    assert_eq!(app.world().resource::<UiLocale>().code(), "en-US");
    assert_eq!(store.settings.lock().unwrap().language, "en-US");
    let mut old = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    old.revision += 1;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(old));
    app.update();
    assert_eq!(app.world().resource::<UiLocale>().code(), "en-US");
    replay(&mut app, &store);
    assert!(app.world().get::<PageRoot>(root).is_some());
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, expected);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    let english_text = app
        .world()
        .get::<Children>(english)
        .unwrap()
        .iter()
        .find_map(|child| app.world().get::<Text>(*child))
        .unwrap();
    assert_eq!(english_text.0, "English");
    let summary = app
        .world_mut()
        .query::<(&Text, &SettingsLine)>()
        .iter(app.world())
        .find(|(_, marker)| marker.0 == SettingsLineKind::Summary)
        .unwrap()
        .0;
    assert_eq!(summary.0, "System and core settings · shared policy");
    let permission = app
        .world_mut()
        .query::<(&Text, &LocalizedText)>()
        .iter(app.world())
        .find(|(_, copy)| copy.key == "settings_tun_permission_hint")
        .unwrap()
        .0;
    assert!(permission.0.starts_with("Grant platform permissions"));
    app.world_mut().commands().trigger(OpenCommandPalette);
    app.update();
    assert_eq!(
        app.world().resource::<CommandPaletteState>().language,
        "en-US",
        "palette consumes the same committed locale"
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert!(
        app.world_mut()
            .query::<&LocalizedText>()
            .iter(app.world())
            .any(|copy| copy.key == "language_system")
    );
}
