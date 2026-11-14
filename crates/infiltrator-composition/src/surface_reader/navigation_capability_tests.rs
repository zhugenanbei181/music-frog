//! test-intent: behavior

use super::project_navigation_capability;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_domain::settings::AppSettings;

#[test]
fn hidden_capabilities_remove_logs_and_proxies_from_the_visible_rows() {
    let settings = AppSettings {
        open_logs: false,
        ..AppSettings::default()
    };
    let snapshot = project_navigation_capability(
        SurfaceKind::SlintDesktop,
        "logs",
        Some(&settings),
        None,
        None,
    );
    assert!(!snapshot.features.open_logs);
    assert!(!snapshot.features.has_proxies);
    assert!(!snapshot.desktop_rows.iter().any(|row| row.target == "logs"));
    assert!(
        !snapshot
            .desktop_rows
            .iter()
            .any(|row| row.target == "proxies")
    );
    assert_eq!(snapshot.active_destination, "overview");
}

#[test]
fn an_observed_requested_page_falls_back_only_when_it_is_hidden() {
    let settings = AppSettings {
        open_logs: false,
        ..AppSettings::default()
    };
    let visible = project_navigation_capability(
        SurfaceKind::SlintDesktop,
        "logs",
        Some(&AppSettings::default()),
        None,
        None,
    );
    assert_eq!(visible.active_destination, "logs");

    let hidden = project_navigation_capability(
        SurfaceKind::SlintDesktop,
        "logs",
        Some(&settings),
        None,
        None,
    );
    assert_eq!(hidden.active_destination, "overview");
    assert!(!hidden.desktop_rows.iter().any(|row| row.target == "logs"));
}
