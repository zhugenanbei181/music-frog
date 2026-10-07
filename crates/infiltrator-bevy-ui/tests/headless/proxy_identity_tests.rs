//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::support::headless_plugins;
use bevy::app::App;
use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::math::Vec2;
use bevy::picking::backend::HitData;
use bevy::picking::events::{Pointer, PointerCancel, PointerClick, PointerPress};
use bevy::picking::pointer::{Location, PointerButton, PointerId};
use bevy::text::TextColor;
use bevy::ui::BackgroundColor;
use bevy::ui::widget::Text;
use bevy::ui::{Display, Node, Pressed};
use bevy::ui_widgets::{Activate, Button, ButtonPlugin};
use infiltrator_application::command_application::{
    CommandApplication, CommandFuture, CommandHandler,
};
use infiltrator_application::proxy_preferences_application::ProxyPreferencesApplication;
use infiltrator_application::proxy_projection::project_proxy_groups;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::CommandPumpPlugin;
use infiltrator_bevy_ui::command_execution::ApplicationCommandSink;
use infiltrator_bevy_ui::pages::proxies::{
    FilterAliveToggle, GroupNodesContainer, LastProxiesProjection, NodePinButton,
    ProxiesProjectionUpdated, ProxyGroupFoldButton, ProxyNodeButton, ToggleViewModeButton,
};
use infiltrator_bevy_ui::pages::proxies_identity::{ProxyCardsContainer, ProxyGroupIdentity};
use infiltrator_bevy_ui::pages::proxies_preferences::{
    FilterAliveIndicator, ProxyFavoriteStar, ProxyViewLabel,
};
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{DemoSurfaceSource, SurfaceSource, proxies_projection};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface_snapshot::{ProxyGroupSnapshot, ProxyNodeSnapshot};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::yield_now;
use std::time::{Duration, Instant};

#[derive(Resource, Clone, Default)]
struct PreferenceCommands(Arc<Mutex<Vec<CommandIntent>>>);
struct TrackedPreferences {
    application: CommandApplication,
    commands: PreferenceCommands,
}
impl CommandHandler for TrackedPreferences {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        self.commands.0.lock().unwrap().push(intent.clone());
        self.application.handle(intent)
    }
}

fn app_with_preferences() -> (App, ProxyPreferencesApplication) {
    let preferences = ProxyPreferencesApplication::new();
    let (application, _) = recording_application();
    let commands = PreferenceCommands::default();
    application.install_command_handler(Arc::new(TrackedPreferences {
        application: CommandApplication::new().with_proxy_preferences(preferences.clone()),
        commands: commands.clone(),
    }));
    let mut app = App::new();
    app.insert_resource(commands);
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        ThemePreference::Fixed(ThemeSkin::Dark),
        1180.0,
    ));
    app.add_plugins(PagesPlugin::demo());
    app.add_plugins(CommandPumpPlugin::new(Arc::new(
        ApplicationCommandSink::new(Arc::new(application)),
    )));
    app.update();
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    (app, preferences)
}

fn publish(
    app: &mut App,
    groups: Vec<ProxyGroupSnapshot>,
    preferences: &ProxyPreferencesApplication,
) {
    let mut snapshot = DemoSurfaceSource::running().surface_snapshot();
    let page = snapshot.pages.proxies.data.as_mut().unwrap();
    let prefs = preferences
        .preferences()
        .expect("shared preference state available");
    page.groups = project_proxy_groups(groups, &prefs);
    page.filter_alive.enabled = prefs.filter_alive;
    page.compact_view = prefs.compact_view;
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(proxies_projection(&snapshot)));
    app.update();
}

fn nodes(app: &mut App) -> HashMap<(String, String), Entity> {
    app.world_mut()
        .query::<(Entity, &ProxyNodeButton)>()
        .iter(app.world())
        .map(|(entity, node)| ((node.group_name.clone(), node.node_name.clone()), entity))
        .collect()
}

fn visible(app: &mut App, group: &str) -> bool {
    app.world_mut()
        .query::<(&Node, &GroupNodesContainer, &ProxyGroupIdentity)>()
        .iter(app.world())
        .find(|(_, _, identity)| identity.0 == group)
        .unwrap()
        .0
        .display
        == Display::Flex
}

