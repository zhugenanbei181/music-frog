//! test-intent: behavior
//! Policy copy must describe the decision on selected and unselected applications.
use super::*;
use infiltrator_domain::app_routing::AppRoutingConfig;
use std::collections::HashSet;

#[test]
fn policy_copy_agrees_with_actual_selected_and_unselected_app_decisions() {
    for (mode, selected, other, copy) in [
        (
            AppRoutingMode::ProxyAll,
            true,
            true,
            "Global Routing (All Apps Proxied)",
        ),
        (
            AppRoutingMode::ProxySelected,
            true,
            false,
            "Proxy selected apps only",
        ),
        (
            AppRoutingMode::BypassSelected,
            false,
            true,
            "Bypass selected apps; proxy the rest",
        ),
    ] {
        let config = AppRoutingConfig {
            mode,
            packages: HashSet::from(["selected.app".into()]),
            ..Default::default()
        };
        assert_eq!(config.should_proxy("selected.app"), selected);
        assert_eq!(config.should_proxy("another.app"), other);
        assert!(routing_summary(mode, 1, "en-US").contains(copy));
    }
    assert!(routing_summary(AppRoutingMode::ProxySelected, 1, "zh-CN").contains("仅代理选中应用"));
    assert!(
        routing_summary(AppRoutingMode::BypassSelected, 1, "zh-CN")
            .contains("选中应用直连，其余代理")
    );
}
#[test]
fn uwp_summary_counts_real_flags_and_unsupported_never_reports_zero_success() {
    let available = UwpLoopbackAvailability::Supported;
    assert_eq!(
        uwp_summary(&available, [true, false, true], "en-US"),
        "3 UWP AppContainers observed · 2 exempted"
    );
    assert_eq!(
        uwp_summary(&available, [], "en-US"),
        "0 UWP AppContainers observed · 0 exempted"
    );
    let unavailable = UwpLoopbackAvailability::Unavailable {
        reason: "permission denied {count}".into(),
    };
    assert_eq!(
        uwp_summary(&unavailable, [true], "en-US"),
        "permission denied {count}"
    );
    assert_eq!(
        process_copy("process-{mode}-{count}", "en-US"),
        "Process: process-{mode}-{count}"
    );
}
