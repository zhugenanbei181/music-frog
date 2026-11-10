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
    LastOverviewProjection, OverviewCardMoveControls, OverviewCardMoveDownButton,
    OverviewCardMoveUpButton, OverviewCardSlot, OverviewCardSlots, OverviewTrafficCard,
    traffic_card_scene,
};
use crate::pages::overview_traffic_scenes::chips_row_scene;
use crate::pages::{overview_cards, overview_public_ip, overview_topology};
use crate::projection::OverviewProjection;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, percent};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::radius;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::design_resources;
use infiltrator_contract::overview_layout::{OverviewCardKind, OverviewLayoutSnapshot};

/// One card move control's square extent in baseline pixels. The reorder pair
/// is a quiet affordance — the reference dashboard shows no move chrome at
/// all, so the controls stay smaller than a real control and carry no fill.
const CARD_MOVE_CONTROL_PX: f32 = 26.0;

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
    // The shared layout contract marks the routing selector and the master
    // switches `DashboardFixed`: the Overview page mounts them once, here,
    // before the reorderable slots, and no other region may mount a second
    // copy. The sidebar used to mount both as well, which is why the region
    // was renamed away from a location.
    let routing_intent: Box<dyn Scene> = Box::new(
        overview_cards::routing_intent_segmented_controller_scene_with_snapshot(
            &projection.routing_intent,
            palette,
        ),
    );
    let master_switches: Box<dyn Scene> = Box::new(
        overview_cards::master_switches_scene_with_snapshot(&projection.system_toggles, palette),
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

    let shell_sidebar: Vec<Box<dyn Scene>> = vec![routing_intent, master_switches];
    let mut remaining: Vec<(OverviewCardKind, Box<dyn Scene>)> = vec![
        (OverviewCardKind::Traffic, traffic),
        (OverviewCardKind::Metrics, metrics),
        (OverviewCardKind::ActiveExit, active_exit),
        (OverviewCardKind::PublicIp, public_ip),
        (OverviewCardKind::Topology, topology),
        (OverviewCardKind::Quota, quota),
    ];

    let mut ordered: Vec<Box<dyn Scene>> = Vec::with_capacity(remaining.len());
    for kind in order.iter().copied().filter(|kind| kind.is_overview_card()) {
        let Some(index) = remaining.iter().position(|(entry, _)| *entry == kind) else {
            continue;
        };
        let (_, card) = remaining.remove(index);
        ordered.push(reorderable_card_scene(kind, card, order, palette));
    }
    shell_sidebar.into_iter().chain(ordered).collect()
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
        if kind.is_overview_card() && !order.contains(kind) {
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
            OverviewCardMoveControls
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
    _palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(CARD_MOVE_CONTROL_PX),
                min_width: rem(CARD_MOVE_CONTROL_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(radius::PILL)),
            }
            OverviewCardMoveUpButton({ kind })
            ButtonDisabled({ !enabled })
            Button
            Children [
                Text({ "↑".to_owned() })
                TextRole(design_resources::TextRole::Body)
                ThemeTextColor({ Token::InkDim })
            ]
    })
}

/// The "move this card down" control.
fn move_down_button_scene(
    kind: OverviewCardKind,
    enabled: bool,
    _palette: &UiPalette,
) -> Box<dyn Scene> {
    Box::new(bsn! {
            Node {
                min_height: rem(CARD_MOVE_CONTROL_PX),
                min_width: rem(CARD_MOVE_CONTROL_PX),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(rem(radius::PILL)),
            }
            OverviewCardMoveDownButton({ kind })
            ButtonDisabled({ !enabled })
            Button
            Children [
                Text({ "↓".to_owned() })
                TextRole(design_resources::TextRole::Body)
                ThemeTextColor({ Token::InkDim })
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

/// Put the card slots back in the order the owner published.
///
/// The move controls publish the shared reorder intent and the owner answers
/// with the next layout snapshot. The page renders that answer: the slots are
/// sorted by the published order, so the card order on screen is the owner's
/// order and never a local guess.
pub(crate) fn sync_overview_card_order(
    last: Option<Res<LastOverviewProjection>>,
    mut containers: Query<&mut Children, With<OverviewCardSlots>>,
    slots: Query<(Entity, &OverviewCardSlot)>,
) {
    let Some(order) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .map(|projection| published_order(&projection.layout))
    else {
        return;
    };
    let Ok(mut children) = containers.single_mut() else {
        return;
    };
    let mut wanted: Vec<Entity> = Vec::with_capacity(order.len());
    for kind in &order {
        let Some((entity, _)) = slots.iter().find(|(_, slot)| slot.0 == *kind) else {
            continue;
        };
        if !wanted.contains(&entity) {
            wanted.push(entity);
        }
    }
    // A card the owner did not list keeps its place at the end: no card may
    // disappear because a layout listed only some of them.
    let current: Vec<Entity> = children.iter().copied().collect();
    for entity in &current {
        if !wanted.contains(entity) {
            wanted.push(*entity);
        }
    }
    // The sort is stable, so a card the owner did not list keeps its relative
    // order behind the listed ones.
    children.sort_by_key(|entity| {
        wanted
            .iter()
            .position(|listed| listed == entity)
            .unwrap_or(wanted.len())
    });
}

/// The move controls are only reachable when the card can actually move, so
/// the disabled bit of the two controls is the shared order's own answer.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_publishes_the_whole_new_order() {
        let order = OverviewCardKind::DEFAULT_ORDER.to_vec();
        let moved = moved_order(&order, OverviewCardKind::Metrics, true);
        assert_eq!(moved[0], OverviewCardKind::Metrics);
        assert_eq!(moved[1], OverviewCardKind::Traffic);
        assert_eq!(moved.len(), order.len());
    }

    #[test]
    fn a_move_of_the_first_or_last_card_is_a_no_op() {
        let order = OverviewCardKind::DEFAULT_ORDER.to_vec();
        assert_eq!(moved_order(&order, OverviewCardKind::Traffic, true), order);
        assert_eq!(moved_order(&order, OverviewCardKind::Quota, false), order);
    }

    /// The dashboard's fixed controls are not reorderable Overview cards and
    /// never become one because a layout listed them.
    #[test]
    fn dashboard_fixed_entries_are_not_overview_cards() {
        let layout = OverviewLayoutSnapshot {
            order: OverviewCardKind::DASHBOARD_FIXED_ORDER.to_vec(),
            is_customized: true,
        };
        assert_eq!(
            published_order(&layout),
            OverviewCardKind::DEFAULT_ORDER.to_vec()
        );
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
