//! test-intent: behavior
//!
//! Product copy stays in each product catalog, so these tests use a synthetic
//! catalog: keys come back as stable markers and only the interpolation
//! template is shaped like real copy.
use super::*;
use crate::copy::TextCatalog;
use std::borrow::Cow;

struct StubCatalog;

impl TextCatalog for StubCatalog {
    fn text(&self, key: &str) -> Cow<'static, str> {
        match key {
            "core_source_live_version" => Cow::Borrowed("<{version}>"),
            _ => Cow::Owned(format!("<{key}>")),
        }
    }
}

#[test]
fn lifecycle_and_source_copy_keep_pending_failure_unknown_and_demo_distinct() {
    let catalog = StubCatalog;
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Running, &catalog),
        "<status_running>"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Ready, &catalog),
        "<status_running>"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Stopped, &catalog),
        "<status_stopped>"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Starting, &catalog),
        "<status_starting>"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Stopping, &catalog),
        "<core_control_stopping>"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Failed, &catalog),
        "<status_error>"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Live, None, &catalog),
        "<core_source_live_unknown_version>"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Live, Some("  "), &catalog),
        "<core_source_live_unknown_version>"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Live, Some(" v1.19.18 "), &catalog),
        "<v1.19.18>"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Demo, Some("v1.19.18"), &catalog),
        "<core_source_demo>"
    );
}

#[test]
fn busy_lifecycles_have_no_fabricated_failure_and_actual_read_errors_remain_visible_and_redacted() {
    let catalog = StubCatalog;
    assert_eq!(failure_copy(&CoreLifecycle::Starting, None, &catalog), "");
    assert_eq!(
        failure_copy(&CoreLifecycle::Stopping, Some("  "), &catalog),
        ""
    );
    assert_eq!(
        failure_copy(&CoreLifecycle::Failed, None, &catalog),
        "<core_failure_reason_unobserved>"
    );
    assert_eq!(
        failure_copy(&CoreLifecycle::Running, Some("read denied"), &catalog),
        "read denied"
    );
    let redacted = failure_copy(
        &CoreLifecycle::Failed,
        Some("Authorization: Bearer private-token"),
        &catalog,
    );
    assert!(!redacted.contains("private-token"));
    assert_eq!(redacted, "Authorization: Bearer ***");
}

#[test]
fn banner_verdict_never_reports_a_live_takeover_from_demo_unauthorized_or_unobserved_facts() {
    let catalog = StubCatalog;
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Demo, &CoreLifecycle::Running, true, false),
        OverviewBannerVerdict::Demo
    );
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Live, &CoreLifecycle::Failed, true, false),
        OverviewBannerVerdict::MissingHost
    );
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Live, &CoreLifecycle::Stopped, true, false),
        OverviewBannerVerdict::CoreStopped
    );
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Live, &CoreLifecycle::Starting, true, false),
        OverviewBannerVerdict::Transitioning
    );
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Live, &CoreLifecycle::Running, true, true),
        OverviewBannerVerdict::VpnUnauthorized
    );
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Live, &CoreLifecycle::Running, false, false),
        OverviewBannerVerdict::VpnUnauthorized
    );
    assert_eq!(
        project_banner_verdict(SurfaceOrigin::Live, &CoreLifecycle::Running, true, false),
        OverviewBannerVerdict::RealRunning
    );
    assert_eq!(
        banner_verdict_copy(
            OverviewBannerVerdict::Demo,
            &CoreLifecycle::Running,
            &catalog,
            false
        ),
        "<overview_demo_mode>"
    );
    assert_eq!(
        banner_verdict_copy(
            OverviewBannerVerdict::CoreStopped,
            &CoreLifecycle::Stopped,
            &catalog,
            false
        ),
        "<status_stopped>"
    );
    assert_eq!(
        banner_verdict_copy(
            OverviewBannerVerdict::VpnUnauthorized,
            &CoreLifecycle::Running,
            &catalog,
            true
        ),
        "<overview_vpn_unauthorized>"
    );
    assert_eq!(
        banner_verdict_copy(
            OverviewBannerVerdict::RealRunning,
            &CoreLifecycle::Running,
            &catalog,
            false
        ),
        "<status_running>"
    );
}
