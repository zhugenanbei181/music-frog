//! What the desktop sidebar itself decides: the routes its rows resolve to and
//! the shapes it mounts per responsive tier.
//!
//! Every fact the pane used to duplicate has its own owner test now - the
//! master switches in `overview_master_switches_tests`, the live rates in
//! `view_chrome_tests`, the folded-page index in `view_settings_tests`, the
//! readout projections in `infiltrator_presentation` - so this file does not
//! restate them.

use super::*;
use infiltrator_contract::command_catalogue::ShellPage;
use infiltrator_contract::responsive_viewport::SidebarForm;
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::shell_navigation::{NavigationTarget, desktop_items};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::system_toggle::SystemToggleState;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::routing_intent::RoutingIntent;
use lurker_core_contract::routing_intent::RoutingIntentSnapshot;
use lurker_core_contract::routing_intent::RoutingIntentStatus;

#[test]
fn shared_observation_replay_keeps_intent_typed_and_preserves_failed_tun_readback() {
    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::unsupported("fixture has no controller read yet"),
    );
    snapshot.core.lifecycle = CoreLifecycle::Running;
    snapshot.core.routing_intent = RoutingIntentSnapshot::default();
    snapshot.revision = 1;
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(state.runtime.routing_intent, None);
    assert_eq!(state.runtime.system_toggles.tun, SystemToggleState::Unknown);
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Ready,
        tun_enabled: Some(false),
        ..Default::default()
    };
    snapshot.core.routing_intent = RoutingIntentSnapshot {
        current: Some(RoutingIntent::RouteAllThroughProxy),
        status: RoutingIntentStatus::Ready,
        failure: None,
    };
    snapshot.revision = 2;
    assert!(state.apply_shared_surface_snapshot(snapshot.clone()));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Disabled
    );
    let denied = Failure::new(ErrorCode::Authentication, "controller denied", false);
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Failed {
            failure: denied.clone(),
        },
        tun_enabled: Some(false),
        ..Default::default()
    };
    snapshot.revision = 3;
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert_eq!(
        state.runtime.routing_intent.as_ref(),
        Some(&RoutingIntent::RouteAllThroughProxy)
    );
    assert_eq!(
        state.runtime.system_toggles.tun,
        SystemToggleState::Failed { failure: denied }
    );
    assert!(!state.runtime.system_toggles.tun.can_toggle());
    assert_eq!(state.runtime.runtime_control.tun_enabled, Some(false));
}

/// Every row the pane paints is a registry destination, and every one of them
/// resolves onto a route this surface can open.
#[test]
fn every_registry_row_resolves_onto_a_route_of_this_surface() {
    let rows = desktop_items(SurfaceKind::IcedDesktop);
    assert_eq!(rows.len(), 8, "the desktop registry mounts eight rows");
    for item in &rows {
        match item.target {
            NavigationTarget::Shared(page) => assert_eq!(
                route_for_destination(item.target),
                Route::from_shell_page(page)
            ),
            NavigationTarget::SurfaceLocal("traffic") => assert_eq!(
                route_for_destination(item.target),
                Route::Traffic,
                "the traffic row opens its standalone route"
            ),
            NavigationTarget::SurfaceLocal(other) => {
                panic!("the Iced pane owns no surface-local row, got {other}")
            }
        }
    }
}

#[test]
fn each_route_selects_the_contract_owner_and_opens_its_actual_sidebar_route() {
    let rows = desktop_items(SurfaceKind::IcedDesktop);
    for (current_route, expected_route) in [
        (Route::Runtime, Route::Settings),
        (Route::Traffic, Route::Traffic),
        (Route::Connections, Route::Connections),
        (Route::Logs, Route::Logs),
    ] {
        let selected_route = selected_navigation_route(current_route);
        assert_eq!(
            selected_route, expected_route,
            "the shared navigation contract owns the selected row"
        );
        let selected = rows
            .iter()
            .filter(|item| route_for_destination(item.target) == selected_route)
            .count();
        assert_eq!(
            selected, 1,
            "each route resolves to exactly one selected row: {current_route:?}"
        );
        assert_eq!(
            selected_navigation_destination_count(current_route),
            selected,
            "capture readiness counts the row the sidebar actually paints"
        );
    }
}

#[test]
fn runtime_and_traffic_have_distinct_contract_navigation_owners() {
    assert_eq!(
        Route::Runtime.navigation_target(),
        NavigationTarget::Shared(ShellPage::Settings)
    );
    assert_eq!(
        Route::Traffic.navigation_target(),
        NavigationTarget::SurfaceLocal("traffic")
    );
    assert_ne!(
        selected_navigation_route(Route::Runtime),
        selected_navigation_route(Route::Traffic)
    );
}

#[test]
fn every_iced_route_resolves_to_exactly_one_mounted_navigation_row() {
    for route in [
        Route::Overview,
        Route::Profiles,
        Route::Proxies,
        Route::Runtime,
        Route::Traffic,
        Route::Connections,
        Route::Logs,
        Route::Rules,
        Route::Dns,
        Route::Sync,
        Route::Editor,
        Route::Settings,
        Route::AppRouting,
        Route::Doctor,
        Route::Tools,
        Route::About,
        Route::Disclaimer,
        Route::Licenses,
    ] {
        let selected_route = selected_navigation_route(route);
        let painted_rows = desktop_items(SurfaceKind::IcedDesktop)
            .iter()
            .filter(|item| route_for_destination(item.target) == selected_route)
            .count();
        assert_eq!(
            painted_rows, 1,
            "{route:?} must resolve through the contract to one mounted row"
        );
        assert_eq!(selected_navigation_destination_count(route), painted_rows);
    }
}

/// The two morphologies of one pane read the same rows, so a destination
/// cannot exist in the rail and be missing from the labelled column.
#[test]
fn the_labelled_column_and_the_rail_share_one_row_set() {
    let (state, _) = AppState::new();
    assert_eq!(
        sidebar_rows().len(),
        desktop_items(SurfaceKind::IcedDesktop).len()
    );
    let _labelled = sidebar(&state);
    let _rail = sidebar_rail(&state);
}

#[test]
fn test_sidebar_rail_render_smoke() {
    let (state, _) = AppState::new();
    let _rail = sidebar_rail(&state);
    assert_eq!(SidebarForm::Rail.width_px(), Some(64));
}
