//! The business panels of the overview workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_refused, assert_retried, business_surface,
};
use infiltrator_slint::surface::business::BusinessId;

/// The page that carries the overview businesses.
const PAGE: &str = "OverviewWorkbench";

#[test]
fn the_overview_reconnect_mask_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::OverviewReconnectMask);
}

#[test]
fn the_overview_reconnect_mask_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::OverviewReconnectMask);
}

#[test]
fn the_overview_reconnect_mask_retry_path() {
    let surface = business_surface(PAGE);

    assert_retried(&surface, BusinessId::OverviewReconnectMask);
}

#[test]
fn the_overview_topology_drilldown_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::OverviewTopologyDrilldown);
}

#[test]
fn the_overview_topology_drilldown_close_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::OverviewTopologyDrilldown);
}
