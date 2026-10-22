//! The business panels of the shell workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_locale, assert_refreshed, assert_refused,
    assert_retried, assert_stable, assert_unsupported, business_surface, cell,
};
use infiltrator_contract::command_catalogue::CommandCatalogue;
use infiltrator_slint::surface::business::BusinessId;
use infiltrator_slint::surface::harness::SurfaceHarness;

/// The page that carries the shell businesses.
const PAGE: &str = "ShellWorkbench";

#[test]
fn the_shell_command_palette_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::ShellCommandPalette);
}

#[test]
fn the_shell_command_palette_cancel_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::ShellCommandPalette);
}

#[test]
fn the_shell_command_palette_refresh_path() {
    let surface = business_surface(PAGE);
    assert_refreshed(&surface, BusinessId::ShellCommandPalette);
}

#[test]
fn the_shell_command_palette_locale_path() {
    let surface = business_surface(PAGE);
    assert_locale(&surface, BusinessId::ShellCommandPalette);
}

/// The command rows the palette shows.
fn palette_facts(surface: &SurfaceHarness) -> Vec<String> {
    surface
        .workbench_items()
        .into_iter()
        .filter(|item| item.id.starts_with("shell-command-palette/fact/"))
        .map(|item| item.detail.to_string())
        .collect()
}

#[test]
fn the_shell_command_palette_full_list_path() {
    let surface = business_surface(PAGE);

    fixture::open(&surface, BusinessId::ShellCommandPalette);

    assert_eq!(
        palette_facts(&surface).len(),
        CommandCatalogue::new().len(),
        "the palette must show every command the shared catalogue publishes"
    );
}

#[test]
fn the_shell_command_palette_empty_path() {
    let surface = business_surface(PAGE);
    surface.workbench_draft_edited("command-query", "no-command-matches-this");

    fixture::open(&surface, BusinessId::ShellCommandPalette);

    assert!(
        palette_facts(&surface).is_empty(),
        "a query that matches nothing must leave the palette empty"
    );
}

#[test]
fn the_shell_routing_intent_control_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_retry_path() {
    let surface = business_surface(PAGE);
    assert_retried(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_close_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_stale_path() {
    let surface = business_surface(PAGE);
    assert_stable(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_refresh_path() {
    let surface = business_surface(PAGE);
    assert_refreshed(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_pending_path() {
    let surface = business_surface(PAGE);
    fixture::assert_pending(&surface, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::ShellRoutingIntentControl);
}

#[test]
fn the_shell_routing_intent_control_guide_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::ShellRoutingIntentControl);

    assert!(
        cell(&surface, "shell-routing-intent-control/guide").is_some(),
        "the routing intent control must publish the shared guide"
    );
}

#[test]
fn the_shell_routing_intent_authentication_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::ShellRoutingIntentAuthentication);
}

#[test]
fn the_shell_routing_intent_authentication_close_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::ShellRoutingIntentAuthentication);
}

#[test]
fn the_shell_routing_intent_authentication_guide_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::ShellRoutingIntentAuthentication);

    assert!(
        cell(&surface, "shell-routing-intent-authentication/guide").is_some(),
        "the routing intent authentication must publish the shared guide"
    );
}
