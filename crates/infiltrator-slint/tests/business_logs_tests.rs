//! The business panels of the logs workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_pending, assert_refused, assert_retried,
    assert_stable, assert_unsupported, business_surface, cell,
};
use infiltrator_contract::surface_snapshot::LogSnapshot;
use infiltrator_slint::surface::action::workbench::business::toggle_action;
use infiltrator_slint::surface::business::BusinessId;
use infiltrator_slint::surface::harness::SurfaceHarness;

/// The page that carries the log businesses.
const PAGE: &str = "LogsWorkbench";

#[test]
fn the_logs_redacted_export_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_cancel_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_retry_path() {
    let surface = business_surface(PAGE);
    assert_retried(&surface, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_stale_path() {
    let surface = business_surface(PAGE);
    assert_stable(&surface, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_pending_path() {
    let surface = business_surface(PAGE);
    assert_pending(&surface, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::LogsRedactedExport);
}

#[test]
fn the_logs_redacted_export_filtered_path() {
    let surface = business_surface(PAGE);
    let mut snapshot = fixture::business_snapshot();
    if let Some(page) = snapshot.pages.logs.data.as_mut() {
        page.active_level = Some("warn".to_owned());
    }
    surface.publish_snapshot(snapshot);
    fixture::open(&surface, BusinessId::LogsRedactedExport);

    let facts = cell(&surface, "logs-redacted-export/fact/2")
        .expect("the export review shows the active filter");

    assert_eq!(
        facts.detail, "warn",
        "the export must honour the filter the owner published"
    );
}

#[test]
fn the_logs_scroll_lock_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::LogsScrollLock);
}

/// The follow copy the shared follow state publishes.
fn follow_label(surface: &SurfaceHarness) -> String {
    fixture::open(surface, BusinessId::LogsScrollLock);
    cell(surface, "logs-scroll-lock/fact/0")
        .expect("the follow panel shows the shared follow state")
        .detail
        .to_string()
}

#[test]
fn the_logs_scroll_lock_lock_path() {
    let surface = business_surface(PAGE);
    let following = follow_label(&surface);

    surface.workbench_action(&toggle_action(BusinessId::LogsScrollLock), "");

    assert_ne!(
        follow_label(&surface),
        following,
        "the locked stream must publish the shared resume copy"
    );
}

#[test]
fn the_logs_scroll_lock_resume_path() {
    let surface = business_surface(PAGE);
    let following = follow_label(&surface);

    surface.workbench_action(&toggle_action(BusinessId::LogsScrollLock), "");
    surface.workbench_action(&toggle_action(BusinessId::LogsScrollLock), "");

    assert_eq!(
        follow_label(&surface),
        following,
        "the resumed stream must publish the shared lock copy again"
    );
}

#[test]
fn the_logs_scroll_lock_history_path() {
    let surface = business_surface(PAGE);

    fixture::open(&surface, BusinessId::LogsScrollLock);
    let facts = cell(&surface, "logs-scroll-lock/fact/0").expect("the follow panel is projected");

    assert!(
        !facts.detail.is_empty(),
        "the panel must show the shared follow state"
    );
}

#[test]
fn the_logs_scroll_lock_growth_path() {
    let surface = business_surface(PAGE);
    fixture::open(&surface, BusinessId::LogsScrollLock);
    let before = surface
        .workbench_items()
        .iter()
        .filter(|item| item.id.starts_with("logs-scroll-lock/fact/"))
        .count();

    let mut snapshot = fixture::business_snapshot();
    if let Some(page) = snapshot.pages.logs.data.as_mut() {
        page.entries.push(LogSnapshot {
            id: 4,
            raw: None,
            timestamp: "12:00:04".into(),
            level: "info".into(),
            tag: "CORE".into(),
            message: "the owner published another entry".into(),
        });
    }
    surface.publish_snapshot(snapshot);
    fixture::open(&surface, BusinessId::LogsScrollLock);

    let after = surface
        .workbench_items()
        .iter()
        .filter(|item| item.id.starts_with("logs-scroll-lock/fact/"))
        .count();
    assert!(
        after > before,
        "the published growth must reach the panel that shows the stream"
    );
}
