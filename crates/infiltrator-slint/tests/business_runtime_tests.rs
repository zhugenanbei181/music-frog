//! The business panels of the runtime workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_empty, assert_locale, assert_refreshed,
    assert_refused, assert_stable, assert_unsupported, business_surface, cell,
};
use infiltrator_contract::shell_readout::ShellObservation;
use infiltrator_slint::surface::business::BusinessId;

/// The page that carries the runtime businesses.
const PAGE: &str = "RuntimeWorkbench";

#[test]
fn the_speedtest_progress_cancel_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SpeedtestProgressCancel);
}

#[test]
fn the_speedtest_progress_cancel_cancel_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::SpeedtestProgressCancel);
}

#[test]
fn the_speedtest_progress_cancel_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::SpeedtestProgressCancel);
}

#[test]
fn the_speedtest_details_modal_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SpeedtestDetailsModal);
}

#[test]
fn the_speedtest_details_modal_close_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::SpeedtestDetailsModal);
}

#[test]
fn the_speedtest_details_modal_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::SpeedtestDetailsModal);
}

#[test]
fn the_speedtest_history_comparison_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SpeedtestHistoryComparison);
}

#[test]
fn the_speedtest_history_comparison_empty_path() {
    let surface = business_surface(PAGE);
    assert_empty(&surface, PAGE, BusinessId::SpeedtestHistoryComparison);
}

#[test]
fn the_runtime_telemetry_observation_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::RuntimeTelemetryObservation);
}

#[test]
fn the_runtime_telemetry_observation_stale_path() {
    let surface = business_surface(PAGE);
    assert_stable(&surface, BusinessId::RuntimeTelemetryObservation);
}

#[test]
fn the_runtime_telemetry_observation_refresh_path() {
    let surface = business_surface(PAGE);
    assert_refreshed(&surface, BusinessId::RuntimeTelemetryObservation);
}

#[test]
fn the_runtime_telemetry_observation_locale_path() {
    let surface = business_surface(PAGE);
    assert_locale(&surface, BusinessId::RuntimeTelemetryObservation);
}

#[test]
fn the_runtime_telemetry_observation_unknown_path() {
    let surface = business_surface(PAGE);
    let summary = cell(&surface, "runtime-telemetry-observation/summary")
        .expect("the telemetry panel is projected");

    assert!(
        !summary.detail.contains("0"),
        "an unobserved telemetry must not claim a measured rate"
    );
}

#[test]
fn the_runtime_telemetry_observation_zero_path() {
    let surface = business_surface(PAGE);
    let mut snapshot = fixture::business_snapshot();
    snapshot.shell_readout.upload_bps = ShellObservation::Current(0.0);
    snapshot.shell_readout.download_bps = ShellObservation::Current(0.0);
    surface.publish_snapshot(snapshot);

    let summary = cell(&surface, "runtime-telemetry-observation/summary")
        .expect("the telemetry panel is projected");

    assert!(
        summary.detail.contains("0"),
        "a measured zero must be shown as the rate the owner published"
    );
}
