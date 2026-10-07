//! test-intent: behavior
use crate::support::headless_plugins;
use bevy::a11y::AccessibilityNode;
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::scene::WorldSceneExt;
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Checkbox, CheckboxPlugin, ToggleChecked, checkbox_self_update};
use infiltrator_bevy_ui::localization::LocalizationPlugin;
use infiltrator_bevy_ui::localized_widgets::{localized_checkbox_scene, localized_field_scene};
use infiltrator_bevy_widgets::localization::{
    LocalizedLabel, LocalizedPlaceholder, LocalizedText, UiLocale,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text_input::state::{TextFieldInput, TextFieldState};
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_bevy_widgets::theme::Theme;
use infiltrator_contract::language::LanguagePreference;
use infiltrator_shared::locales::{Lang, Localizer};

#[test]
fn shell_mode_text_and_accessibility_refresh_in_place_from_shared_copy() {
    use infiltrator_bevy_ui::app::{ShellPlugin, SidebarScriptModePill};
    use infiltrator_bevy_ui::pages::overview::OverviewModePill;
    use infiltrator_contract::command::ProxyMode;

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    app.update();
    let roots = app
        .world_mut()
        .query::<(
            Entity,
            Option<&OverviewModePill>,
            Option<&SidebarScriptModePill>,
        )>()
        .iter(app.world())
        .filter_map(|(entity, mode, script)| {
            mode.map(|mode| (entity, mode.0))
                .or_else(|| script.map(|_| (entity, ProxyMode::Script)))
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 4);
    let labels = roots
        .iter()
        .map(|(root, mode)| {
            let label = app
                .world()
                .get::<Children>(*root)
                .unwrap()
                .iter()
                .copied()
                .find(|child| app.world().get::<LocalizedText>(*child).is_some())
                .unwrap();
            (label, *mode)
        })
        .collect::<Vec<_>>();
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    for ((root, _), (label, mode)) in roots.iter().zip(&labels) {
        let expected = match mode {
            ProxyMode::Rule => "Rule",
            ProxyMode::Global => "Global",
            ProxyMode::Direct => "Direct",
            ProxyMode::Script => "Script",
        };
        assert_eq!(app.world().get::<Text>(*label).unwrap().0, expected);
        assert_eq!(
            app.world().get::<AccessibilityNode>(*root).unwrap().label(),
            Some(expected)
        );
        assert!(app.world().get::<Children>(*root).unwrap().contains(label));
    }
}

#[test]
fn typed_scene_labels_keep_native_checkbox_input_accessibility_focus_selection_and_preedit() {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins((LocalizationPlugin, CheckboxPlugin));
    app.insert_resource(UiLocale::new("zh-CN"));
    let palette = UiPalette::new(&Theme::dark());
    let checkbox = app
        .world_mut()
        .spawn_scene(localized_checkbox_scene(
            LocalizedText::plain("aggregator_dedup"),
            true,
            &palette,
        ))
        .expect("native checkbox scene resolves")
        .id();
    app.world_mut()
        .entity_mut(checkbox)
        .observe(checkbox_self_update);
    let field = app
        .world_mut()
        .spawn_scene(localized_field_scene(
            "user draft {count}".into(),
            LocalizedText::plain("aggregator_name_placeholder"),
            &palette,
        ))
        .expect("native field scene resolves")
        .id();
    app.update();
    assert!(app.world().get::<Checkbox>(checkbox).is_some());
    assert!(app.world().get::<Checked>(checkbox).is_some());
    let label = app
        .world()
        .get::<Children>(checkbox)
        .unwrap()
        .iter()
        .copied()
        .find(|entity| app.world().get::<LocalizedText>(*entity).is_some())
        .unwrap();
    let mut state = app.world_mut().get_mut::<TextField>(field).unwrap();
    state.0.apply(TextFieldInput::SelectAll);
    state.0.set_preedit("zhong");
    let mut expected = state.0.clone();
    app.world_mut()
        .get_mut::<TextFieldFocused>(field)
        .unwrap()
        .0 = true;
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    expected.set_placeholder(Lang("en-US").tr("aggregator_name_placeholder").as_ref());
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, expected);
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(
        app.world().get::<Text>(label).unwrap().0,
        Lang("en-US").tr("aggregator_dedup")
    );
    assert_eq!(
        app.world()
            .get::<AccessibilityNode>(checkbox)
            .unwrap()
            .label(),
        Some(Lang("en-US").tr("aggregator_dedup").as_ref())
    );
    assert_eq!(
        app.world().get::<AccessibilityNode>(field).unwrap().label(),
        Some(Lang("en-US").tr("aggregator_name_placeholder").as_ref())
    );
    app.world_mut().trigger(ToggleChecked { entity: checkbox });
    app.update();
    assert!(
        app.world().get::<Checked>(checkbox).is_none(),
        "the native SDK event reaches the original root"
    );
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(label).unwrap().0,
        Lang("zh-CN").tr("aggregator_dedup")
    );
    assert!(app.world().get::<Checked>(checkbox).is_none());
}

