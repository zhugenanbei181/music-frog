//! test-intent: behavior
//! Group expansion and identity are stored in typed outbound preferences.
use crate::test_support::demo_env;
use iced::advanced::widget::Tree;
use infiltrator_contract::outbound::{
    details::OutboundSnapshot,
    identity::{OutboundId, SnapshotRevision},
    inventory::{
        OutboundInventorySnapshot, OutboundInventoryState, OutboundSelectionSet,
        OutboundSelectionState, SelectionAvailability,
    },
};
use infiltrator_contract::surface_snapshot::PageData;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;

fn mounted_nodes(tree: &Tree) -> usize {
    1 + tree.children.iter().map(mounted_nodes).sum::<usize>()
}

fn demo_state() -> AppState {
    AppState::demo(&demo_env(Route::Proxies)).0
}

fn group_id(state: &AppState, name: &str) -> OutboundId {
    state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .and_then(|inventory| match inventory.inventory() {
            OutboundInventoryState::Ready { collections, .. } => collections
                .iter()
                .find(|collection| collection.id.value() == name)
                .map(|collection| collection.id.clone()),
            _ => None,
        })
        .expect("typed collection ID")
}

fn is_expanded(state: &AppState, name: &str) -> bool {
    let Some(id) = state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .and_then(|inventory| match inventory.inventory() {
            OutboundInventoryState::Ready { collections, .. } => collections
                .iter()
                .find(|collection| collection.id.value() == name)
                .map(|collection| &collection.id),
            _ => None,
        })
    else {
        return false;
    };
    state
        .runtime
        .outbound_ui_preferences
        .data
        .as_ref()
        .is_none_or(|preferences| !preferences.collapsed_group_ids.contains(id))
}

#[test]
fn group_fold_replays_a_real_smaller_widget_tree_and_reopens_without_side_effects() {
    let mut state = demo_state();
    let (name, members) = state.runtime.filtered_groups.first().unwrap().clone();
    let id = group_id(&state, &name);
    assert!(!members.is_empty());
    assert!(state.runtime.outbound_ui_preferences.data.is_some());
    let original = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(id.clone()))
            .units(),
        0
    );
    assert!(!is_expanded(&state, &name));
    let collapsed = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert!(
        collapsed < original,
        "closing the group must remove its visible native node cards"
    );
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(id))
            .units(),
        0
    );
    assert!(is_expanded(&state, &name));
    assert_eq!(
        mounted_nodes(&Tree::new(state.view().as_widget())),
        original
    );
}

#[test]
fn group_fold_keeps_other_identity_preferences_and_rejects_an_unknown_collection() {
    let mut state = demo_state();
    let groups: Vec<_> = state
        .runtime
        .filtered_groups
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    assert!(groups.len() > 1);
    let first = group_id(&state, &groups[0]);
    let second = group_id(&state, &groups[1]);
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(first.clone()))
            .units(),
        0
    );
    assert!(!is_expanded(&state, &groups[0]));
    let before_unknown = state
        .runtime
        .outbound_ui_preferences
        .data
        .clone()
        .expect("typed preferences");
    state.runtime.filtered_groups.reverse();
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(second.clone()))
            .units(),
        0
    );
    assert!(!is_expanded(&state, &groups[1]));
    assert!(!is_expanded(&state, &groups[0]));
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(second))
            .units(),
        0
    );
    assert!(is_expanded(&state, &groups[1]));

    let unknown = OutboundId::new(first.engine_id().clone(), "not-observed")
        .expect("valid but unobserved typed ID");
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(unknown))
            .units(),
        0
    );
    let after_unknown = state
        .runtime
        .outbound_ui_preferences
        .data
        .as_ref()
        .expect("typed preferences after ignored request");
    assert_eq!(after_unknown.search_query, before_unknown.search_query);
    assert_eq!(
        after_unknown.favorite_outbound_ids,
        before_unknown.favorite_outbound_ids
    );
    assert_eq!(after_unknown.group_order, before_unknown.group_order);
    assert!(after_unknown.collapsed_group_ids.contains(&first));
    assert!(
        !after_unknown
            .collapsed_group_ids
            .contains(&group_id(&state, &groups[1]))
    );
}

