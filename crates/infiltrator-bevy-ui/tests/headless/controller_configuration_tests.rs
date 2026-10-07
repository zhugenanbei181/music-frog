//! test-intent: behavior
use infiltrator_bevy_ui::controller::{FailureDwell, controller_config_from_raw};
use std::time::{Duration, Instant};

// ---- the config seam ----------------------------------------------------------

/// The env-shaped config resolver: a valid controller switches the source,
/// junk/missing values keep the demo frontend, the secret is trimmed and
/// optional. Pure-function level (no process-global env assertions).
#[test]
fn controller_config_resolves_or_keeps_the_demo() {
    assert!(controller_config_from_raw(None, None).is_none());
    assert!(controller_config_from_raw(Some(""), None).is_none());
    assert!(controller_config_from_raw(Some("127.0.0.1:9099"), None).is_none());

    let config = controller_config_from_raw(Some("http://127.0.0.1:9099"), Some("sekrit"))
        .expect("valid controller resolves");
    assert_eq!(config.endpoint, "http://127.0.0.1:9099");
    assert_eq!(config.secret.as_deref(), Some("sekrit"));
    assert_eq!(config.sample_interval, Duration::from_millis(700));

    assert!(controller_config_from_raw(Some("http://127.0.0.1:9099"), None).is_some());
}

// ---- the failure-verdict dwell -------------------------------------------------

/// The dwell's own law, at pure-resource level: successful snapshots are
/// deferred only inside the latched window.
#[test]
fn failure_dwell_defers_successes_only_inside_the_window() {
    let mut dwell = FailureDwell::new(Duration::from_secs(5));
    let now = Instant::now();
    assert!(
        dwell.success_may_pass(now),
        "an unlatched dwell passes everything"
    );
    dwell.latch(now);
    assert!(
        !dwell.success_may_pass(now + Duration::from_millis(4_999)),
        "a success inside the window is deferred"
    );
    assert!(
        dwell.success_may_pass(now + Duration::from_secs(5)),
        "the first success after the window clears the verdict"
    );
}
