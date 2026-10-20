//! SLINT-001.9: The Slint capture marker of the parity capture path.
//!
//! The marker is the receipt contract between the capture path and the
//! parity receipt pipeline: it names the activated business and the real
//! geometry of the surface that was opened. A scenario the surface did not
//! open is never published as activated, so a capture can never certify a
//! business the surface does not implement.
//!
//! The capture fixtures live behind the `capture-tools` feature, so this file
//! only carries cases when that feature is enabled.

#![cfg(feature = "capture-tools")]

use infiltrator_slint::capture_scenario::CaptureBounds;
use infiltrator_slint::demo::{DemoSettings, write_capture_marker};
use std::fs;
use std::path::PathBuf;

fn settings(marker: PathBuf, scenario: Option<&str>) -> DemoSettings {
    DemoSettings {
        enabled: true,
        page: "Config".into(),
        skin: "dark".into(),
        window_size: (1180.0, 780.0),
        marker: Some(marker),
        scenario: scenario.map(str::to_owned),
    }
}

fn read_marker(path: &PathBuf) -> String {
    fs::read_to_string(path).expect("capture marker")
}

#[test]
fn the_marker_publishes_the_activated_scenario_with_its_real_bounds() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("marker.log");
    let bounds = CaptureBounds {
        x: 270.0,
        y: 364.0,
        width: 640.0,
        height: 416.0,
    };

    write_capture_marker(
        &settings(marker.clone(), Some("profiles-import-modal")),
        &infiltrator_slint::AppWindow::new().unwrap(),
        true,
        Some(bounds),
    );

    let line = read_marker(&marker);
    assert!(
        line.contains("CAPTURE_READY page=profiles skin=dark"),
        "{line}"
    );
    assert!(
        line.contains("scenario=profiles-import-modal activated=true"),
        "{line}"
    );
    assert!(line.contains("bounds=270,364,640,416"), "{line}");
}

#[test]
fn the_marker_never_claims_a_scenario_the_surface_did_not_open() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("marker.log");

    write_capture_marker(
        &settings(marker.clone(), Some("dns-leak-alert")),
        &infiltrator_slint::AppWindow::new().unwrap(),
        false,
        None,
    );

    let line = read_marker(&marker);
    assert!(
        line.contains("scenario=dns-leak-alert activated=false"),
        "{line}"
    );
    assert!(!line.contains("activated=true"), "{line}");
    assert!(!line.contains("bounds="), "{line}");
}