#[test]
fn newly_published_typed_collection_expands_without_reopening_a_collapsed_group() {
    let mut state = demo_state();
    let (closed, visible_members) = state.runtime.filtered_groups[0].clone();
    let closed_id = group_id(&state, &closed);
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(closed_id.clone()))
            .units(),
        0
    );
    let collapsed = mounted_nodes(&Tree::new(state.view().as_widget()));
    let scope = state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .expect("typed demo inventory")
        .scope()
        .clone();
    let current = state.runtime.outbound_inventory.data.as_ref().unwrap();
    let OutboundInventoryState::Ready {
        mut entries,
        mut collections,
    } = current.inventory().clone()
    else {
        panic!("typed demo inventory is ready")
    };
    let outbound_id = state
        .runtime
        .outbound_inventory
        .data
        .as_ref()
        .and_then(|inventory| match inventory.inventory() {
            OutboundInventoryState::Ready { entries, .. } => entries
                .iter()
                .find(|entry| entry.id.value() == visible_members[0])
                .map(|entry| entry.id.clone()),
            _ => None,
        })
        .expect("visible member ID");
    let new_group = OutboundId::new(scope.identity.engine.clone(), "newly-published-group")
        .expect("valid typed group ID");
    entries.push(OutboundSnapshot {
        id: new_group.clone(),
        display_name: Some(new_group.value().to_owned()),
    });
    collections.push(OutboundSelectionSet {
        id: new_group.clone(),
        member_ids: vec![outbound_id.clone()],
        availability: SelectionAvailability::Selectable,
        selection: OutboundSelectionState::Ready {
            selected: outbound_id,
        },
    });
    let next_inventory = OutboundInventorySnapshot::new(
        scope,
        SnapshotRevision::new(current.revision().value() + 1),
        OutboundInventoryState::Ready {
            entries,
            collections,
        },
    )
    .expect("valid typed inventory with a new collection");
    let mut surface = state.surface.latest().unwrap().clone();
    surface.revision += 1;
    surface.outbound_inventory = PageData::ready(next_inventory);
    assert!(state.apply_shared_surface_snapshot(surface));

    assert!(!is_expanded(&state, &closed));
    assert!(is_expanded(&state, new_group.value()));
    let expanded = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert!(
        expanded > collapsed,
        "new collection renders its observed member"
    );
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(new_group.clone()))
            .units(),
        0
    );
    let folded = mounted_nodes(&Tree::new(state.view().as_widget()));
    assert!(folded < expanded);
    assert!(!is_expanded(&state, &closed));
    assert_eq!(
        state
            .update(Message::ToggleOutboundGroupExpanded(new_group))
            .units(),
        0
    );
    assert_eq!(
        mounted_nodes(&Tree::new(state.view().as_widget())),
        expanded
    );
}

#[test]
fn nonanimated_proxy_capture_requests_real_frames_until_its_receipt_is_written() {
    use infiltrator_contract::parity::FeatureId;
    use std::path::PathBuf;
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    let mut state = demo_state();
    let baseline = state.subscription().units();
    state.shell.capture_marker = Some(PathBuf::from("unused-native-layout-receipt"));
    state.shell.capture_scenario = Some(FeatureId::ProxiesGroupExpanded);
    assert_eq!(state.subscription().units(), baseline + 1);
    assert_eq!(state.update(Message::TickFrame(Instant::now())).units(), 1);
    state
        .shell
        .capture_marker_written
        .store(true, Ordering::SeqCst);
    assert_eq!(state.subscription().units(), baseline);
    assert_eq!(state.update(Message::TickFrame(Instant::now())).units(), 0);
}

#[test]
fn stable_node_widget_ids_keep_names_with_delimiters_distinct() {
    use infiltrator_iced::view::proxies::node_region_id;
    assert_ne!(node_region_id("a:b", "c"), node_region_id("a", "b:c"));
    assert_eq!(
        node_region_id("香港", "node:one"),
        node_region_id("香港", "node:one")
    );
}
