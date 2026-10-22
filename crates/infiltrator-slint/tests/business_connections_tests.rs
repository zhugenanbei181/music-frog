//! The business panels of the connections workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{assert_accepted, assert_dismissed, assert_unsupported, business_surface};
use infiltrator_slint::surface::business::BusinessId;
use infiltrator_slint::surface::harness::SurfaceHarness;

/// The page that carries the connections businesses.
const PAGE: &str = "ConnectionsWorkbench";

#[test]
fn the_connections_close_all_confirm_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::ConnectionsCloseAllConfirm);
}

#[test]
fn the_connections_close_all_confirm_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::ConnectionsCloseAllConfirm);
}

#[test]
fn the_connections_grouping_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::ConnectionsGrouping);
}

#[test]
fn the_connections_details_drawer_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::ConnectionsDetailsDrawer);
}

#[test]
fn the_connections_details_drawer_close_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::ConnectionsDetailsDrawer);
}

#[test]
fn the_connections_details_drawer_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::ConnectionsDetailsDrawer);
}

/// The connection facts the drawer shows.
fn connection_facts(surface: &SurfaceHarness) -> Vec<String> {
    surface
        .workbench_items()
        .into_iter()
        .filter(|item| item.id.starts_with("connections-details-drawer/fact/"))
        .map(|item| item.detail.to_string())
        .collect()
}

#[test]
fn the_connections_details_drawer_reorder_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::ConnectionsDetailsDrawer);
    let before = connection_facts(&surface);

    let mut snapshot = fixture::business_snapshot();
    if let Some(page) = snapshot.pages.connections.data.as_mut() {
        page.connections.reverse();
    }
    surface.publish_snapshot(snapshot);
    fixture::open(&surface, BusinessId::ConnectionsDetailsDrawer);

    let after = connection_facts(&surface);
    assert_eq!(
        after.len(),
        before.len(),
        "the reordered list must keep every published connection"
    );
    assert_ne!(
        after, before,
        "the drawer must follow the order the owner published"
    );
}

#[test]
fn the_connections_details_drawer_removed_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::ConnectionsDetailsDrawer);
    let before = connection_facts(&surface).len();

    let mut snapshot = fixture::business_snapshot();
    if let Some(page) = snapshot.pages.connections.data.as_mut() {
        page.connections.pop();
    }
    surface.publish_snapshot(snapshot);
    fixture::open(&surface, BusinessId::ConnectionsDetailsDrawer);

    assert_eq!(
        connection_facts(&surface).len(),
        before - 1,
        "a removed connection must leave the drawer that shows them"
    );
}
