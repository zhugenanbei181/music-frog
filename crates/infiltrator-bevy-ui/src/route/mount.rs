//! Route mounting consumes only the accepted render cache and the state needed for one subtree.
use super::{
    ActiveRoute, Route, RouteChanged, RouteHistory, SurfaceSourceHandle,
    trigger_page_projection_events,
};
use crate::app::ContentSlot;
use crate::app::ShellLayoutState;
use crate::history::TrafficHistory;
use crate::pages::about::{about_page, disclaimer_page, licenses_page, tools_page};
use crate::pages::app_routing::app_routing_page;
use crate::pages::connections::connections_page;
use crate::pages::dns::dns_page;
use crate::pages::doctor::doctor_page;
use crate::pages::logs::logs_page;
use crate::pages::overview::overview_page;
use crate::pages::profiles::profiles_page;
use crate::pages::proxies::scene::proxies_page;
use crate::pages::requests::{RequestFeed, requests_page};
use crate::pages::rules::rules_page;
use crate::pages::settings::settings_page;
use crate::pages::sync::sync_page;
use crate::surface::LatestSurfaceSnapshot;
use crate::surface::{
    app_routing_projection, connections_projection, dns_projection, doctor_projection,
    logs_projection, overview_projection, profiles_projection, proxies_projection,
    rules_projection, settings_projection, sync_projection,
};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{CommandsSceneExt, Scene};
use infiltrator_bevy_widgets::motion::{Spring, SpringRouteTransition};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_presentation::motion_projection::is_headless_capture_active;
use infiltrator_shared::locales::resolve_language_code;

#[derive(SystemParam)]
pub(crate) struct RouteMount<'w, 's> {
    slots: Query<'w, 's, Entity, With<ContentSlot>>,
    active: Option<Res<'w, ActiveRoute>>,
    history: Option<ResMut<'w, RouteHistory>>,
    palette: Res<'w, UiPalette>,
    source: Res<'w, SurfaceSourceHandle>,
    history_ring: Res<'w, TrafficHistory>,
    latest: Option<Res<'w, LatestSurfaceSnapshot>>,
    layout: Option<Res<'w, ShellLayoutState>>,
}

pub(crate) fn sync_route(trigger: On<RouteChanged>, mut mount: RouteMount, mut commands: Commands) {
    let route = trigger.0;
    if mount.active.is_some_and(|mounted| mounted.0 == Some(route)) {
        return;
    }
    let Ok(slot) = mount.slots.single() else {
        return;
    };
    if let Some(ref mut history) = mount.history {
        history.push(route);
    }
    let snapshot = mount.latest.as_ref().map_or_else(
        || mount.source.0.surface_snapshot(),
        |latest| latest.0.clone(),
    );
    let shell = mount
        .layout
        .as_deref()
        .copied()
        .unwrap_or_else(ShellLayoutState::default);
    let viewport = ResponsiveViewportSnapshot::from_dimensions(shell.width_px, 760.0);
    let mut projection = overview_projection(&snapshot);
    projection.viewport = viewport.clone();
    let language = snapshot
        .language_settings
        .preference
        .map(|preference| resolve_language_code(preference.as_setting()))
        .unwrap_or_else(|| {
            resolve_language_code(&snapshot.pages.settings.data.as_ref().map_or_else(
                || "system".to_string(),
                |settings| settings.language.clone(),
            ))
        });
    let scene = match route {
        Route::Overview => Box::new(overview_page(
            &projection,
            &mount.history_ring,
            &mount.palette,
            &language,
        )),
        _ => page_scene(
            route,
            &snapshot,
            &viewport,
            &mount.history_ring,
            &mount.palette,
            &language,
        ),
    };
    commands.entity(slot).despawn_children();
    let transition = if is_headless_capture_active() {
        SpringRouteTransition {
            translation_y: Spring::new(0.0, 300.0, 24.0),
            is_active: false,
        }
    } else {
        SpringRouteTransition::default()
    };
    commands
        .spawn_scene(scene)
        .insert((ChildOf(slot), transition));
    // Spawn is committed before plugin-installed projection observers run.
    trigger_page_projection_events(&snapshot, &mut commands);
    commands.insert_resource(ActiveRoute(Some(route)));
}

/// The route → scene table. The only place that knows which page backs
/// which route.
pub(crate) fn page_scene(
    route: Route,
    snapshot: &SurfaceSnapshot,
    viewport: &ResponsiveViewportSnapshot,
    history: &TrafficHistory,
    palette: &UiPalette,
    language: &str,
) -> Box<dyn Scene> {
    let projection = overview_projection(snapshot);
    match route {
        Route::Overview => Box::new(overview_page(&projection, history, palette, language)),
        Route::Proxies => Box::new(proxies_page(
            &proxies_projection(snapshot),
            viewport,
            palette,
        )),
        Route::Profiles => Box::new(profiles_page(
            &profiles_projection(snapshot),
            viewport,
            palette,
        )),
        Route::Rules => Box::new(rules_page(&rules_projection(snapshot), palette)),
        Route::Connections => {
            Box::new(connections_page(&connections_projection(snapshot), palette))
        }
        Route::Requests => Box::new(requests_page(
            &snapshot.requests,
            RequestFeed::default(),
            palette,
        )),
        Route::Logs => Box::new(logs_page(&logs_projection(snapshot), palette)),
        Route::Dns => Box::new(dns_page(&dns_projection(snapshot), palette)),
        Route::Doctor => Box::new(doctor_page(&doctor_projection(snapshot), palette)),
        Route::AppRouting => Box::new(app_routing_page(&app_routing_projection(snapshot), palette)),
        Route::Sync => Box::new(sync_page(&sync_projection(snapshot), palette)),
        Route::Settings => Box::new(settings_page(&settings_projection(snapshot), palette)),
        Route::Tools => Box::new(tools_page(palette)),
        Route::About => Box::new(about_page(palette)),
        Route::Disclaimer => Box::new(disclaimer_page(palette)),
        Route::Licenses => Box::new(licenses_page(palette)),
    }
}
