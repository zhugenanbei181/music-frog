//! The business panels of the rules workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_keeps_review, assert_locale, assert_pending,
    assert_refreshed, assert_refused, assert_retried, assert_review_open, assert_stable,
    business_surface, cell,
};
use infiltrator_slint::surface::business::BusinessId;
use infiltrator_slint::surface::harness::SurfaceOutcome;

/// The page that carries the rules businesses.
const PAGE: &str = "RulesWorkbench";

#[test]
fn the_rules_list_editor_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_list_editor_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_list_editor_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_list_editor_retry_path() {
    let surface = business_surface(PAGE);

    assert_retried(&surface, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_list_editor_refresh_path() {
    let surface = business_surface(PAGE);

    assert_refreshed(&surface, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_list_editor_stale_path() {
    let surface = business_surface(PAGE);

    assert_stable(&surface, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_list_editor_focus_path() {
    let surface = business_surface(PAGE);

    assert_keeps_review(&surface, PAGE, BusinessId::RulesListEditor);
}

#[test]
fn the_rules_override_editor_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::RulesOverrideEditor);
}

#[test]
fn the_rules_override_editor_cancel_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::RulesOverrideEditor);
}

#[test]
fn the_rules_override_editor_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::RulesOverrideEditor);
}

#[test]
fn the_rules_provider_refresh_error_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::RulesProviderRefreshError);
}

#[test]
fn the_rules_provider_refresh_error_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::RulesProviderRefreshError);
}

#[test]
fn the_rules_provider_refresh_error_retry_path() {
    let surface = business_surface(PAGE);

    assert_retried(&surface, BusinessId::RulesProviderRefreshError);
}

#[test]
fn the_rules_tracer_drawer_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::RulesTracerDrawer);
}

#[test]
fn the_rules_tracer_drawer_close_path() {
    let surface = business_surface(PAGE);

    assert_dismissed(&surface, BusinessId::RulesTracerDrawer);
}

#[test]
fn the_rules_tracer_drawer_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::RulesTracerDrawer);
}

#[test]
fn the_rules_statistics_inspector_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_cancel_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_retry_path() {
    let surface = business_surface(PAGE);
    assert_retried(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_refresh_path() {
    let surface = business_surface(PAGE);
    assert_refreshed(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_stale_path() {
    let surface = business_surface(PAGE);
    assert_stable(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_pending_path() {
    let surface = business_surface(PAGE);
    assert_pending(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_locale_path() {
    let surface = business_surface(PAGE);
    assert_locale(&surface, BusinessId::RulesStatisticsInspector);
}

#[test]
fn the_rules_statistics_inspector_unknown_path() {
    let surface = business_surface(PAGE);

    assert!(
        cell(&surface, "rules-statistics-inspector/empty").is_some(),
        "an unobserved hit audit must leave the inspector in its unknown state"
    );
}

#[test]
fn the_rules_statistics_inspector_cleanup_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::RulesStatisticsInspector);

    let outcome = fixture::confirm(&surface, BusinessId::RulesStatisticsInspector);

    match outcome {
        SurfaceOutcome::Dispatched(actions) => {
            let intent = actions
                .iter()
                .find_map(|action| match action {
                    infiltrator_slint::SlintAction::Command(intent) => Some(intent.clone()),
                    _ => None,
                })
                .expect("the cleanup dispatches one owner command");
            assert!(
                matches!(intent, ResetRuleHitCounters { .. }),
                "the cleanup must dispatch the shared counter reset"
            );
        }
        other => panic!("the cleanup must dispatch the shared command, got {other:?}"),
    }
}
