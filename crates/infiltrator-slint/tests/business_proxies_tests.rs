//! The business panels of the proxies workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_refreshed, assert_refused, assert_retried,
    assert_unsupported, business_surface,
};
use infiltrator_slint::surface::business::BusinessId;
use infiltrator_slint::surface::business::BusinessPhase;
use infiltrator_slint::surface::harness::SurfaceHarness;

/// The page that carries the proxies businesses.
const PAGE: &str = "ProxiesWorkbench";

#[test]
fn the_proxies_custom_node_modal_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::ProxiesCustomNodeModal);
}

#[test]
fn the_proxies_custom_node_modal_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::ProxiesCustomNodeModal);
}

#[test]
fn the_proxies_custom_node_modal_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::ProxiesCustomNodeModal);
}

#[test]
fn the_proxies_group_reorder_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::ProxiesGroupReorder);
}

#[test]
fn the_proxies_group_reorder_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::ProxiesGroupReorder);
}

#[test]
fn the_proxies_group_reorder_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::ProxiesGroupReorder);
}

#[test]
fn the_proxies_group_reorder_retry_path() {
    let surface = business_surface(PAGE);

    assert_retried(&surface, BusinessId::ProxiesGroupReorder);
}

#[test]
fn the_proxies_group_reorder_refresh_path() {
    let surface = business_surface(PAGE);

    assert_refreshed(&surface, BusinessId::ProxiesGroupReorder);
}

#[test]
fn the_proxies_probe_settings_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::ProxiesProbeSettings);
}

#[test]
fn the_proxies_probe_settings_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::ProxiesProbeSettings);
}

#[test]
fn the_proxies_probe_settings_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::ProxiesProbeSettings);
}

#[test]
fn the_proxies_probe_settings_retry_path() {
    let surface = business_surface(PAGE);

    assert_retried(&surface, BusinessId::ProxiesProbeSettings);
}

#[test]
fn the_proxies_probe_settings_refresh_path() {
    let surface = business_surface(PAGE);

    assert_refreshed(&surface, BusinessId::ProxiesProbeSettings);
}

#[test]
fn the_proxies_probe_settings_custom_timeout_and_url_path() {
    use infiltrator_contract::command::CommandIntent;
    use infiltrator_slint::SlintAction;
    use infiltrator_slint::surface::action::workbench::business::confirm_action;
    use infiltrator_slint::surface::harness::SurfaceOutcome;

    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::ProxiesProbeSettings);
    surface.set_workbench_draft("https://probe.example.test/204|9000");
    let outcome = surface.workbench_action(&confirm_action(BusinessId::ProxiesProbeSettings), "");
    let SurfaceOutcome::Dispatched(actions) = outcome else {
        panic!("expected dispatched actions");
    };
    assert_eq!(actions.len(), 1);
    let SlintAction::Command(CommandIntent::SetRuntimeProbeSettings { settings }) = &actions[0]
    else {
        panic!("expected SetRuntimeProbeSettings command");
    };
    assert_eq!(settings.url(), "https://probe.example.test/204");
    assert_eq!(settings.timeout_ms(), 9000);

    surface.publish_owner_success();
    assert_eq!(
        fixture::phase(&surface, BusinessId::ProxiesProbeSettings),
        BusinessPhase::Accepted
    );
}

#[test]
fn the_proxies_uri_import_preview_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::ProxiesUriImportPreview);
}

#[test]
fn the_proxies_uri_import_preview_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::ProxiesUriImportPreview);
}

#[test]
fn the_proxies_uri_import_preview_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::ProxiesUriImportPreview);
}

#[test]
fn the_proxies_group_expanded_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::ProxiesGroupExpanded);
}

#[test]
fn the_proxies_group_expanded_close_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::ProxiesGroupExpanded);
}

/// The group identities the expansion panel shows.
fn group_facts(surface: &SurfaceHarness) -> Vec<String> {
    surface
        .workbench_items()
        .into_iter()
        .filter(|item| item.id.starts_with("proxies-group-expanded/fact/"))
        .map(|item| item.detail.to_string())
        .collect()
}

