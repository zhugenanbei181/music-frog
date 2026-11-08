//! SLINT-001.9: the refresh callbacks that had no call site.
//!
//! `refresh-profile` and `refresh-home-egress` were declared on the window but
//! never reached a widget, so the subscription refresh and the public egress
//! re-probe had no carrier. These cases drive the production callbacks through
//! the headless surface harness and assert the shared owner command. The config
//! page header now carries `refresh-profile`, and it may only send the id the
//! owner published - the surface must not invent one.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::shell_readout::{ShellObservation, ShellProfile};
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

fn snapshot_with_profile(profile: ShellObservation<ShellProfile>) -> SurfaceSnapshot {
    let mut snapshot = desktop_snapshot();
    snapshot.shell_readout.profile = profile;
    snapshot
}

/// The header's refresh button is enabled and keyed by the owner's current
/// profile id, projected verbatim onto the window.
#[test]
fn the_refresh_carrier_sends_the_owner_current_profile_id() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();
    surface.publish_snapshot(snapshot_with_profile(ShellObservation::Current(
        ShellProfile {
            id: "alpha".to_owned(),
            name: "Alpha Fast".to_owned(),
            subscription: true,
            used_bytes: None,
            total_bytes: None,
            usage_fraction: None,
        },
    )));

    let projected = surface.window().get_current_profile_id().to_string();
    assert_eq!(projected, "alpha", "the id must arrive from the snapshot");

    let command = single_command(surface.refresh_profile(&projected));
    assert_eq!(
        command,
        CommandIntent::UpdateProfile {
            profile_id: "alpha".to_owned()
        }
    );
}

/// With nothing current the projection publishes no id, so the header control
/// is disabled rather than sending a command for a profile that does not exist.
#[test]
fn an_unobserved_profile_projects_no_refreshable_id() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();
    surface.publish_snapshot(snapshot_with_profile(ShellObservation::Unobserved));

    assert_eq!(surface.window().get_current_profile_id(), "");
}

#[test]
fn the_public_egress_reprobe_dispatches_the_shared_probe_command() {
    let surface = harness::SurfaceHarness::new(Some(desktop_snapshot())).unwrap();

    let command = single_command(surface.refresh_home_egress());

    assert_eq!(command, CommandIntent::RefreshPublicIpProbe);
}
