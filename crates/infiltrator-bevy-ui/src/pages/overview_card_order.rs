//! The Overview page's reorderable card slots (DUAL-03-12).
//!
//! The shared [`OverviewLayoutSnapshot`] owns the card order; this module
//! mounts one slot per card with the shared up/down controls and turns a move
//! into the shared [`CommandIntent::ReorderOverviewCards`] intent, carrying the
//! *whole* new order. The surface never derives an order of its own and never
//! keeps a shadow copy of the order: the buttons publish a move, the owner
//! answers with the next layout snapshot, and the page renders that order.
//!
//! [`CommandIntent::ReorderOverviewCards`]: infiltrator_contract::command::CommandIntent

use crate::command::{CommandSinkHandle, UiCommand};
use crate::history::TrafficHistory;
use crate::pages::overview::{
    LastOverviewProjection, OverviewCardMoveDownButton, OverviewCardMoveUpButton, OverviewCardSlot,
    OverviewTrafficCard, chips_row_scene, traffic_card_scene,
};
use crate::pages::{overview_cards, overview_public_ip, overview_topology};
use crate::projection::OverviewProjection;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res};
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::overview_layout::{OverviewCardKind, OverviewLayoutSnapshot};

/// The Overview page's cards in the shared layout order, each in its own
/// reorderable slot.
///
/// The order is the shared layout snapshot's; the slot carries the shared
/// up/down controls, so a move publishes the shared reorder intent instead of
/// a locally asserted position.
pub fn overview_card_scenes(
    projection: &OverviewProjection,
    history: &TrafficHistory,
    palette: &UiPalette,
    order: &[OverviewCardKind],
) -> Vec<Box<dyn Scene>> {
    let routing_intent: Box<dyn Scene> = Box::new(
        overview_cards::routing_intent_segmented_controller_scene_with_snapshot(
            &projection.routing_intent,
            palette,
        ),
    );
    let traffic: Box<dyn Scene> = Box::new(bsn! {
            Node {
                width: percent(100),
            }
            OverviewTrafficCard
            Children [
                @{ traffic_card_scene(projection, history, palette) }
            ]
    });
    let metrics: Box<dyn Scene> = Box::new(chips_row_scene(projection, palette));
    let master_switches: Box<dyn Scene> = Box::new(
        overview_cards::master_switches_scene_with_snapshot(&projection.system_toggles, palette),
    );
    let active_exit: Box<dyn Scene> = Box::new(
        overview_cards::active_exit_node_scene_with_snapshot(&projection.active_exit, palette),
    );
    let public_ip: Box<dyn Scene> = Box::new(
        overview_public_ip::public_ip_probe_card_scene_with_snapshot(
            &projection.public_ip,
            palette,
        ),
    );
    let topology: Box<dyn Scene> = Box::new(overview_topology::topology_chain_scene_with_snapshot(
        &projection.traffic_topology,
        palette,
    ));
    let quota: Box<dyn Scene> = Box::new(overview_cards::subscription_quota_scene_with_snapshot(
        &projection.subscription_quota,
        palette,
    ));

    let mut remaining: Vec<(OverviewCardKind, Box<dyn Scene>)> = vec![
        (OverviewCardKind::RoutingIntentSegment, routing_intent),
        (OverviewCardKind::Traffic, traffic),
        (OverviewCardKind::Metrics, metrics),
        (OverviewCardKind::MasterSwitches, master_switches),
        (OverviewCardKind::ActiveExit, active_exit),
        (OverviewCardKind::PublicIp, public_ip),
        (OverviewCardKind::Topology, topology),
        (OverviewCardKind::Quota, quota),
    ];

    let mut ordered: Vec<Box<dyn Scene>> = Vec::with_capacity(remaining.len());
    for kind in order {
        let Some(index) = remaining.iter().position(|(entry, _)| entry == kind) else {
            continue;
        };
        let (_, card) = remaining.remove(index);
        ordered.push(reorderable_card_scene(*kind, card, order, palette));
    }
    ordered
}

/// The card order the shared layout snapshot publishes.
///
/// The owner's order comes first; a card the owner did not list is appended in
/// the contract's default order, so every card is rendered exactly once and
/// no card can disappear because a layout listed only some of them.
pub fn published_order(layout: &OverviewLayoutSnapshot) -> Vec<OverviewCardKind> {
    let mut order: Vec<OverviewCardKind> =
        Vec::with_capacity(OverviewCardKind::DEFAULT_ORDER.len());
    for kind in &layout.order {
        if !order.contains(kind) {
            order.push(*kind);
        }
    }
    for kind in OverviewCardKind::DEFAULT_ORDER {
        if !order.contains(&kind) {
            order.push(kind);
        }
    }
    order
}