#[test]
fn locale_changes_replay_text_and_accessibility_without_replacing_focused_controls() {
    let mut app = App::new();
    app.insert_resource(UiLocale::new("en-US"));
    app.add_plugins(LocalizationPlugin);
    let caption = app
        .world_mut()
        .spawn(LocalizedText::plain("proxies_filter_alive"))
        .id();
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        Lang("en-US").tr("proxies_filter_alive")
    );
    let mut editing = TextFieldState::new("用户数据 {count}");
    editing.apply(TextFieldInput::SelectAll);
    editing.set_preedit("zhong");
    let field = app
        .world_mut()
        .spawn((
            TextField(editing),
            TextFieldFocused(true),
            AccessibilityNode(accesskit::Node::new(accesskit::Role::TextInput)),
            LocalizedLabel::plain("proxies_search_placeholder"),
            LocalizedPlaceholder::plain("proxies_search_placeholder"),
        ))
        .id();
    app.update();
    let mut expected = app.world().get::<TextField>(field).unwrap().0.clone();
    expected.set_placeholder(Lang("zh-CN").tr("proxies_search_placeholder").as_ref());
    app.world_mut().insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        Lang("zh-CN").tr("proxies_filter_alive")
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "用户数据 {count}"
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, expected);
    assert_eq!(
        app.world().get::<AccessibilityNode>(field).unwrap().label(),
        Some(Lang("zh-CN").tr("proxies_search_placeholder").as_ref())
    );
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        Lang("en-US").tr("proxies_filter_alive")
    );
    assert!(app.world().get_entity(field).is_ok());
    expected.set_placeholder(Lang("en-US").tr("proxies_search_placeholder").as_ref());
    assert_eq!(app.world().get::<TextField>(field).unwrap().0, expected);
}

#[test]
fn native_navigation_labels_and_title_follow_locale_without_remounting() {
    use crate::support::headless_plugins;
    use infiltrator_bevy_ui::app::{ContentTitleLabel, ShellPlugin, SidebarNavItem};
    use infiltrator_bevy_ui::route::ActiveRoute;
    use infiltrator_bevy_ui::route::Route;
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.insert_resource(ActiveRoute(Some(Route::Settings)));
    app.update();
    let nav = app
        .world_mut()
        .query::<(Entity, &SidebarNavItem)>()
        .iter(app.world())
        .map(|(entity, item)| (entity, item.0))
        .collect::<Vec<_>>();
    assert_eq!(nav.len(), Route::ALL.len());
    let title = app
        .world_mut()
        .query::<(Entity, &ContentTitleLabel)>()
        .single(app.world())
        .unwrap()
        .0;
    assert_eq!(app.world().get::<Text>(title).unwrap().0, "系统设置");
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    app.update();
    assert_eq!(app.world().get::<Text>(title).unwrap().0, "Settings");
    for (entity, route) in &nav {
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(*entity)
                .unwrap()
                .label(),
            Some(Lang("en-US").tr(route.label_key()).as_ref())
        );
    }
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    app.update();
    assert_eq!(app.world().get::<Text>(title).unwrap().0, "系统设置");
    for (entity, route) in nav {
        assert_eq!(
            app.world()
                .get::<AccessibilityNode>(entity)
                .unwrap()
                .label(),
            Some(Lang("zh-CN").tr(route.label_key()).as_ref())
        );
    }
}

