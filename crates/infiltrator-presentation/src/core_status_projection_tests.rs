//! test-intent: behavior
use super::*;
use infiltrator_shared::locales::Lang;

#[test]
fn lifecycle_and_source_copy_keep_pending_failure_unknown_and_demo_distinct_in_both_languages() {
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Running, &Lang("en-US")),
        "Running"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Ready, &Lang("en-US")),
        "Running"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Stopped, &Lang("zh-CN")),
        "已停止"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Starting, &Lang("en-US")),
        "Starting..."
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Stopping, &Lang("en-US")),
        "Stopping…"
    );
    assert_eq!(
        lifecycle_copy(&CoreLifecycle::Failed, &Lang("en-US")),
        "Runtime Error"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Live, None, &Lang("en-US")),
        "Live core · Version not observed"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Live, Some("  "), &Lang("zh-CN")),
        "实时内核 · 版本未观测"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Live, Some(" v1.19.18 "), &Lang("en-US")),
        "Live core · v1.19.18"
    );
    assert_eq!(
        source_copy(SurfaceOrigin::Demo, Some("v1.19.18"), &Lang("en-US")),
        "Demo data · No live core"
    );
}

#[test]
fn busy_lifecycles_have_no_fabricated_failure_and_actual_read_errors_remain_visible_and_redacted() {
    assert_eq!(
        failure_copy(&CoreLifecycle::Starting, None, &Lang("en-US")),
        ""
    );
    assert_eq!(
        failure_copy(&CoreLifecycle::Stopping, Some("  "), &Lang("zh-CN")),
        ""
    );
    assert_eq!(
        failure_copy(&CoreLifecycle::Failed, None, &Lang("en-US")),
        "Failure reason not observed"
    );
    assert_eq!(
        failure_copy(&CoreLifecycle::Running, Some("read denied"), &Lang("en-US")),
        "read denied"
    );
    let redacted = failure_copy(
        &CoreLifecycle::Failed,
        Some("Authorization: Bearer private-token"),
        &Lang("en-US"),
    );
    assert!(!redacted.contains("private-token"));
    assert_eq!(redacted, "Authorization: Bearer ***");
}
