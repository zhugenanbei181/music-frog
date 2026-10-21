//! GAP-4.1: the parity capture scenarios of the real Slint surface.
//!
//! The capture path may only publish a business the surface really opens.
//! Every case drives the production capture activation against the headless
//! surface harness and asserts that the carrier state is really open, that a
//! business the surface does not carry is refused, and that the capture
//! matrix lists exactly the businesses the surface carries.

use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ProfileSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_slint::capture_scenario::{Activation, CarriedScenario, ScenarioCarrier, activate};
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn profile(id: &str, name: &str, active: bool) -> ProfileSnapshot {
    serde_json::from_value(json!({
        "id": id,
        "name": name,
        "url": "https://example.com/sub",
        "updated_at": "2026-10-16 12:00:00",
        "is_active": active,
        "write_protection": "editable"
    }))
    .expect("profile JSON")
}

fn capture_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    let page: ProfilesPageSnapshot = serde_json::from_value(json!({
        "profiles": [
            profile("profile-1", "Alpha Fast (HK/JP/US)", true),
            profile("profile-2", "Beta Local Direct", false)
        ],
        "auto_update_interval_hours": 24,
        "updating": false
    }))
    .expect("profiles page JSON");
    snapshot.pages.profiles = PageData::ready(page);
    snapshot
}

/// The rows of the crate's capture matrix.
fn matrix_rows() -> Vec<(String, String, String, String)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/capture/parity_scenarios.tsv");
    let text = fs::read_to_string(&path).expect("capture matrix");
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        rows.push((
            fields.next().unwrap_or_default().to_owned(),
            fields.next().unwrap_or_default().to_owned(),
            fields.next().unwrap_or_default().to_owned(),
            fields.next().unwrap_or_default().to_owned(),
        ));
    }
    rows
}

#[test]
fn every_carried_scenario_really_opens_its_carrier() {
    let snapshot = capture_snapshot();
    // The owner's typed answer of every business the capture drives.
    let answer = Failure::new(ErrorCode::Internal, "the owner refused the command", true);

    for scenario in CarriedScenario::ALL {
        // Every business is captured in its own process, so every scenario
        // starts from the route-only render of its page.
        let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
        let window = surface.window();
        let activation = Activation::default();
        // The first call drives the business's own gesture.
        activate(
            window,
            scenario.name(),
            &snapshot,
            surface.workbenches(),
            &activation,
        );
        if matches!(scenario.carrier(), ScenarioCarrier::WorkbenchPage(_)) {
            assert!(
                activation.performed(),
                "{} must drive its own gesture",
                scenario.name()
            );
        }
        // The owner answers, so the business shows its own outcome.
        surface.publish_owner_failure(&answer);
        assert!(
            activate(
                window,
                scenario.name(),
                &snapshot,
                surface.workbenches(),
                &activation,
            ),
            "the surface carries {} and must show its outcome",
            scenario.name()
        );
        match scenario.carrier() {
            ScenarioCarrier::WorkbenchPage(page) => {
                assert_eq!(
                    window.get_page().as_str(),
                    page,
                    "{} must open the {} page",
                    scenario.name(),
                    page
                );
                assert!(
                    !surface.workbench_items().is_empty(),
                    "{} must project the business panel of its page",
                    scenario.name()
                );
            }
            ScenarioCarrier::ProfileImportSheet => {
                assert!(
                    window.get_profile_add_open(),
                    "{} must open the add-profile sheet",
                    scenario.name()
                );
            }
            ScenarioCarrier::ProfileDeleteSheet => {
                assert_eq!(
                    window.get_pending_delete_id(),
                    "profile-2",
                    "{} must open the delete sheet of the deletable profile",
                    scenario.name()
                );
            }
        }
    }
}

#[test]
fn the_capture_gesture_changes_the_rendered_page() {
    let snapshot = capture_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    let window = surface.window();
    let activation = Activation::default();
    let answer = Failure::new(ErrorCode::Internal, "the owner refused the command", true);

    // The DNS hosts editor is typed into and the row is added, so the
    // business's own list grows.
    let before = surface.workbench_items().len();
    activate(
        window,
        "dns-hosts-editor",
        &snapshot,
        surface.workbenches(),
        &activation,
    );
    let after = surface.workbench_items();
    assert!(
        after.len() > before,
        "the hosts editor adds the row the capture typed"
    );
    assert!(
        after
            .iter()
            .any(|item| item.detail.contains("blocked.example.com")),
        "the hosts editor shows the row the capture typed"
    );

    // The owner answers, so the business shows its own outcome.
    surface.publish_owner_failure(&answer);
    assert!(
        activate(
            window,
            "dns-hosts-editor",
            &snapshot,
            surface.workbenches(),
            &activation,
        ),
        "the business shows the owner's answer"
    );
}

#[test]
fn a_business_the_surface_does_not_carry_is_never_activated() {
    let snapshot = capture_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    let window = surface.window();
    let activation = Activation::default();

    // The businesses the Slint surface still has no carrier for.
    for absent in [
        "connections-details-drawer",
        "connections-close-all-confirm",
        "connections-grouping",
        "logs-search-highlight",
        "logs-scroll-lock",
        "logs-redacted-export",
        "proxies-group-expanded",
        "rules-list-editor",
        "settings-permission-panel",
        "shell-command-palette",
        "speedtest-details-modal",
        "a-business-that-does-not-exist",
    ] {
        assert!(
            !activate(
                window,
                absent,
                &snapshot,
                surface.workbenches(),
                &activation,
            ),
            "{absent} has no carrier on the Slint surface and must never be activated"
        );
    }

    // A refused scenario leaves the surface where it was.
    assert_eq!(window.get_page().as_str(), "Home");
    assert!(!window.get_profile_add_open());
}

#[test]
fn the_capture_matrix_lists_exactly_the_carried_businesses() {
    let rows = matrix_rows();
    let expected: BTreeSet<String> = CarriedScenario::ALL
        .into_iter()
        .flat_map(|scenario| {
            [
                format!("{}-standard", scenario.name()),
                format!("{}-compact", scenario.name()),
            ]
        })
        .collect();
    let listed: BTreeSet<String> = rows.iter().map(|(name, ..)| name.clone()).collect();
    assert_eq!(
        listed, expected,
        "the capture matrix must list every carried business in both viewports"
    );

    for (name, page, skin, window_size) in &rows {
        assert!(!page.is_empty(), "{name} must name the page it captures");
        assert!(!skin.is_empty(), "{name} must name the skin it captures");
        assert!(
            window_size.contains('x'),
            "{name} must name the window size it captures"
        );
    }
}
