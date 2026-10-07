//! test-intent: behavior
use crate::command_harness::{RecordingHandler, recording_application};
use crate::support::headless_plugins;
use bevy::app::{App, Update};
use bevy::ecs::change_detection::{DetectChanges, Ref};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, ResMut};
use bevy::text::TextSpan;
use bevy::ui::widget::Text;
use bevy::ui::{Node, Val};
use bevy::ui_widgets::Activate;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::project_proxy_groups;
use infiltrator_application::proxy_search_projection::project_name_runs;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, UiCommand};
use infiltrator_bevy_ui::command_events::CommandExecutedEvent;
use infiltrator_bevy_ui::pages::proxies::{
    LastProxiesProjection, NodeNameText, ProxiesProjectionUpdated, ProxiesScrollArea,
    ProxyNodeButton,
};
use infiltrator_bevy_ui::pages::proxies_highlight::sync_name_highlights;
use infiltrator_bevy_ui::pages::proxies_search::{
    ClearProxySearch, ProxySearchInput, ProxySearchState, ProxySearchStatus, RetryProxySearch,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{DemoSurfaceSource, SurfaceSource, proxies_projection};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, TextFieldFocused};
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface_snapshot::ProxyGroupSnapshot;
use std::sync::Arc;
use std::thread::yield_now;
use std::time::{Duration, Instant};

fn entity<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query::<(Entity, &T)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0
}
fn setup() -> (
    App,
    Arc<CoreApplication>,
    ProxyPreferencesApplication,
    Vec<ProxyGroupSnapshot>,
) {
    let preferences = ProxyPreferencesApplication::new();
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    let application = Arc::new(application);
    let mut groups = DemoSurfaceSource::running()
        .surface_snapshot()
        .pages
        .proxies
        .data
        .unwrap()
        .groups;
    // Unicode here is actual opaque controller data in the isolated native search test.
    for group in &mut groups {
        for node in &mut group.proxies {
            if node.name.starts_with("HK-") {
                node.name = format!("🇭🇰 香港 {}", node.name);
            }
        }
        if group.current.starts_with("HK-") {
            group.current = format!("🇭🇰 香港 {}", group.current);
        }
    }
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::for_application(application.clone()));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    app.update();
    publish(&mut app, &groups, &preferences);
    (app, application, preferences, groups)
}
fn field(app: &mut App) -> Entity {
    let wrapper = entity::<ProxySearchInput>(app);
    app.world()
        .get::<Children>(wrapper)
        .unwrap()
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .unwrap()
}
fn publish(
    app: &mut App,
    groups: &[ProxyGroupSnapshot],
    preferences: &ProxyPreferencesApplication,
) {
    let prefs = preferences
        .preferences()
        .expect("shared preference state available");
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let page = snapshot.pages.proxies.data.as_mut().unwrap();
    page.search_query = prefs.search_query.clone();
    page.groups = project_proxy_groups(groups.to_vec(), &prefs);
    page.name_runs = project_name_runs(&page.groups, &prefs.search_query);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(proxies_projection(&snapshot)));
    app.update();
    app.update();
}
fn settle(app: &mut App) {
    app.update();
    let deadline = Instant::now() + Duration::from_secs(3);
    while app.world().resource::<ProxySearchState>().pending.is_some() {
        assert!(
            Instant::now() < deadline,
            "search must receive its real terminal result"
        );
        app.update();
        yield_now();
    }
}
fn edit(app: &mut App, field: Entity, query: &str) {
    app.world_mut()
        .get_mut::<TextField>(field)
        .unwrap()
        .0
        .apply(TextFieldInput::SetText(query.into()));
    app.update();
    settle(app);
}