#[test]
fn the_proxies_group_expanded_reorder_path() {
    let surface = business_surface(PAGE);
    surface.publish_snapshot(fixture::inventory_snapshot(&["AUTO", "PROXIES"]));
    fixture::open(&surface, BusinessId::ProxiesGroupExpanded);

    assert_eq!(
        group_facts(&surface),
        vec!["AUTO".to_owned(), "PROXIES".to_owned()],
        "the panel must show the group order the owner published"
    );
}

#[test]
fn the_proxies_group_expanded_added_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::ProxiesGroupExpanded);
    let before = group_facts(&surface).len();

    surface.publish_snapshot(fixture::inventory_snapshot(&["PROXIES", "AUTO"]));
    fixture::open(&surface, BusinessId::ProxiesGroupExpanded);

    assert_eq!(
        group_facts(&surface).len(),
        before + 1,
        "a published group must reach the panel that shows the groups"
    );
}

#[test]
fn the_proxies_group_expanded_removed_path() {
    let surface = business_surface(PAGE);
    surface.publish_snapshot(fixture::inventory_snapshot(&["PROXIES", "AUTO"]));
    fixture::open(&surface, BusinessId::ProxiesGroupExpanded);
    let before = group_facts(&surface).len();

    surface.publish_snapshot(fixture::inventory_snapshot(&["PROXIES"]));
    fixture::open(&surface, BusinessId::ProxiesGroupExpanded);

    assert_eq!(
        group_facts(&surface).len(),
        before - 1,
        "a removed group must leave the panel that shows the groups"
    );
}

#[test]
fn the_proxies_node_detail_drawer_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::ProxiesNodeDetailDrawer);
}

#[test]
fn the_proxies_node_detail_drawer_close_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::ProxiesNodeDetailDrawer);
}

#[test]
fn the_proxies_node_detail_drawer_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::ProxiesNodeDetailDrawer);
}

#[test]
fn the_proxies_node_detail_drawer_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::ProxiesNodeDetailDrawer);
}

#[test]
fn the_proxies_node_detail_drawer_retry_path() {
    let surface = business_surface(PAGE);
    assert_retried(&surface, BusinessId::ProxiesNodeDetailDrawer);
}

/// The node facts the drawer shows.
fn node_facts(surface: &SurfaceHarness) -> Vec<String> {
    surface
        .workbench_items()
        .into_iter()
        .filter(|item| item.id.starts_with("proxies-node-detail-drawer/fact/"))
        .map(|item| item.detail.to_string())
        .collect()
}

#[test]
fn the_proxies_node_detail_drawer_reorder_path() {
    let surface = business_surface(PAGE);
    surface.publish_snapshot(fixture::inventory_nodes_snapshot(&[
        ("HK-01", "HK IPLC 01"),
        ("JP-01", "Tokyo 01"),
    ]));
    fixture::open(&surface, BusinessId::ProxiesNodeDetailDrawer);
    let before = node_facts(&surface);

    surface.publish_snapshot(fixture::inventory_nodes_snapshot(&[
        ("JP-01", "Tokyo 01"),
        ("HK-01", "HK IPLC 01"),
    ]));
    fixture::open(&surface, BusinessId::ProxiesNodeDetailDrawer);

    let after = node_facts(&surface);
    assert_eq!(
        after.len(),
        before.len(),
        "the reordered list must keep every published node"
    );
    assert_ne!(
        after, before,
        "the drawer must follow the order the owner published"
    );
}

#[test]
fn the_proxies_node_detail_drawer_removed_path() {
    let surface = business_surface(PAGE);
    surface.publish_snapshot(fixture::inventory_nodes_snapshot(&[
        ("HK-01", "HK IPLC 01"),
        ("JP-01", "Tokyo 01"),
    ]));
    fixture::open(&surface, BusinessId::ProxiesNodeDetailDrawer);
    let before = node_facts(&surface).len();

    surface.publish_snapshot(fixture::inventory_nodes_snapshot(&[(
        "HK-01",
        "HK IPLC 01",
    )]));
    fixture::open(&surface, BusinessId::ProxiesNodeDetailDrawer);

    assert_eq!(
        node_facts(&surface).len(),
        before - 1,
        "a removed node must leave the drawer that shows them"
    );
}