#[test]
fn native_fold_round_trip_executes_the_shared_preference_command_and_preserves_nodes() {
    let (mut app, preferences) = app_with_preferences();
    let groups = DemoSurfaceSource::running()
        .surface_snapshot()
        .pages
        .proxies
        .data
        .unwrap()
        .groups;
    let group = groups[0].name.clone();
    let originals = nodes(&mut app);
    let button = app
        .world_mut()
        .query::<(Entity, &ProxyGroupFoldButton)>()
        .iter(app.world())
        .find(|(_, button)| button.group_name == group)
        .unwrap()
        .0;
    for expected in [false, true] {
        app.world_mut()
            .commands()
            .trigger(Activate { entity: button });
        app.update();
        let deadline = Instant::now() + Duration::from_secs(2);
        while preferences.is_group_expanded(&group).unwrap() != expected {
            assert!(
                Instant::now() < deadline,
                "native fold command must complete"
            );
            yield_now();
        }
        publish(&mut app, groups.clone(), &preferences);
        assert_eq!(visible(&mut app, &group), expected);
        assert_eq!(nodes(&mut app), originals);
    }
}

fn find_group(app: &App, entity: Entity) -> Option<String> {
    if let Some(identity) = app.world().get::<ProxyGroupIdentity>(entity) {
        return Some(identity.0.clone());
    }
    app.world()
        .get::<Children>(entity)?
        .iter()
        .find_map(|child| find_group(app, *child))
}

#[test]
fn native_alive_and_compact_controls_can_enable_and_disable_the_observed_preferences() {
    let (mut app, preferences) = app_with_preferences();
    let groups = DemoSurfaceSource::running()
        .surface_snapshot()
        .pages
        .proxies
        .data
        .unwrap()
        .groups;
    let filter = app
        .world_mut()
        .query::<(Entity, &FilterAliveToggle)>()
        .single(app.world())
        .unwrap()
        .0;
    let compact = app
        .world_mut()
        .query::<(Entity, &ToggleViewModeButton)>()
        .single(app.world())
        .unwrap()
        .0;
    let indicator = app
        .world_mut()
        .query::<(Entity, &FilterAliveIndicator)>()
        .single(app.world())
        .unwrap()
        .0;
    let label = app
        .world_mut()
        .query::<(Entity, &ProxyViewLabel)>()
        .single(app.world())
        .unwrap()
        .0;
    for expected in [true, false] {
        app.world_mut()
            .commands()
            .trigger(Activate { entity: filter });
        app.world_mut()
            .commands()
            .trigger(Activate { entity: compact });
        app.update();
        let deadline = Instant::now() + Duration::from_secs(2);
        while preferences.filter_alive().unwrap() != expected
            || preferences.compact_view().unwrap() != expected
        {
            assert!(
                Instant::now() < deadline,
                "native preference commands must complete"
            );
            app.update();
            yield_now();
        }
        publish(&mut app, groups.clone(), &preferences);
        let palette = app.world().resource::<UiPalette>();
        let color = if expected {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
        assert_eq!(
            app.world().get::<BackgroundColor>(indicator).unwrap().0,
            color
        );
        assert_eq!(
            app.world().get::<Text>(label).unwrap().0,
            if expected {
                "紧凑列表"
            } else {
                "网格视图"
            }
        );
        assert!(app.world().get::<FilterAliveToggle>(filter).is_some());
        assert!(app.world().get::<ToggleViewModeButton>(compact).is_some());
    }
}

#[test]
fn reordered_and_changed_group_lists_keep_live_identities_and_reject_removed_controls() {
    let (mut app, preferences) = app_with_preferences();
    let mut groups = DemoSurfaceSource::running()
        .surface_snapshot()
        .pages
        .proxies
        .data
        .unwrap()
        .groups;
    let original_nodes = nodes(&mut app);
    let group = groups[0].name.clone();
    let old_button = app
        .world_mut()
        .query::<(Entity, &ProxyGroupFoldButton)>()
        .iter(app.world())
        .find(|(_, button)| button.group_name == group)
        .unwrap()
        .0;
    preferences
        .reorder_groups(
            groups
                .iter()
                .rev()
                .map(|group| group.name.clone())
                .collect(),
        )
        .unwrap();
    publish(&mut app, groups.clone(), &preferences);
    assert_eq!(nodes(&mut app), original_nodes);
    let container = app
        .world_mut()
        .query::<(Entity, &ProxyCardsContainer)>()
        .iter(app.world())
        .next()
        .unwrap()
        .0;
    let order = app
        .world()
        .get::<Children>(container)
        .unwrap()
        .iter()
        .filter_map(|entity| find_group(&app, *entity))
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        groups
            .iter()
            .rev()
            .map(|group| group.name.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        app.world()
            .get::<ProxyGroupFoldButton>(old_button)
            .unwrap()
            .group_name,
        group
    );
    groups[0].proxies.push(ProxyNodeSnapshot {
        name: "new-live-node".into(),
        node_type: "VLESS".into(),
        delay_ms: Some(10),
        alive: Some(true),
        selected: false,
        favorite: false,
        features: vec![],
    });
    publish(&mut app, groups.clone(), &preferences);
    let updated = nodes(&mut app);
    for (key, entity) in original_nodes {
        assert_eq!(updated[&key], entity);
    }
    assert!(updated.contains_key(&(group.clone(), "new-live-node".into())));
    let surviving = nodes(&mut app);
    let mut added = groups[0].clone();
    added.name = "newly-published-group".into();
    let added_name = added.name.clone();
    preferences.set_group_expanded(&group, false).unwrap();
    groups.push(added);
    publish(&mut app, groups.clone(), &preferences);
    assert!(visible(&mut app, &added_name));
    assert!(!visible(&mut app, &group));
    let updated = nodes(&mut app);
    for (key, entity) in surviving {
        assert_eq!(updated[&key], entity);
    }
    assert!(updated.keys().any(|(name, _)| name == &added_name));
    preferences.set_group_expanded(&group, true).unwrap();
    groups.remove(0);
    publish(&mut app, groups, &preferences);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: old_button });
    app.update();
    assert!(preferences.is_group_expanded(&group).unwrap());
    assert!(!nodes(&mut app).keys().any(|(name, _)| name == &group));
}