#[test]
fn native_search_executes_shared_preferences_renders_highlights_and_clear_restores_nodes_without_losing_focus()
 {
    let (mut app, _, preferences, groups) = setup();
    let field = field(&mut app);
    app.world_mut()
        .get_mut::<TextFieldFocused>(field)
        .unwrap()
        .0 = true;
    let initial = app
        .world_mut()
        .query::<&ProxyNodeButton>()
        .iter(app.world())
        .count();
    edit(&mut app, field, "香港");
    assert_eq!(
        preferences
            .preferences()
            .expect("shared preference state available")
            .search_query,
        "香港"
    );
    publish(&mut app, &groups, &preferences);
    let nodes: Vec<_> = app
        .world_mut()
        .query::<&ProxyNodeButton>()
        .iter(app.world())
        .map(|node| node.node_name.clone())
        .collect();
    assert!(!nodes.is_empty());
    assert!(nodes.len() < initial);
    assert!(nodes.iter().all(|node| node.contains("香港")));
    assert!(
        app.world_mut()
            .query::<&TextSpan>()
            .iter(app.world())
            .any(|span| span.0 == "香港")
    );
    let name = entity::<NodeNameText>(&mut app);
    let mut rendered = app.world().get::<Text>(name).unwrap().0.clone();
    if let Some(children) = app.world().get::<Children>(name) {
        for child in children {
            if let Some(span) = app.world().get::<TextSpan>(*child) {
                rendered.push_str(&span.0);
            }
        }
    }
    assert!(
        nodes.contains(&rendered),
        "native text runs must preserve the complete observed name"
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    app.world_mut()
        .get_mut::<TextField>(field)
        .unwrap()
        .0
        .set_preedit("draft");
    app.world_mut().insert_resource(UiLocale::new("en-US"));
    app.update();
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.preedit(),
        "draft"
    );
    assert!(app.world().resource::<ProxySearchState>().pending.is_none());
    let mut refreshed = groups.clone();
    let matching = refreshed
        .iter_mut()
        .flat_map(|group| &mut group.proxies)
        .find(|node| node.name.contains("香港"))
        .unwrap();
    matching.delay_ms = Some(142);
    publish(&mut app, &refreshed, &preferences);
    assert!(
        app.world_mut()
            .query::<&Text>()
            .iter(app.world())
            .any(|text| text.0 == "142 ms")
    );
    assert_eq!(
        app.world().get::<TextField>(field).unwrap().0.text(),
        "香港"
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    edit(&mut app, field, "missing-node");
    publish(&mut app, &groups, &preferences);
    assert_eq!(
        app.world_mut()
            .query::<&ProxyNodeButton>()
            .iter(app.world())
            .count(),
        0
    );
    let status = entity::<ProxySearchStatus>(&mut app);
    assert_eq!(
        app.world().get::<Text>(status).unwrap().0,
        LocalizedText::plain("proxies_search_empty").render(app.world().resource::<UiLocale>())
    );
    let clear = entity::<ClearProxySearch>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: clear });
    app.update();
    settle(&mut app);
    publish(&mut app, &groups, &preferences);
    assert!(
        preferences
            .preferences()
            .expect("shared preference state available")
            .search_query
            .is_empty()
    );
    assert_eq!(
        app.world_mut()
            .query::<&ProxyNodeButton>()
            .iter(app.world())
            .count(),
        initial
    );
    assert!(app.world().get::<TextFieldFocused>(field).unwrap().0);
    assert_eq!(
        app.world()
            .resource::<LastProxiesProjection>()
            .0
            .as_ref()
            .unwrap()
            .groups[0]
            .current,
        groups[0].current,
        "search and clear preserve the actual observed controller selection"
    );
}

#[test]
fn native_search_preserves_rejected_drafts_ignores_stale_results_and_retries_through_the_real_owner()
 {
    let (mut app, application, preferences, groups) = setup();
    let rejected = Arc::new(RecordingHandler::default());
    let failure = Failure::new(ErrorCode::Permission, "search denied", false);
    *rejected.1.lock().unwrap() = Some(failure.clone());
    application.install_command_handler(rejected);
    let field = field(&mut app);
    edit(&mut app, field, "hk");
    assert_eq!(
        app.world().resource::<ProxySearchState>().failure,
        Some(failure)
    );
    assert_eq!(app.world().get::<TextField>(field).unwrap().0.text(), "hk");
    assert!(
        preferences
            .preferences()
            .expect("shared preference state available")
            .search_query
            .is_empty()
    );
    app.world_mut().commands().trigger(CommandExecutedEvent {
        request_id: RequestId(999999),
        command: UiCommand::SetProxySearchQuery {
            query: "old".into(),
        },
        result: Ok(CommandOutput::Unit),
    });
    app.update();
    assert!(app.world().resource::<ProxySearchState>().failure.is_some());
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_proxy_preferences(preferences.clone()),
    ));
    let retry = entity::<RetryProxySearch>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: retry });
    app.update();
    settle(&mut app);
    publish(&mut app, &groups, &preferences);
    assert_eq!(
        preferences
            .preferences()
            .expect("shared preference state available")
            .search_query,
        "hk"
    );
    assert!(app.world().resource::<ProxySearchState>().failure.is_none());
}

#[derive(Resource, Default)]
struct NameChanges(usize);
fn observe_name_changes(
    names: Query<Ref<Text>, With<NodeNameText>>,
    mut changes: ResMut<NameChanges>,
) {
    changes.0 += names.iter().filter(|text| text.is_changed()).count();
}

#[test]
fn idle_highlights_do_not_relayout_text_and_short_spacing_restores_without_replacing_the_input() {
    let (mut app, _, _, _) = setup();
    let input = field(&mut app);
    app.init_resource::<NameChanges>();
    app.add_systems(Update, observe_name_changes.after(sync_name_highlights));
    app.update();
    app.world_mut().resource_mut::<NameChanges>().0 = 0;
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<NameChanges>().0,
        0,
        "idle frames must not dirty every node's glyph layout"
    );
    let scroll = entity::<ProxiesScrollArea>(&mut app);
    for (height, gap) in [(480.0, 8.0), (780.0, 16.0)] {
        app.world_mut()
            .insert_resource(ResponsiveContext::new(720.0, height));
        app.update();
        assert_eq!(
            app.world().get::<Node>(scroll).unwrap().row_gap,
            Val::Px(gap)
        );
        assert_eq!(field(&mut app), input);
    }
}
