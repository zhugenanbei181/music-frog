//! test-intent: behavior
use crate::app_routing_page_model::AppRoutingPageModel;
use infiltrator_contract::app_routing::{AppRoutingMode, AppRoutingRule};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface_snapshot::{AppRoutingPageSnapshot, AppSnapshot};
use infiltrator_contract::uwp::{UwpLoopbackAvailability, UwpLoopbackSnapshot, UwpPackageSnapshot};

fn app(id: &str, name: &str, process: &str) -> AppSnapshot {
    AppSnapshot {
        id: id.to_owned(),
        name: name.to_owned(),
        process_name: process.to_owned(),
        rule: AppRoutingRule::Proxy,
        is_system: false,
    }
}

fn page() -> AppRoutingPageSnapshot {
    AppRoutingPageSnapshot {
        mode: AppRoutingMode::ProxySelected,
        include_system: true,
        apps: vec![
            app("com.example.browser", "Browser", "browser.exe"),
            app("com.example.mail", "Mail", "mail.exe"),
        ],
        uwp_loopback: UwpLoopbackSnapshot {
            availability: UwpLoopbackAvailability::Supported,
            packages: vec![
                UwpPackageSnapshot {
                    sid: "S-1-5-21-a".to_owned(),
                    display_name: "First App".to_owned(),
                    package_family_name: "Example.First".to_owned(),
                    loopback_exempt: true,
                },
                UwpPackageSnapshot {
                    sid: "S-1-5-21-b".to_owned(),
                    display_name: "Second App".to_owned(),
                    package_family_name: "Example.Second".to_owned(),
                    loopback_exempt: false,
                },
            ],
            revision: 9,
        },
    }
}

#[test]
fn the_page_reports_the_owner_values_without_a_second_shape() {
    let mut model = AppRoutingPageModel::default();
    // Before the owner published anything the page reports no mode at all.
    assert!(model.mode().is_none());
    assert!(model.apps().is_empty());
    assert!(model.uwp_packages().is_empty());

    model.observe_page(page());
    assert_eq!(model.mode(), Some(AppRoutingMode::ProxySelected));
    assert_eq!(model.include_system(), Some(true));
    assert_eq!(model.apps().len(), 2);
    assert_eq!(model.uwp_packages().len(), 2);
    assert_eq!(model.uwp_revision(), 9);
    assert_eq!(
        model.uwp_availability(),
        &UwpLoopbackAvailability::Supported
    );
}

#[test]
fn the_application_filter_hides_the_applications_it_does_not_match() {
    let mut model = AppRoutingPageModel::default();
    model.observe_page(page());

    assert_eq!(model.visible_apps().len(), 2);

    model.edit_app_filter("mail");
    let visible = model.visible_apps();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, "com.example.mail");

    model.edit_app_filter("com.example");
    assert_eq!(model.visible_apps().len(), 2);

    model.edit_app_filter("nothing");
    assert!(model.visible_apps().is_empty());
}

#[test]
fn the_loopback_filter_hides_the_packages_it_does_not_match() {
    let mut model = AppRoutingPageModel::default();
    model.observe_page(page());

    assert_eq!(model.visible_uwp_packages().len(), 2);

    model.edit_uwp_filter("second");
    let visible = model.visible_uwp_packages();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].sid, "S-1-5-21-b");
    assert!(!visible[0].loopback_exempt);

    model.edit_uwp_filter("example.first");
    assert_eq!(model.visible_uwp_packages().len(), 1);
}

#[test]
fn the_scan_is_one_typed_phase_instead_of_a_boolean() {
    let mut model = AppRoutingPageModel::default();
    assert!(!model.scan_phase().is_running());

    let token = model.scan_phase_mut().begin();
    assert!(model.scan_phase().is_running());
    assert_eq!(model.scan_phase().token(), Some(token));

    assert!(model.scan_phase_mut().succeed(token));
    assert!(!model.scan_phase().is_running());
}

#[test]
fn the_owner_intents_carry_the_selected_values() {
    let mut model = AppRoutingPageModel::default();
    model.observe_page(page());

    assert_eq!(
        model.set_mode_intent(AppRoutingMode::BypassSelected),
        CommandIntent::SetAppRoutingMode {
            mode: AppRoutingMode::BypassSelected,
        }
    );
    assert_eq!(
        model.set_app_rule_intent("com.example.mail", AppRoutingRule::Direct),
        CommandIntent::SetAppRule {
            app_id: "com.example.mail".to_owned(),
            rule: AppRoutingRule::Direct,
        }
    );
    assert_eq!(
        model.set_exemption_intent("S-1-5-21-a", false),
        CommandIntent::SetUwpAppExemption {
            sid: "S-1-5-21-a".to_owned(),
            exempt: false,
        }
    );
    assert_eq!(
        model.set_all_exemptions_intent(true),
        CommandIntent::SetAllUwpExemptions { exempt: true }
    );
    assert_eq!(model.scan_intent(), CommandIntent::ScanUwpApps);
}

#[test]
fn the_application_rule_comes_from_the_owner_snapshot() {
    let mut model = AppRoutingPageModel::default();
    model.observe_page(page());

    assert_eq!(
        model.app_rule("com.example.browser"),
        Some(AppRoutingRule::Proxy)
    );
    assert!(model.app_rule("com.example.missing").is_none());
}