/// The shared order after one card moved up or down.
///
/// The move itself is the shared layout arithmetic, so both surfaces agree on
/// what a move means; this function only reads the result.
pub fn moved_order(
    order: &[OverviewCardKind],
    kind: OverviewCardKind,
    up: bool,
) -> Vec<OverviewCardKind> {
    let mut layout = OverviewLayoutSnapshot::new(order.to_vec());
    if up {
        layout.move_up(kind);
    } else {
        layout.move_down(kind);
    }
    layout.order
}

/// One reorderable card slot: the shared up/down controls above the card.
pub fn reorderable_card_scene(
    kind: OverviewCardKind,
    card: Box<dyn Scene>,
    order: &[OverviewCardKind],
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let position = order.iter().position(|entry| *entry == kind);
    let can_up = position.is_some_and(|index| index > 0);
    let can_down = position.is_some_and(|index| index + 1 < order.len());
    let controls = move_controls_scene(kind, can_up, can_down, palette);
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S4),
            }
            OverviewCardSlot({ kind })
            Children [
                @{ controls }
                --
                @{ card }
            ]
    })
}

/// The shared move controls of one card slot.
fn move_controls_scene(
    kind: OverviewCardKind,
    can_up: bool,
    can_down: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                column_gap: rem(space::S4),
            }
            Children [
                @{ move_up_button_scene(kind, can_up, palette) }
                --
                @{ move_down_button_scene(kind, can_down, palette) }
            ]
    })
}

/// The "move this card up" control.
fn move_up_button_scene(
    kind: OverviewCardKind,
    enabled: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px),
                min_width: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            OverviewCardMoveUpButton({ kind })
            ButtonDisabled({ !enabled })
            Button
            Children [
                Text({ "↑".to_owned() }) TextRole(Role::BodyStrong)
            ]
    })
}

/// The "move this card down" control.
fn move_down_button_scene(
    kind: OverviewCardKind,
    enabled: bool,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(palette.control_height_px),
                min_width: rem(palette.control_height_px),
                padding: UiRect::horizontal(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            OverviewCardMoveDownButton({ kind })
            ButtonDisabled({ !enabled })
            Button
            Children [
                Text({ "↓".to_owned() }) TextRole(Role::BodyStrong)
            ]
    })
}

/// A move-up activation: publish the shared reorder intent with the new order.
pub(crate) fn on_card_move_up_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewCardMoveUpButton>,
    last: Option<Res<LastOverviewProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    submit_reorder(&handle, last.as_deref(), button.0, true);
}

/// A move-down activation: publish the shared reorder intent with the new order.
pub(crate) fn on_card_move_down_activated(
    activate: On<Activate>,
    buttons: Query<&OverviewCardMoveDownButton>,
    last: Option<Res<LastOverviewProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    submit_reorder(&handle, last.as_deref(), button.0, false);
}

/// Publish the shared reorder intent for one move.
///
/// The move is computed against the order the owner last published, and the
/// whole new order travels with the intent: the owner owns the layout, so the
/// surface never asserts a position of its own.
fn submit_reorder(
    handle: &CommandSinkHandle,
    last: Option<&LastOverviewProjection>,
    kind: OverviewCardKind,
    up: bool,
) {
    let order = match last.and_then(|last| last.0.as_ref()) {
        Some(projection) => published_order(&projection.layout),
        None => OverviewCardKind::DEFAULT_ORDER.to_vec(),
    };
    let next = moved_order(&order, kind, up);
    if next == order {
        return;
    }
    handle.submit(UiCommand::ReorderOverviewCards { order: next });
}

/// The move controls are only reachable when the card can actually move, so
/// the disabled bit of the two controls is the shared order's own answer.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_publishes_the_whole_new_order() {
        let order = OverviewCardKind::DEFAULT_ORDER.to_vec();
        let moved = moved_order(&order, OverviewCardKind::Traffic, true);
        assert_eq!(moved[0], OverviewCardKind::Traffic);
        assert_eq!(moved[1], OverviewCardKind::RoutingIntentSegment);
        assert_eq!(moved.len(), order.len());
    }

    #[test]
    fn a_move_of_the_first_or_last_card_is_a_no_op() {
        let order = OverviewCardKind::DEFAULT_ORDER.to_vec();
        assert_eq!(
            moved_order(&order, OverviewCardKind::RoutingIntentSegment, true),
            order
        );
        assert_eq!(moved_order(&order, OverviewCardKind::Quota, false), order);
    }

    #[test]
    fn an_unordered_layout_publishes_the_contract_default_order() {
        let layout = OverviewLayoutSnapshot {
            order: Vec::new(),
            is_customized: false,
        };
        assert_eq!(
            published_order(&layout),
            OverviewCardKind::DEFAULT_ORDER.to_vec()
        );
    }
}