#[test]
fn localized_placeholders_reach_the_native_text_runs_in_the_same_update() {
    use crate::support::headless_plugins;
    use bevy::app::Update;
    use bevy::scene::{CommandsSceneExt, bsn};
    use infiltrator_bevy_widgets::palette::UiPalette;
    use infiltrator_bevy_widgets::text_input::render::sync_text_fields;
    use infiltrator_bevy_widgets::text_input::{
        TextFieldPlaceholder, text_field_with_placeholder_scene,
    };
    use infiltrator_bevy_widgets::theme::Theme;
    let mut app = App::new();
    headless_plugins(&mut app);
    let palette = UiPalette::new(&Theme::dark());
    app.insert_resource(palette);
    app.insert_resource(UiLocale::new("zh-CN"));
    app.add_plugins(LocalizationPlugin);
    app.add_systems(Update, sync_text_fields);
    let field = app
        .world_mut()
        .commands()
        .spawn_scene(bsn! {
            @{ text_field_with_placeholder_scene(String::new(), String::new(), &palette) }
            LocalizedPlaceholder::plain("connections_search_placeholder")
        })
        .id();
    app.update();
    let caption = app
        .world_mut()
        .query::<(Entity, &TextFieldPlaceholder)>()
        .single(app.world())
        .unwrap()
        .0;
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        "按域名/IP/进程即时搜索连接"
    );
    app.insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        "Search connections by domain, IP or process"
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.placeholder(),
        "Search connections by domain, IP or process"
    );
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "");
    app.insert_resource(UiLocale::new("zh-CN"));
    app.update();
    assert_eq!(
        app.world().get::<Text>(caption).unwrap().0,
        "按域名/IP/进程即时搜索连接"
    );
}

#[test]
fn changing_language_keeps_live_connection_phase_counts_and_native_entities() {
    use crate::support::headless_plugins;
    use infiltrator_bevy_ui::app::ShellPlugin;
    use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink};
    use infiltrator_bevy_ui::pages::connections::{ConnectionsLine, ConnectionsLineKind};
    use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
    use infiltrator_bevy_ui::surface::{LatestSurfaceSnapshot, SurfaceSnapshotUpdated};
    use std::sync::Arc;

    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(Arc::new(
        DemoCommandSink::accepting(),
    )));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Connections));
    app.update();
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.pages.settings.data.as_mut().unwrap().language = "en-US".into();
    snapshot.language_settings.preference = Some(LanguagePreference::English);
    snapshot
        .pages
        .connections
        .data
        .as_mut()
        .unwrap()
        .total_connections = 37;
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
    let lines = app
        .world_mut()
        .query::<(Entity, &ConnectionsLine, &Text)>()
        .iter(app.world())
        .map(|(entity, kind, text)| (kind.0, entity, text.0.clone()))
        .collect::<Vec<_>>();
    let (_, stream, value) = lines
        .iter()
        .find(|(kind, _, _)| *kind == ConnectionsLineKind::Stream)
        .unwrap();
    assert_eq!(value, Lang("en-US").tr("connections_stream_live").as_ref());
    let (_, summary, value) = lines
        .iter()
        .find(|(kind, _, _)| *kind == ConnectionsLineKind::Summary)
        .unwrap();
    assert!(value.contains("37"));
    let mut snapshot = app.world().resource::<LatestSurfaceSnapshot>().0.clone();
    snapshot.revision += 1;
    snapshot.pages.settings.data.as_mut().unwrap().language = "zh-CN".into();
    snapshot.language_settings.preference = Some(LanguagePreference::SimplifiedChinese);
    app.world_mut()
        .commands()
        .trigger(SurfaceSnapshotUpdated(snapshot));
    app.update();
    app.update();
    assert_eq!(
        app.world().get::<Text>(*stream).unwrap().0,
        Lang("zh-CN").tr("connections_stream_live")
    );
    assert!(app.world().get::<Text>(*summary).unwrap().0.contains("37"));
}
