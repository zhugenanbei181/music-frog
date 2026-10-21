//! Behaviour tests for the Overview card-order owner.
//!
//! The owner is the only writer of the card order: every test proves that a
//! mutation really moved the owner's state, that an invalid order is refused
//! with a typed failure, and that a stale observation cannot move the state.

use super::*;

#[test]
fn a_move_applies_the_swap_against_the_owner_and_reads_the_order_back() {
    let owner = OverviewLayoutApplication::new();
    let mutation = owner.move_card(OverviewCardKind::Traffic, OverviewCardMoveDirection::Up);

    let order = &mutation.snapshot().order;
    assert_eq!(order[0], OverviewCardKind::Traffic);
    assert_eq!(order[1], OverviewCardKind::RoutingIntentSegment);
    assert!(mutation.snapshot().is_customized);
    assert_eq!(mutation.revision(), 1);
    // The owner's own state really changed.
    assert_eq!(owner.snapshot(), *mutation.snapshot());
    assert_eq!(owner.revision(), 1);
}

#[test]
fn a_move_of_the_first_or_last_card_does_not_move_the_state() {
    let owner = OverviewLayoutApplication::new();
    let mutation = owner.move_card(
        OverviewCardKind::RoutingIntentSegment,
        OverviewCardMoveDirection::Up,
    );

    assert!(matches!(mutation, OverviewLayoutMutation::Unchanged { .. }));
    assert_eq!(owner.revision(), 0);
    assert_eq!(
        owner.snapshot().order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
}

#[test]
fn an_order_that_repeats_a_card_is_refused_with_a_typed_failure() {
    let owner = OverviewLayoutApplication::new();
    let failure = owner
        .reorder(vec![OverviewCardKind::Traffic, OverviewCardKind::Traffic])
        .expect_err("a repeated card is not an order");

    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert!(!failure.retryable);
    // The refused order never reached the owner.
    assert_eq!(
        owner.snapshot().order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
    assert_eq!(owner.revision(), 0);
}

#[test]
fn an_order_that_names_no_card_is_refused_with_a_typed_failure() {
    let owner = OverviewLayoutApplication::new();
    let failure = owner
        .reorder(Vec::new())
        .expect_err("an empty order is not an order");

    assert_eq!(failure.code, ErrorCode::InvalidInput);
    assert_eq!(
        owner.snapshot().order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
}

#[test]
fn a_stale_observation_cannot_move_the_owner_state() {
    let owner = OverviewLayoutApplication::new();
    let observed = owner.revision();
    owner.move_card(OverviewCardKind::Traffic, OverviewCardMoveDirection::Up);

    let stale_order = vec![
        OverviewCardKind::Quota,
        OverviewCardKind::RoutingIntentSegment,
        OverviewCardKind::Traffic,
        OverviewCardKind::Metrics,
        OverviewCardKind::MasterSwitches,
        OverviewCardKind::ActiveExit,
        OverviewCardKind::PublicIp,
        OverviewCardKind::Topology,
    ];
    let failure = owner
        .reorder_at_revision(stale_order.clone(), observed)
        .expect_err("the observed revision is stale");

    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(failure.retryable);
    // The stale answer did not move the owner's state.
    assert_eq!(owner.snapshot().order[0], OverviewCardKind::Traffic);
    assert_eq!(owner.revision(), 1);
}

#[test]
fn a_fresh_observation_applies_the_new_order() {
    let owner = OverviewLayoutApplication::new();
    let observed = owner.revision();
    let order = vec![OverviewCardKind::Quota, OverviewCardKind::Traffic];
    let mutation = owner
        .reorder_at_revision(order.clone(), observed)
        .expect("the observed revision is current");

    assert_eq!(mutation.snapshot().order, order);
    assert_eq!(owner.snapshot().order, order);
    assert_eq!(owner.revision(), 1);
}

#[test]
fn the_reset_restores_the_canonical_order() {
    let owner = OverviewLayoutApplication::new();
    owner.move_card(OverviewCardKind::Traffic, OverviewCardMoveDirection::Up);
    let mutation = owner.reset();

    assert_eq!(
        mutation.snapshot().order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
    assert!(!mutation.snapshot().is_customized);
    assert_eq!(
        owner.snapshot().order,
        OverviewCardKind::DEFAULT_ORDER.to_vec()
    );
}
