//! SLINT-001.9: Parity scenario anchors of the real Slint surface.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness and asserts the exact typed owner command the surface
//! emits together with the owner-visible projection it leaves behind.
//!
//! The two businesses covered here are the ones the Slint surface really
//! implements:
//!
//! - `profiles-import-modal` (`success`, `cancel`, `error`): the add-profile
//!   sheet opens, the sheet passes its own draft to the shared import
//!   command, the dismissal keeps the catalog unchanged and an invalid Age
//!   private key is refused with the typed failure.
//! - `profiles-delete-confirm` (`success`, `cancel`, `error`): the delete
//!   sheet opens for the requested profile, the confirmation submits the
//!   shared delete command, the dismissal keeps the profile and the typed
//!   failure of a refused deletion reaches the notice.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{
    ProfileSnapshot, ProfilesPageSnapshot, SurfaceSnapshot,
};
use infiltrator_slint::SlintAction;
use infiltrator_slint::capture_scenario;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;
use serde_json::json;

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

fn profiles_snapshot() -> SurfaceSnapshot {
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

/// Asserts that the surface dispatched exactly one owner action.
fn single_action(outcome: harness::SurfaceOutcome) -> SlintAction {
    match outcome {
        harness::SurfaceOutcome::Dispatched(mut actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            actions.remove(0)
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner action, got the failure {failure:?}")
        }
    }
}

/// Asserts that the surface refused the gesture with the given typed failure.
fn rejection(outcome: harness::SurfaceOutcome) -> Failure {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            panic!("expected a refusal, got the owner actions {actions:?}")
        }
        harness::SurfaceOutcome::Rejected { failure, actions } => {
            assert!(
                actions.is_empty(),
                "a refused gesture must not dispatch owner actions, got {actions:?}"
            );
            failure
        }
    }
}

#[test]
fn profiles_import_modal_success_dispatches_the_shared_import_command() {
    let surface = harness::SurfaceHarness::new(Some(profiles_snapshot())).unwrap();

    // The user opens the sheet from the "添加配置" tile.
    match surface.open_profile_add() {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "opening the sheet must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the sheet to open, got the failure {failure:?}")
        }
    }
    assert!(surface.profile_add_open());

    // The user fills the sheet and confirms it.
    surface.fill_profile_add("Beta Subscription", "https://example.com/sub/beta", "");
    let action = single_action(surface.confirm_profile_add());
    match action {
        SlintAction::ImportEncryptedSubscription {
            name,
            url,
            private_key,
        } => {
            assert_eq!(name, "Beta Subscription");
            assert_eq!(url, "https://example.com/sub/beta");
            assert_eq!(private_key, None);
        }
        other => panic!("expected the shared import command, got {other:?}"),
    }
}

#[test]
fn profiles_import_modal_cancel_keeps_the_catalog_unchanged() {
    let surface = harness::SurfaceHarness::new(Some(profiles_snapshot())).unwrap();

    match surface.open_profile_add() {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "opening the sheet must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the sheet to open, got the failure {failure:?}")
        }
    }
    surface.fill_profile_add("Beta Subscription", "https://example.com/sub/beta", "");

    // The user dismisses the sheet without confirming it.
    match surface.cancel_profile_add() {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "a dismissed sheet must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the sheet to close, got the failure {failure:?}")
        }
    }
    assert!(!surface.profile_add_open());
}

#[test]
fn profiles_import_modal_error_refuses_an_invalid_age_key() {
    let surface = harness::SurfaceHarness::new(Some(profiles_snapshot())).unwrap();

    surface.open_profile_add();
    surface.fill_profile_add(
        "Beta Subscription",
        "https://example.com/sub/beta",
        "age-key-with-a-trailing-space ",
    );

    let failure = rejection(surface.confirm_profile_add());
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(
        failure.message,
        "Age private key must be a single trimmed line"
    );
    assert_eq!(surface.notice(), failure.message);
}

#[test]
fn profiles_delete_confirm_success_dispatches_the_shared_delete_command() {
    let surface = harness::SurfaceHarness::new(Some(profiles_snapshot())).unwrap();

    // The user asks for the deletion of a profile from its card.
    match surface.request_delete_profile("profile-2") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "asking for a deletion must not ask the owner for anything yet, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the delete sheet to open, got the failure {failure:?}")
        }
    }
    assert_eq!(surface.pending_delete_id(), "profile-2");
    assert_eq!(surface.pending_delete_name(), "Beta Local Direct");

    // The user confirms the deletion.
    let action = single_action(surface.confirm_delete_profile());
    match action {
        SlintAction::Command(CommandIntent::DeleteProfile { profile_id }) => {
            assert_eq!(profile_id, "profile-2");
        }
        other => panic!("expected the shared delete command, got {other:?}"),
    }
    assert_eq!(surface.pending_delete_id(), "");
}

#[test]
fn profiles_delete_confirm_cancel_keeps_the_profile() {
    let surface = harness::SurfaceHarness::new(Some(profiles_snapshot())).unwrap();

    match surface.request_delete_profile("profile-2") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "asking for a deletion must not ask the owner for anything yet, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the delete sheet to open, got the failure {failure:?}")
        }
    }

    // The user dismisses the sheet without confirming it.
    match surface.cancel_delete_profile() {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "a dismissed sheet must not ask the owner for anything, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the sheet to close, got the failure {failure:?}")
        }
    }
    assert_eq!(surface.pending_delete_id(), "");
    assert_eq!(surface.pending_delete_name(), "");
}

#[test]
fn profiles_delete_confirm_error_projects_the_owner_failure() {
    let surface = harness::SurfaceHarness::new(Some(profiles_snapshot())).unwrap();

    match surface.request_delete_profile("profile-2") {
        harness::SurfaceOutcome::Dispatched(actions) => assert!(
            actions.is_empty(),
            "asking for a deletion must not ask the owner for anything yet, got {actions:?}"
        ),
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected the delete sheet to open, got the failure {failure:?}")
        }
    }
    single_action(surface.confirm_delete_profile());

    // The owner refuses the deletion and reports its typed failure.
    let failure = Failure::new(
        ErrorCode::InvalidInput,
        "The profile is still active and cannot be deleted",
        false,
    );
    surface.publish_owner_failure(&failure);

    assert_eq!(surface.notice(), failure.message);
    assert_eq!(surface.pending_delete_id(), "");
}

#[test]
fn the_capture_scenario_opens_the_sheet_of_the_business_it_names() {
    let snapshot = profiles_snapshot();
    let surface = harness::SurfaceHarness::new(Some(snapshot.clone())).unwrap();
    let window = surface.window();

    // The import scenario opens the add-profile sheet of the surface.
    assert!(capture_scenario::activate(
        window,
        "profiles-import-modal",
        &snapshot,
        surface.workbenches(),
        &capture_scenario::Activation::default(),
    ));
    assert!(window.get_profile_add_open());

    // The delete scenario opens the delete sheet of the surface.
    assert!(capture_scenario::activate(
        window,
        "profiles-delete-confirm",
        &snapshot,
        surface.workbenches(),
        &capture_scenario::Activation::default(),
    ));
    assert_eq!(window.get_pending_delete_id(), "profile-2");
    assert_eq!(window.get_pending_delete_name(), "Beta Local Direct");

    // A business the surface does not carry is never activated. The DNS
    // workbench pages are carried now, so the probe uses a business the
    // Slint surface still has no page for.
    assert!(!capture_scenario::activate(
        window,
        "connections-details-drawer",
        &snapshot,
        surface.workbenches(),
        &capture_scenario::Activation::default(),
    ));
}
