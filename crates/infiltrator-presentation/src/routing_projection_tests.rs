//! test-intent: behavior
//! Policy copy must describe the decision on selected and unselected applications.
use super::*;
use infiltrator_domain::app_routing::AppRoutingConfig;
use infiltrator_shared::locales::{Lang, Localizer};
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
        assert!(routing_summary(Some(mode), 1, &Lang("en-US")).contains(copy));
    }
    assert!(
        routing_summary(Some(AppRoutingMode::ProxySelected), 1, &Lang("zh-CN"))
            .contains("仅代理选中应用")
    );
    assert!(
        routing_summary(Some(AppRoutingMode::BypassSelected), 1, &Lang("zh-CN"))
            .contains("选中应用直连，其余代理")
    );
}

#[test]
fn an_unobserved_routing_mode_has_its_own_copy_in_both_languages() {
    // The unobserved case is a fact of its own: it never falls back to a
    // mode the owner never reported.
    assert_eq!(mode_option_key(None), "app_routing_mode_unobserved");
    assert_eq!(
        mode_option_key(Some(AppRoutingMode::ProxyAll)),
        "app_routing_mode_global"
    );

    let english = Lang("en-US");
    let chinese = Lang("zh-CN");
    assert_eq!(
        english.tr(mode_option_key(None)),
        "Routing mode not observed"
    );
    assert_eq!(chinese.tr(mode_option_key(None)), "尚未观测分流模式");

    let summary = routing_summary(None, 3, &english);
    assert!(summary.contains("mode not observed"));
    assert!(summary.contains('3'));
    assert!(!summary.contains("Global Routing"));
    assert!(
        routing_summary(None, 3, &chinese).contains("尚未观测分流模式"),
        "the unobserved summary exists in the zh-CN table too"
    );
}
#[test]
fn uwp_summary_counts_real_flags_and_unsupported_never_reports_zero_success() {
    let available = UwpLoopbackAvailability::Supported;
    assert_eq!(
        uwp_summary(&available, [true, false, true], &Lang("en-US")),
        "3 UWP AppContainers observed · 2 exempted"
    );
    assert_eq!(
        uwp_summary(&available, [], &Lang("en-US")),
        "0 UWP AppContainers observed · 0 exempted"
    );
    let unavailable = UwpLoopbackAvailability::Unavailable {
        reason: "permission denied {count}".into(),
    };
    assert_eq!(
        uwp_summary(&unavailable, [true], &Lang("en-US")),
        "permission denied {count}"
    );
    assert_eq!(
        process_copy("process-{mode}-{count}", &Lang("en-US")),
        "Process: process-{mode}-{count}"
    );
}
