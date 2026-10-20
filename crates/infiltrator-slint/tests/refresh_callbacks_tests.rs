//! SLINT-001.9: the two declared Slint callbacks that had no call site.
//!
//! `refresh-profile` and `refresh-home-egress` were declared on the window but
//! never reached a widget, so the subscription refresh and the public egress
//! re-probe had no carrier. These cases drive the production callbacks through
//! the headless surface harness and assert the shared owner command.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness;
use lurker_core_contract::error::{ErrorCode, Failure};

/// The owner snapshot the surface observes.
fn desktop_snapshot() -> SurfaceSnapshot {
    SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    )
}

/// Asserts that the surface dispatched exactly one owner command.
fn single_command(outcome: harness::SurfaceOutcome) -> CommandIntent {
    match outcome {
        harness::SurfaceOutcome::Dispatched(actions) => {
            assert_eq!(
                actions.len(),
                1,
                "expected one owner action, got {actions:?}"
            );
            match &actions[0] {
                SlintAction::Command(intent) => intent.clone(),
                other => panic!("expected an owner command, got {other:?}"),
            }
        }
        harness::SurfaceOutcome::Rejected { failure, .. } => {
            panic!("expected an owner command, got the failure {failure:?}")
        }
    }
}

#[test]
fn the_subscription_refresh_dispatches_the_shared_update_command() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.refresh_profile("main"));

    assert_eq!(
        command,
        CommandIntent::UpdateProfile {
            profile_id: "main".to_owned()
        }
    );
}

#[test]
fn the_public_egress_reprobe_dispatches_the_shared_probe_command() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.refresh_home_egress());

    assert_eq!(command, CommandIntent::RefreshPublicIpProbe);
}