fn pointer() -> Pointer {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::None {
                width: 1180,
                height: 780,
            },
            position: Vec2::ZERO,
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
fn favorite_star(app: &App, entity: Entity) -> Option<Entity> {
    if app.world().get::<ProxyFavoriteStar>(entity).is_some() {
        return Some(entity);
    }
    app.world()
        .get::<Children>(entity)?
        .iter()
        .copied()
        .find_map(|child| favorite_star(app, child))
}

#[test]
fn real_pointer_pin_click_and_cancel_never_select_the_node_and_favorite_ink_replays_in_place() {
    let (mut app, preferences) = app_with_preferences();
    app.add_plugins(ButtonPlugin);
    let groups = DemoSurfaceSource::running()
        .surface_snapshot()
        .pages
        .proxies
        .data
        .unwrap()
        .groups;
    publish(&mut app, groups.clone(), &preferences);
    let (pin, name) = app
        .world_mut()
        .query::<(Entity, &NodePinButton, &Text)>()
        .iter(app.world())
        .next()
        .map(|(entity, pin, _)| (entity, pin.node_name.clone()))
        .unwrap();
    assert!(
        app.world().get::<Button>(pin).is_some(),
        "the icon is born with actual native button behavior"
    );
    let original_nodes = nodes(&mut app);
    let node_entity = *original_nodes
        .iter()
        .find(|((_, node), _)| node == &name)
        .unwrap()
        .1;
    let star = favorite_star(&app, node_entity).unwrap();
    assert!(!preferences.is_favorite(&name).unwrap());
    press(&mut app, pin);
    assert!(app.world().get::<Pressed>(pin).is_some());
    assert!(
        original_nodes
            .values()
            .all(|node| app.world().get::<Pressed>(*node).is_none()),
        "press propagation stops at the pin"
    );
    app.world_mut().trigger(PointerCancel {
        entity: pin,
        pointer: pointer(),
        hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
    });
    app.world_mut().flush();
    assert!(app.world().get::<Pressed>(pin).is_none());
    assert!(!preferences.is_favorite(&name).unwrap());
    for favorite in [true, false] {
        press(&mut app, pin);
        app.world_mut().trigger(PointerClick {
            entity: pin,
            pointer: pointer(),
            button: PointerButton::Primary,
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
            duration: Duration::from_millis(120),
            count: 1,
        });
        app.world_mut().flush();
        app.world_mut().trigger(PointerCancel {
            entity: pin,
            pointer: pointer(),
            hit: HitData::new(Entity::PLACEHOLDER, 0.0, None, None),
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while preferences.is_favorite(&name).unwrap() != favorite {
            assert!(
                Instant::now() < deadline,
                "real favorite command must complete"
            );
            app.update();
            yield_now();
        }
        publish(&mut app, groups.clone(), &preferences);
        app.update();
        assert_eq!(nodes(&mut app), original_nodes);
        assert_eq!(
            app.world().get::<Text>(star).unwrap().0,
            if favorite { "★" } else { "☆" }
        );
        let palette = app.world().resource::<UiPalette>();
        assert_eq!(
            app.world().get::<TextColor>(pin).unwrap().0,
            if favorite {
                palette.warning
            } else {
                palette.ink_dim
            }
        );
        let projection = app
            .world()
            .resource::<LastProxiesProjection>()
            .0
            .as_ref()
            .unwrap();
        assert!(projection.groups.iter().all(|current| {
            groups
                .iter()
                .any(|initial| initial.name == current.name && initial.current == current.current)
        }));
        let recorded = app
            .world()
            .resource::<PreferenceCommands>()
            .0
            .lock()
            .unwrap()
            .clone();
        assert!(recorded.iter().all(|intent| matches!(intent, CommandIntent::ToggleFavoriteProxy { proxy } if proxy == &name)), "pin clicks cannot submit node selection");
    }
}
