//! SLINT-001.10: the Overview card order of the real Slint surface.
//!
//! The Overview card order is the shared owner's fact. The surface names the
//! card and the direction, the owner applies the swap against the order it
//! holds, and the surface renders the order the owner published. The cases
//! below therefore cannot pass with a surface-local copy of the order: the
//! dispatch case asserts the shared intent, the render case asserts the
//! owner's published order, and the refusal case asserts that a refused move
//! leaves the published order untouched.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::overview_layout::OverviewCardKind;
use infiltrator_contract::overview_layout::{
    OverviewCardKind, OverviewCardMoveDirection, OverviewLayoutSnapshot,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::SlintAction;
use infiltrator_slint::surface::harness::SurfaceHarness;
use infiltrator_slint::surface::harness::SurfaceOutcome;
use lurker_core_contract::error::{ErrorCode, Failure};

/// The owner snapshot the Overview page reads.
fn overview_snapshot(order: Vec<OverviewCardKind>) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::SlintDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "initial", true),
    );
    snapshot.overview_layout = OverviewLayoutSnapshot::new(order);
    snapshot
}

/// The ids of one card order.
fn ids(order: &[OverviewCardKind]) -> Vec<String> {
    order.iter().map(|kind| kind.as_str().to_owned()).collect()
}

/// The canonical order of the shared contract.
fn default_order() -> Vec<OverviewCardKind> {
    OverviewCardKind::DEFAULT_ORDER.to_vec()
}

#[test]
fn the_card_move_dispatches_the_shared_move_intent() {
    let surface = SurfaceHarness::new(Some(overview_snapshot(default_order()))).unwrap();
    surface.publish_snapshot(overview_snapshot(default_order()));

    let SurfaceOutcome::Dispatched(actions) = surface.move_overview_card("traffic", true) else {
        panic!("the move must dispatch the shared intent");
    };
    assert_eq!(actions.len(), 1, "one move dispatches one owner command");
    match &actions[0] {
        SlintAction::Command(CommandIntent::MoveOverviewCard { kind, direction }) => {
            assert_eq!(*kind, OverviewCardKind::Traffic);
            assert_eq!(*direction, OverviewCardMoveDirection::Up);
        }
        other => panic!("the move must dispatch the shared move intent, got {other:?}"),
    }

    let SurfaceOutcome::Dispatched(actions) = surface.move_overview_card("metrics", false) else {
        panic!("the move must dispatch the shared intent");
    };
    match &actions[0] {
        SlintAction::Command(CommandIntent::MoveOverviewCard { kind, direction }) => {
            assert_eq!(*kind, OverviewCardKind::Metrics);
            assert_eq!(*direction, OverviewCardMoveDirection::Down);
        }
        other => panic!("the move must dispatch the shared move intent, got {other:?}"),
    }
}

#[test]
fn the_card_list_renders_the_order_the_owner_published() {
    // The owner publishes a custom order; the surface renders exactly that
    // order, so the surface cannot hold a copy of its own.
    let owner_order = vec![
        OverviewCardKind::Metrics,
        OverviewCardKind::Traffic,
        OverviewCardKind::RoutingIntentSegment,
        OverviewCardKind::MasterSwitches,
        OverviewCardKind::ActiveExit,
        OverviewCardKind::PublicIp,
        OverviewCardKind::Topology,
        OverviewCardKind::Quota,
    ];
    let surface = SurfaceHarness::new(Some(overview_snapshot(owner_order.clone()))).unwrap();
    surface.publish_snapshot(overview_snapshot(owner_order.clone()));

    assert_eq!(surface.overview_card_order(), ids(&owner_order));
}

#[test]
fn a_refused_move_leaves_the_published_order_untouched() {
    let surface = SurfaceHarness::new(Some(overview_snapshot(default_order()))).unwrap();
    surface.publish_snapshot(overview_snapshot(default_order()));

    surface.move_overview_card("traffic", true);
    let failure = Failure::new(ErrorCode::Internal, "the owner refused the command", true);
    surface.publish_owner_failure(&failure);

    // The surface has no order of its own, so a refused move leaves the
    // owner's published order exactly as it was.
    assert_eq!(surface.overview_card_order(), ids(&default_order()));
}

#[test]
fn the_reset_dispatches_the_shared_reset_intent() {
    let surface = SurfaceHarness::new(Some(overview_snapshot(default_order()))).unwrap();
    surface.publish_snapshot(overview_snapshot(default_order()));

    let SurfaceOutcome::Dispatched(actions) = surface.reset_overview_card_order() else {
        panic!("the reset must dispatch the shared intent");
    };
    assert_eq!(actions.len(), 1, "one reset dispatches one owner command");
    match &actions[0] {
        SlintAction::Command(CommandIntent::ResetOverviewCardOrder) => {}
        other => panic!("the reset must dispatch the shared reset intent, got {other:?}"),
    }
}

#[test]
fn a_card_the_list_does_not_offer_is_refused_with_a_typed_failure() {
    let surface = SurfaceHarness::new(Some(overview_snapshot(default_order()))).unwrap();
    surface.publish_snapshot(overview_snapshot(default_order()));

    let SurfaceOutcome::Rejected { failure, actions } =
        surface.move_overview_card("not-a-card", true)
    else {
        panic!("an unknown card must be refused");
    };
    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert!(
        actions.is_empty(),
        "a refused move must not dispatch an owner command"
    );
    assert_eq!(surface.overview_card_order(), ids(&default_order()));
}

#[test]
fn the_card_order_sheet_opens_the_way_the_overview_page_opens_it() {
    let surface = SurfaceHarness::new(Some(overview_snapshot(default_order()))).unwrap();
    surface.publish_snapshot(overview_snapshot(default_order()));

    surface.open_overview_order();
    assert!(
        surface.window().get_overview_order_open(),
        "the card order sheet must be open after the overview page opens it"
    );
}
