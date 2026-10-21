//! The surface's confirmation dialogs are the toolkit's popup windows.
//!
//! The toolkit's `PopupWindow` owns the modal layer above the page and the
//! open/close lifecycle, so the surface no longer keeps a second "is the
//! dialog open" flag: the popup's own `is-open` is the one truth the surface
//! publishes. These cases drive the real surface gestures and read that
//! state back.

use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;

fn empty_snapshot() -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    snapshot.pages.profiles = PageData::ready(
        serde_json::from_value(serde_json::json!({
            "profiles": [],
            "auto_update_interval_hours": 24,
            "updating": false
        }))
        .expect("profiles page JSON"),
    );
    snapshot
}

/// The clear-traffic confirmation opens as the toolkit's popup, so the
/// surface publishes the popup's own open state.
#[test]
fn the_clear_traffic_confirmation_is_the_toolkit_popup() {
    let surface = harness::SurfaceHarness::new(Some(empty_snapshot())).unwrap();
    assert!(
        !surface.clear_traffic_dialog_open(),
        "a closed surface must not carry an open confirmation"
    );

    match surface.open_clear_traffic_dialog() {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "opening the confirmation must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the confirmation to open, got the failure {failure:?}")
        }
    }
    assert!(
        surface.clear_traffic_dialog_open(),
        "the toolkit's popup owns the open state of the confirmation"
    );
}

/// The network-mode confirmation opens as the toolkit's popup too, so both
/// confirmations share one framework primitive.
#[test]
fn the_network_mode_confirmation_is_the_toolkit_popup() {
    let surface = harness::SurfaceHarness::new(Some(empty_snapshot())).unwrap();
    assert!(
        !surface.network_mode_dialog_open(),
        "a closed surface must not carry an open confirmation"
    );

    match surface.open_network_mode_dialog("TUN") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "opening the confirmation must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the confirmation to open, got the failure {failure:?}")
        }
    }
    assert!(
        surface.network_mode_dialog_open(),
        "the toolkit's popup owns the open state of the confirmation"
    );
}

/// The back gesture closes both confirmations, the way it closes the menus.
#[test]
fn the_back_gesture_closes_both_confirmations() {
    let surface = harness::SurfaceHarness::new(Some(empty_snapshot())).unwrap();

    surface.open_clear_traffic_dialog();
    assert!(surface.clear_traffic_dialog_open());
    surface.go_back();
    assert!(
        !surface.clear_traffic_dialog_open(),
        "the back gesture closes the clear-traffic confirmation"
    );

    surface.open_network_mode_dialog("TUN");
    assert!(surface.network_mode_dialog_open());
    surface.go_back();
    assert!(
        !surface.network_mode_dialog_open(),
        "the back gesture closes the network-mode confirmation"
    );
}
